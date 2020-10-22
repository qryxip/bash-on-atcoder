use anyhow::{bail, Context as _};
use curl::easy::{Easy, Form};
use env_logger::fmt::{Color, WriteStyle};
use log::{info, Level, LevelFilter};
use once_cell::sync::Lazy;
use scraper::{selector::Selector, Html};
use serde::{
    de::{DeserializeOwned, Error as _},
    Deserialize, Deserializer,
};
use std::{
    env::{self, VarError},
    io::{self, Write as _},
    num::ParseIntError,
    str, thread,
    time::Duration,
};
use structopt::StructOpt;
use url::Url;

macro_rules! selector(($s:expr $(,)?) => ({
    static SELECTOR: Lazy<Selector> = Lazy::new(|| Selector::parse($s).unwrap());
    &SELECTOR
}));

fn main() -> anyhow::Result<()> {
    let Opt {
        timeout,
        contest,
        color,
        code,
    } = Opt::from_args();

    init_logger(color);

    let username = env::var("ATCODER_USERNAME").or_else::<anyhow::Error, _>(|err| match err {
        VarError::NotPresent => rprompt::prompt_reply_stderr("Username: ").map_err(Into::into),
        _ => Err(err.into()),
    })?;
    let password = env::var("ATCODER_PASSWORD").or_else::<anyhow::Error, _>(|err| match err {
        VarError::NotPresent => {
            rpassword::read_password_from_tty(Some("Password: ")).map_err(Into::into)
        }
        _ => Err(err.into()),
    })?;

    let mut sess = Session::new(timeout);

    let csrf_token = sess.get_html("/login", &[200])?.extract_csrf_token()?;
    let payload = [
        ("csrf_token", &*csrf_token),
        ("username", &*username),
        ("password", &*password),
    ];
    sess.post_form("/login", &payload, &[302])?;

    if sess.get_status("/settings", &[200, 302])? == 302 {
        bail!("Failed to login");
    }

    let csrf_token = sess
        .get_html(&format!("/contests/{}/custom_test", contest), &[200])?
        .extract_csrf_token()?;

    let code = shell_escape::unix::escape(code.into());

    let md5sum = {
        let code = format!(
            r#"CODE={}
output="$(bash -c "$CODE" && printf '#')" && echo -n "${{output%#}}" > ./output && md5sum ./output"#,
            code,
        );
        submit_bash_code(&mut sess, &contest, &csrf_token, &code)?
    };
    let md5sum = shell_escape::unix::escape(str::from_utf8(&md5sum)?.trim_end().into());

    let mut acc = vec![];
    while {
        let code = format!(
            r#"
CODE={}
MD5SUM={}
OFFSET={}
output="$(bash -c "$CODE" && printf '#')" && output="${{output%#}}" && echo -n "$output" > ./output && echo -n "$MD5SUM" > ./check && md5sum -c ./check >&2 && echo -n "${{output:$OFFSET:{}}}""#,
            code,
            md5sum,
            acc.len(),
            CHUNK_LEN,
        );
        acc.extend(submit_bash_code(&mut sess, &contest, &csrf_token, &code)?);
        acc.len() % CHUNK_LEN == 0
    } {}

    let mut stdout = io::stdout();
    stdout.write_all(&acc)?;
    stdout.flush()?;
    return Ok(());

    const INTERVAL: Duration = Duration::from_secs(2);
    const CHUNK_LEN: usize = 2048;

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct ResponsePayload {
        result: ResponsePayloadResult,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct ResponsePayloadResult {
        #[serde(deserialize_with = "deser_b64")]
        source_code: Vec<u8>,
        #[serde(deserialize_with = "deser_b64")]
        input: Vec<u8>,
        #[serde(deserialize_with = "deser_b64")]
        output: Vec<u8>,
        #[serde(deserialize_with = "deser_b64")]
        error: Vec<u8>,
        exit_code: i32,
        status: u32,
        language_id: u32,
    }

    fn deser_b64<'de, D>(deserializer: D) -> std::result::Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let b64 = String::deserialize(deserializer)?;
        base64::decode(&b64).map_err(D::Error::custom)
    }

    fn submit_bash_code(
        sess: &mut Session,
        contest: &str,
        csrf_token: &str,
        code: &str,
    ) -> anyhow::Result<Vec<u8>> {
        let payload = [
            ("data.LanguageId", BASH_ID),
            ("sourceCode", code),
            ("input", ""),
            ("csrf_token", csrf_token),
        ];

        sess.post_form(
            &format!("/contests/{}/custom_test/submit/json", contest),
            &payload,
            &[200],
        )?;

        return loop {
            let ResponsePayload { result } =
                sess.get_json(&format!("/contests/{}/custom_test/json", contest), &[200])?;

            info!("Result.Status = {}", result.status);

            if result.source_code == code.as_bytes()
                && result.input == b""
                && result.status == 3
                && result.language_id.to_string() == BASH_ID
            {
                if result.exit_code != 0 {
                    bail!(
                        "Failed with code {}: {:?}",
                        result.exit_code,
                        String::from_utf8_lossy(&result.error),
                    );
                }
                break Ok(result.output);
            }

            info!("Waiting {:?}...", INTERVAL);
            thread::sleep(INTERVAL);
        };

        static BASH_ID: &str = "4007";
    }
}

#[derive(StructOpt, Debug)]
struct Opt {
    /// Timeout
    #[structopt(long, value_name("SECS"), parse(try_from_str = parse_seconds))]
    timeout: Option<Duration>,

    #[structopt(long, value_name("STRING"), default_value("practice"))]
    contest: String,

    /// Coloring
    #[structopt(
        long,
        value_name("WHEN"),
        default_value("auto"),
        possible_values(&["auto", "always", "never"]),
        parse(from_str = parse_write_style_unwrap)
    )]
    color: WriteStyle,

    /// Bash code
    code: String,
}

fn parse_seconds(s: &str) -> std::result::Result<Duration, ParseIntError> {
    s.parse().map(Duration::from_millis)
}

/// Parses `s` to a `WriteStyle`.
///
/// # Panics
///
/// Panics `s` is not "auto", "always", or "never".
fn parse_write_style_unwrap(s: &str) -> WriteStyle {
    match s {
        "auto" => WriteStyle::Auto,
        "always" => WriteStyle::Always,
        "never" => WriteStyle::Never,
        _ => panic!(r#"expected "auto", "always", or "never""#),
    }
}

fn init_logger(color: WriteStyle) {
    env_logger::Builder::new()
        .format(|buf, record| {
            macro_rules! style(($fg:expr, $intense:expr) => ({
                let mut style = buf.style();
                style.set_color($fg).set_intense($intense);
                style
            }));

            let color = match record.level() {
                Level::Error => Color::Red,
                Level::Warn => Color::Yellow,
                Level::Info => Color::Cyan,
                Level::Debug => Color::Green,
                Level::Trace => Color::White,
            };

            let path = record
                .module_path()
                .map(|p| p.split("::").next().unwrap())
                .filter(|&p| p != module_path!().split("::").next().unwrap())
                .map(|p| format!(" {}", p))
                .unwrap_or_default();

            writeln!(
                buf,
                "{}{}{}{} {}",
                style!(Color::Black, true).value('['),
                style!(color, false).value(record.level()),
                path,
                style!(Color::Black, true).value(']'),
                record.args(),
            )
        })
        .filter_level(LevelFilter::Info)
        .write_style(color)
        .init();
}

static USER_AGENT: &str = "bash-on-atcoder <https://github.com/qryxip/bash-on-atcoder>";

struct Session {
    handle: Easy,
    timeout: Option<Duration>,
}

impl Session {
    fn new(timeout: Option<Duration>) -> Self {
        Self {
            handle: Easy::new(),
            timeout,
        }
    }

    fn get_status(&mut self, rel_url: &str, statuses: &[u32]) -> anyhow::Result<u32> {
        self.get(rel_url, statuses, |n, _| Ok(n))
    }

    fn get_html(&mut self, rel_url: &str, statuses: &[u32]) -> anyhow::Result<Html> {
        self.get(rel_url, statuses, |_, s| Ok(Html::parse_document(s)))
    }

    fn get_json<T: DeserializeOwned>(
        &mut self,
        rel_url: &str,
        statuses: &[u32],
    ) -> anyhow::Result<T> {
        self.get(rel_url, statuses, |_, s| {
            serde_json::from_str(s).with_context(|| "failed to deserialize the response")
        })
    }

    fn get<F: FnOnce(u32, &str) -> anyhow::Result<O>, O>(
        &mut self,
        rel_url: &str,
        statuses: &[u32],
        deser: F,
    ) -> anyhow::Result<O> {
        self.handle.reset();
        let url = url(rel_url)?;
        info!("GET {}", url);
        self.handle.url(url.as_ref())?;
        self.handle.useragent(USER_AGENT)?;
        self.handle.cookie_file("")?;
        if let Some(timeout) = self.timeout {
            self.handle.timeout(timeout)?;
        }
        let mut data = vec![];
        let mut transfer = self.handle.transfer();
        transfer.write_function(|chunk| {
            data.extend_from_slice(chunk);
            Ok(chunk.len())
        })?;
        transfer.perform()?;
        drop(transfer);
        let response_code = self.handle.response_code()?;
        info!("{}", response_code);
        if !statuses.contains(&response_code) {
            bail!("{}: expected {:?}, got {}", url, statuses, response_code);
        }
        let data = str::from_utf8(&data).with_context(|| "non-UTF8 content")?;
        deser(response_code, data)
    }

    fn post_form(
        &mut self,
        rel_url: &str,
        form: &[(&str, &str)],
        statuses: &[u32],
    ) -> anyhow::Result<()> {
        self.handle.reset();
        let url = url(rel_url)?;
        info!("POST {}", url);
        self.handle.url(url.as_ref())?;
        self.handle.useragent(USER_AGENT)?;
        self.handle.cookie_file("")?;
        if let Some(timeout) = self.timeout {
            self.handle.timeout(timeout)?;
        }
        self.handle.httppost({
            let mut payload = Form::new();
            for (name, value) in form {
                payload.part(name).contents(value.as_ref()).add()?;
            }
            payload
        })?;
        self.handle.perform()?;
        let response_code = self.handle.response_code()?;
        info!("{}", response_code);
        if !statuses.contains(&response_code) {
            bail!("{}: expected {:?}, got {}", url, statuses, response_code);
        }
        Ok(())
    }
}

fn url(path: &str) -> std::result::Result<Url, url::ParseError> {
    return BASE.join(path);
    static BASE: Lazy<Url> = Lazy::new(|| "https://atcoder.jp".parse().unwrap());
}

trait HtmlExt {
    fn extract_csrf_token(&self) -> anyhow::Result<String>;
}

impl HtmlExt for Html {
    fn extract_csrf_token(&self) -> anyhow::Result<String> {
        (|| {
            let token = self
                .select(selector!("[name=\"csrf_token\"]"))
                .next()?
                .value()
                .attr("value")?
                .to_owned();
            Some(token).filter(|token| !token.is_empty())
        })()
        .with_context(|| "failed to find the CSRF token")
        .with_context(|| "failed to scrape")
    }
}
