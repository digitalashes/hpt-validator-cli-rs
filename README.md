# hpt-validator-cli

[![CI](https://github.com/digitalashes/hpt-validator-cli-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/digitalashes/hpt-validator-cli-rs/actions/workflows/ci.yml)

A Rust port of [`@cmsgov/hpt-validator-cli`](https://github.com/CMSgov/hpt-validator-cli),
the command line for validating CMS Hospital Price Transparency machine-readable files.
It runs [`hpt-validator`](https://github.com/digitalashes/hpt-validator-rs), the Rust port
of the validator itself, and prints what upstream's CLI prints.

> **Unofficial.** This project is not affiliated with, endorsed by, or maintained by the
> Centers for Medicare & Medicaid Services. It reproduces the behaviour of the upstream
> tools and does not speak for CMS.

- **Same output as upstream.** The errors and alerts are those of `@cmsgov/hpt-validator`
  2.6.0, in the same order with the same messages, and both output formats are laid out as
  upstream's CLI 1.10.8 lays them out.
- **One binary.** No Node, no network access, no runtime dependencies.
- **Made for scripts and agents.** Exit codes say whether the file is valid, and
  `-o json` gives every finding a stable code. See [AGENTS.md](AGENTS.md).

## Install

Download a binary for macOS, Linux or Windows from the
[releases](https://github.com/digitalashes/hpt-validator-cli-rs/releases), or build one:

```sh
cargo install hpt-validator-cli
```

The binary is `hpt-validator`.

## Usage

```
hpt-validator [options] <filepath> <version>
```

```sh
hpt-validator 123456789_hospital-name_standardcharges.csv v3.0
hpt-validator standardcharges.json.gz 2.2 -o json
curl -s https://example.org/standardcharges.csv | hpt-validator - v3.0 -f csv
```

| Argument or option         | Meaning                                                                              |
|----------------------------|--------------------------------------------------------------------------------------|
| `<filepath>`               | The file to validate, or `-` for standard input. A `.gz` file is decompressed.       |
| `<version>`                | `v2.1`, `v2.2` or `v3.0`. A bare major picks its newest minor: `2` is 2.2.           |
| `-f, --format csv\|json`   | The file format; by default taken from the extension, ignoring `.gz`.                |
| `-o, --output table\|json` | Tables for people (the default) or one JSON object.                                  |
| `-e, --error-limit <n>`    | Stop after `n` errors; alerts are capped at `n` too. Default 1000, `0` for no limit. |
| `--extended`               | Also report the file type and what upstream drops or never reports.                  |
| `-V, --version`            | Print the CLI and validator versions.                                                |

Exit codes:

| Code | Meaning                                                   |
|------|-----------------------------------------------------------|
| 0    | The file is valid. Alerts do not change this.             |
| 1    | The file has errors.                                      |
| 2    | Usage error, or a file type that is neither CSV nor JSON. |
| 3    | The input could not be read.                              |

## Output

`-o json` prints the object upstream prints:

```json
{
  "file": "/data/123456789_hospital-name_standardcharges.csv",
  "validatorVersion": "0.1.0",
  "dataDictionaryVersion": "3.0.0",
  "timestamp": "2026-10-08T09:30:00.000Z",
  "valid": false,
  "errorCount": 1,
  "alertCount": 0,
  "errors": [
    {
      "path": "D5",
      "message": "A value is required for \"setting\". You must encode the missing information.",
      "columnName": "setting",
      "code": "HPT_CSV_REQUIRED_VALUE"
    }
  ],
  "alerts": []
}
```

Each finding has upstream's keys followed by `code` and, for JSON schema errors, `keyword`.
With `--extended` the object ends with `fileType` (`json`, `tall`, `wide` or `unknown`)
and `extended`, a list of further findings, and every CSV finding gets `line`, the
physical line its row starts on. The codes and the Extended findings are described in
[`hpt-validator`'s documentation](https://github.com/digitalashes/hpt-validator-rs#findings).

## Differences from upstream's CLI

- Exit codes. Upstream exits with 0 whatever the result, and with 1 when it crashes.
- No network. Upstream asks the npm registry for the latest CLI version on every run;
  `--version` here prints the CLI and validator versions only.
- `-` reads standard input, and `--extended` is new.
- `validatorVersion` and the version lines name this port's validator.
- A negative `--error-limit` is refused. Upstream takes it as no limit on errors and
  drops every row alert.
- A missing or unreadable file is an error message and exit code 3, not a stack trace.
- Table output: times are in ISO 8601 UTC rather than local time, cells are never
  coloured, and the `originalError` cell of a JSON syntax error shows `{}` rather than the
  parser's stack trace. Character widths follow Node's rules for builds without ICU, so a
  table can be misaligned by emoji and rare combining marks.
- In a gzipped JSON file with a syntax error after the first 16 KiB, the position in the
  error message can differ, because upstream's decompressor passes the text on in
  smaller pieces.

## How parity is tested

`cargo test` runs the CLI on every case of `hpt-validator`'s test suite at each version
the CLI accepts, and compares its JSON output with upstream's results for the case. It
expects `hpt-validator-rs` to be checked out next to this repository.

`oracle/run.sh` builds upstream's CLI, pinned in `UPSTREAM.lock`, and compares both
output formats of the two CLIs on all those cases, apart from paths, versions, times and
the keys this CLI adds. It needs Node 20.19.4 on `PATH`.

## License

[CC0 1.0](LICENSE), like upstream.
