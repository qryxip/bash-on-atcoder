# bash-on-atcoder

[![CI](https://github.com/qryxip/bash-on-atcoder/workflows/CI/badge.svg)](https://github.com/qryxip/bash-on-atcoder/actions?workflow=CI)
[![Crates.io](https://img.shields.io/crates/v/bash-on-atcoder.svg)](https://crates.io/crates/bash-on-atcoder)
[![Crates.io](https://img.shields.io/crates/l/bash-on-atcoder.svg)](https://crates.io/crates/bash-on-atcoder)

Run Bash scripts on AtCoder.

```console
❯ envchain atcoder bash-on-atcoder 'ls -l /imojudge' --short
[INFO] GET https://atcoder.jp/login
[INFO] 200
[INFO] POST https://atcoder.jp/login
[INFO] 302
[INFO] GET https://atcoder.jp/settings
[INFO] 200
[INFO] GET https://atcoder.jp/contests/practice/custom_test
[INFO] 200
[INFO] POST https://atcoder.jp/contests/practice/custom_test/submit/json
[INFO] 200
[INFO] GET https://atcoder.jp/contests/practice/custom_test/json
[INFO] 200
[INFO] Result.Status = 1
[INFO] Waiting 2s...
[INFO] GET https://atcoder.jp/contests/practice/custom_test/json
[INFO] 200
[INFO] Result.Status = 3
total 20
drwxr-xr-x 3 contestant contestant 4096 Apr  1  2020 csharp
drwxr-xr-x 3 contestant contestant 4096 Apr  1  2020 fsharp
drwxr-xr-x 4 contestant contestant 4096 Apr  1  2020 rust
drwxr-xr-x 1 contestant contestant 4096 Apr  1  2020 sandbox
drwxr-xr-x 3 contestant contestant 4096 Apr  1  2020 visualbasic
```

```console
❯ envchain atcoder bash-on-atcoder 'cat /imojudge/rust/Cargo.lock' > ./cargo-lock.toml
[INFO] GET https://atcoder.jp/login
[INFO] 200
[INFO] POST https://atcoder.jp/login
[INFO] 302
[INFO] GET https://atcoder.jp/settings
[INFO] 200
[INFO] GET https://atcoder.jp/contests/practice/custom_test
[INFO] 200
[INFO] POST https://atcoder.jp/contests/practice/custom_test/submit/json
[INFO] 200
[INFO] GET https://atcoder.jp/contests/practice/custom_test/json
[INFO] 200
[INFO] Result.Status = 1
[INFO] Waiting 2s...
[INFO] GET https://atcoder.jp/contests/practice/custom_test/json
[INFO] 200
[INFO] Result.Status = 3
[INFO] POST https://atcoder.jp/contests/practice/custom_test/submit/json
[INFO] 200
[INFO] GET https://atcoder.jp/contests/practice/custom_test/json
[INFO] 200
[INFO] Result.Status = 1
[INFO] Waiting 2s...
[INFO] GET https://atcoder.jp/contests/practice/custom_test/json
[INFO] 200
[INFO] Result.Status = 3
...
[INFO] POST https://atcoder.jp/contests/practice/custom_test/submit/json
[INFO] 200
[INFO] GET https://atcoder.jp/contests/practice/custom_test/json
[INFO] 200
[INFO] Result.Status = 1
[INFO] Waiting 2s...
[INFO] GET https://atcoder.jp/contests/practice/custom_test/json
[INFO] 200
[INFO] Result.Status = 3
❯ ll ./cargo-lock.toml
.rw-r--r-- 15k ryo ryo 2020-10-22 14:13 -N ./cargo-lock.toml
```

## Installation

### Crates.io

```console
❯ cargo install bash-on-atcoder
```

### `master`

```console
❯ cargo install --git https://github.com/qryxip/bash-on-atcoder
```

### GitHub Releases

[Releases](https://github.com/qryxip/bash-on-atcoder/releases)

## License

Licensed under [CC0-1.0](https://creativecommons.org/publicdomain/zero/1.0/).
