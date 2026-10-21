//! Command-line arguments, read as upstream's commander setup reads them.

use crate::output::print;
use hpt_validator::{Format, Mode, Options, VERSION};
use std::ffi::OsString;

const HELP: &str = r#"Usage: hpt-validator [options] <filepath> <version>

Arguments:
  filepath                   filepath to validate, - for standard input
  version                    version of data dictionary requirements to validate against (choices: "v2.1", "v2.2", "v3.0")

Options:
  -V, --version              output the CLI version number
  -f, --format <string>      file format of file (choices: "csv", "json")
  -o, --output <string>      output format (choices: "table", "json", default: "table")
  -e, --error-limit <value>  maximum number for errors (default: 1000)
  --extended                 also report the file type and what upstream drops or never reports
  -h, --help                 display help for command

This tool validates CSV and JSON files against the CMS requirements.
- v2.1 represents the requirements effective July 1, 2024.
- v2.2 represents the requirements effective January 1, 2025.
- v3.0 represents the requirements effective January 1, 2026 and enforced April 1, 2026.
See https://github.com/CMSgov/hospital-price-transparency for information about these requirements.

Exit codes: 0 valid, 1 invalid, 2 usage error, 3 the input could not be read.
"#;

const FORMAT: &str = "-f, --format <string>";
const OUTPUT: &str = "-o, --output <string>";
const LIMIT: &str = "-e, --error-limit <value>";

/// What to validate and how to report it.
pub struct Args {
    pub path: OsString,
    pub version: &'static str,
    pub format: Option<Format>,
    pub json: bool,
    pub options: Options,
}

/// The arguments, or a usage error. `--help` and `--version` print and exit.
pub fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Args, String> {
    let mut positional = Vec::new();
    let (mut format, mut json) = (None, false);
    let mut options = Options { max_errors: 1000, mode: Mode::Default };
    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy().into_owned();
        if text == "-" || !text.starts_with('-') {
            positional.push(arg);
            continue;
        }
        let (name, mut inline) = match text.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_string())),
            _ if text.starts_with("--") => (text.as_str(), None),
            // Commander reads the rest of a short option as its value: `-e5`, and `-e=5` as `=5`.
            _ => match text.char_indices().nth(2) {
                Some((i, _)) => (&text[..i], Some(text[i..].to_string())),
                None => (text.as_str(), None),
            },
        };
        let mut value = |option: &str| {
            inline
                .take()
                .or_else(|| args.next().map(|a| a.to_string_lossy().into_owned()))
                .ok_or(format!("option '{option}' argument missing"))
        };
        match name {
            "-h" | "--help" => {
                print(HELP);
                std::process::exit(0);
            }
            "-V" | "--version" => {
                let cli = env!("CARGO_PKG_VERSION");
                print(&format!(
                    "Current CLI version: {cli}\nUsing hpt-validator version: {VERSION}\n"
                ));
                std::process::exit(0);
            }
            "--extended" => options.mode = Mode::Extended,
            "-f" | "--format" => {
                format = Some(match value(FORMAT)?.as_str() {
                    "csv" => Format::Csv,
                    "json" => Format::Json,
                    v => return Err(invalid(FORMAT, v, "Allowed choices are csv, json.")),
                });
            }
            "-o" | "--output" => {
                json = match value(OUTPUT)?.as_str() {
                    "table" => false,
                    "json" => true,
                    v => return Err(invalid(OUTPUT, v, "Allowed choices are table, json.")),
                }
            }
            "-e" | "--error-limit" => {
                let v = value(LIMIT)?;
                options.max_errors = error_limit(&v).map_err(|why| invalid(LIMIT, &v, why))?;
            }
            _ => return Err(format!("unknown option '{text}'")),
        }
    }
    // Commander 12 ignores arguments past the second.
    let mut positional = positional.into_iter();
    let path = positional.next().ok_or("missing required argument 'filepath'")?;
    let version = positional.next().ok_or("missing required argument 'version'")?;
    let version = version.to_string_lossy();
    let Some(version) = adjust_version(&version) else {
        return Err(format!(
            "command-argument value '{version}' is invalid for argument 'version'. \
             Allowed choices are v2.1, v2.2, v3.0."
        ));
    };
    Ok(Args { path, version, format, json, options })
}

/// Commander's message for an option argument it refuses.
fn invalid(option: &str, value: &str, why: &str) -> String {
    format!("option '{option}' argument '{value}' is invalid. {why}")
}

/// The CLI's `adjustVersionChoice`: with a dot, the minor version given; without, the
/// newest minor of the major. Unlike the library, `2` means 2.2.0, and 2.0 is refused.
fn adjust_version(text: &str) -> Option<&'static str> {
    match (text.contains('.'), coerce(text)?) {
        (true, (3, 0)) | (false, (3, _)) => Some("3.0.0"),
        (true, (2, 2)) | (false, (2, _)) => Some("2.2.0"),
        (true, (2, 1)) => Some("2.1.0"),
        _ => None,
    }
}

/// Major and minor of `semver.coerce`: the first run of 1–16 digits and up to two more
/// after dots, then semver's parse, which refuses leading zeros and unsafe integers.
fn coerce(text: &str) -> Option<(u64, u64)> {
    let b = text.as_bytes();
    let run =
        |i: usize| b.get(i..).map_or(0, |r| r.iter().take_while(|c| c.is_ascii_digit()).count());
    let starts = |&i: &usize| (i == 0 || !b[i - 1].is_ascii_digit()) && (1..=16).contains(&run(i));
    let start = (0..b.len()).find(starts)?;
    let mut end = start + run(start);
    let mut parts = vec![&text[start..end]];
    while parts.len() < 3 && b.get(end) == Some(&b'.') && (1..=16).contains(&run(end + 1)) {
        parts.push(&text[end + 1..end + 1 + run(end + 1)]);
        end += 1 + run(end + 1);
    }
    let number = |p: &&str| match p.parse::<u64>() {
        Ok(n) if n < 1 << 53 && (*p == "0" || !p.starts_with('0')) => Some(n),
        _ => None,
    };
    let numbers: Vec<u64> = parts.iter().map(number).collect::<Option<_>>()?;
    Some((numbers[0], numbers.get(1).copied().unwrap_or(0)))
}

/// Commander's `parseInt(value, 10)`. Upstream reads a negative limit as no limit on
/// errors and no row alerts at all; the port refuses it.
fn error_limit(text: &str) -> Result<usize, &'static str> {
    let text = text.trim_start();
    let (negative, text) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let digits = &text[..text.bytes().take_while(u8::is_ascii_digit).count()];
    if digits.is_empty() {
        return Err("Must be a number.");
    }
    if negative && digits.bytes().any(|b| b != b'0') {
        return Err("Must be 0 or more.");
    }
    Ok(digits.parse().unwrap_or(usize::MAX))
}
