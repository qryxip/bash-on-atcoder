use anyhow::{bail, Context as _};
use env_logger::fmt::{Color, WriteStyle};
use log::{info, Level, LevelFilter};
use maplit::hashmap;
use once_cell::sync::Lazy;
use reqwest::redirect::Policy;
use scraper::selector::Selector;
use scraper::Html;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use structopt::StructOpt;
use url::Url;

use std::env::{self, VarError};
use std::io::{self, Write as _};
use std::num::ParseIntError;
use std::time::Duration;
use std::{str, thread};

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

    let client = setup_client(timeout)?;

    let csrf_token = get(&client, "/login", &[200])?
        .html()?
        .extract_csrf_token()?;
    let payload = hashmap!(
        "csrf_token" => csrf_token,
        "username" => username,
        "password" => password,
    );
    post_form(&client, "/login", &payload, &[302])?;
    if get(&client, "/settings", &[200, 302])?.status() == 302 {
        bail!("Failed to login");
    }

    let csrf_token = get(
        &client,
        &format!("/contests/{}/custom_test", contest),
        &[200],
    )?
    .html()?
    .extract_csrf_token()?;

    let code = shell_escape::unix::escape(code.into());

    let md5sum = {
        let code = format!(
            r#"CODE={}
output="$(bash -c "$CODE" && printf '#')" && echo -n "${{output%#}}" > ./output && md5sum ./output"#,
            code,
        );
        submit_bash_code(&client, &contest, &csrf_token, &code)?
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
        acc.extend(submit_bash_code(&client, &contest, &csrf_token, &code)?);
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
        client: &reqwest::blocking::Client,
        contest: &str,
        csrf_token: &str,
        code: &str,
    ) -> anyhow::Result<Vec<u8>> {
        let payload = hashmap!(
            "data.LanguageId" => BASH_ID,
            "sourceCode" => code,
            "input" => "",
            "csrf_token" => csrf_token,
        );

        post_form(
            &client,
            &format!("/contests/{}/custom_test/submit/json", contest),
            &payload,
            &[200],
        )?;

        return loop {
            let ResponsePayload { result } = get(
                &client,
                &format!("/contests/{}/custom_test/json", contest),
                &[200],
            )?
            .json()?;

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

fn setup_client(timeout: Option<Duration>) -> reqwest::Result<reqwest::blocking::Client> {
    return reqwest::blocking::ClientBuilder::new()
        .user_agent(USER_AGENT)
        .cookie_store(true)
        .redirect(Policy::none())
        .referer(false)
        .timeout(timeout)
        .build();

    static USER_AGENT: &str = "bash-on-atcoder <qryxip@gmail.com>";
}

fn get(
    client: &reqwest::blocking::Client,
    path: &str,
    statuses: &[u16],
) -> anyhow::Result<reqwest::blocking::Response> {
    let url = url(path)?;
    info!("GET {}", url);
    let res = client.get(url.clone()).send()?;
    info!("{}", res.status());
    if !statuses.contains(&res.status().as_u16()) {
        bail!("{}: expected {:?}, got {}", url, statuses, res.status());
    }
    Ok(res)
}

fn post_form(
    client: &reqwest::blocking::Client,
    path: &str,
    form: &impl Serialize,
    statuses: &[u16],
) -> anyhow::Result<reqwest::blocking::Response> {
    let url = url(path)?;
    info!("POST {}", url);
    let res = client.post(url.clone()).form(form).send()?;
    info!("{}", res.status());
    if !statuses.contains(&res.status().as_u16()) {
        bail!("{}: expected {:?}, got {}", url, statuses, res.status());
    }
    Ok(res)
}

fn url(path: &str) -> std::result::Result<Url, url::ParseError> {
    return BASE.join(path);
    static BASE: Lazy<Url> = Lazy::new(|| "https://atcoder.jp".parse().unwrap());
}

trait ResponseExt {
    fn html(self) -> reqwest::Result<Html>;
}

impl ResponseExt for reqwest::blocking::Response {
    fn html(self) -> reqwest::Result<Html> {
        let text = self.text()?;
        Ok(Html::parse_document(&text))
    }
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
