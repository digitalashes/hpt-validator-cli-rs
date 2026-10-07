//! `hpt-validator`: the command line of `@cmsgov/hpt-validator-cli` 1.10.8 over the
//! `hpt-validator` crate, plus `--extended`, `-` for standard input and exit codes.

use flate2::read::MultiGzDecoder;
use hpt_validator::{Code, FileType, Finding, Format, Mode, Options, VERSION};
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

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

struct Args {
    path: OsString,
    version: &'static str,
    format: Option<Format>,
    json: bool,
    max_errors: usize,
    extended: bool,
}

fn main() -> ExitCode {
    let args = match parse(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("error: {e}\n\nRun hpt-validator --help for usage.");
            return ExitCode::from(2);
        }
    };
    let Some(format) = args.format.or_else(|| format_of(&args.path)) else {
        let message = "This is not a valid file type. Files must be in a required CMS template format (.json or .csv)";
        print(&match args.json {
            true => pretty(&format!("{{\"error\":{}}}", json_str(message))) + "\n",
            false => format!("{message}\n"),
        });
        return ExitCode::from(2);
    };
    let input = match open(&args.path) {
        Ok(input) => input,
        Err(e) => {
            eprintln!("error: {}: {e}", args.path.to_string_lossy());
            return ExitCode::from(3);
        }
    };
    let file = match args.path == "-" {
        true => "-".to_string(),
        false => resolve(&args.path),
    };
    let version = args.version;
    if !args.json {
        let effective = match version {
            "2.1.0" => "effective July 1, 2024",
            "2.2.0" => "effective January 1, 2025",
            _ => "effective January 1, 2026 and enforced April 1, 2026",
        };
        print(&format!(
            "Validating file: {file}\nUsing hpt-validator version {VERSION}\n\
             Using data dictionary version {version} - {effective}\n\
             Validator run started at {}\n",
            now()
        ));
    }
    let options = Options {
        max_errors: args.max_errors,
        mode: if args.extended {
            Mode::Extended
        } else {
            Mode::Default
        },
    };
    let report = match hpt_validator::validate(input, format, version, &options) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("error: {}: {e}", args.path.to_string_lossy());
            return ExitCode::from(3);
        }
    };

    let mut out = String::new();
    if args.json {
        let list = |findings: &[Finding]| {
            let items: Vec<String> = findings.iter().map(Finding::to_json).collect();
            format!("[{}]", items.join(","))
        };
        write!(
            out,
            "{{\"file\":{},\"validatorVersion\":\"{VERSION}\",\"dataDictionaryVersion\":\"{version}\",\
             \"timestamp\":\"{}\",\"valid\":{},\"errorCount\":{},\"alertCount\":{},\"errors\":{},\"alerts\":{}",
            json_str(&file),
            now(),
            report.valid(),
            report.errors.len(),
            report.alerts.len(),
            list(&report.errors),
            list(&report.alerts),
        )
        .unwrap();
        if let Some(file_type) = report.file_type {
            let extended = list(&report.extended);
            write!(
                out,
                ",\"fileType\":\"{}\",\"extended\":{extended}",
                name(file_type)
            )
            .unwrap();
        }
        out = pretty(&(out + "}")) + "\n";
    } else {
        let color = io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        let paint = |code: u8, text: String| match color {
            true => format!("\x1b[{code}m{text}\x1b[39m\n"),
            false => text + "\n",
        };
        let count = |n: usize, what: &str| match n {
            1 => format!("1 {what} found"),
            n => format!("{n} {what}s found"),
        };
        writeln!(out, "Validator run completed at {}", now()).unwrap();
        if let Some(file_type) = report.file_type {
            writeln!(out, "File type: {}", name(file_type)).unwrap();
        }
        if report.errors.is_empty() {
            out += &paint(32, "No errors found".into());
        } else {
            out += &paint(31, count(report.errors.len(), "error"));
            out += &table(&report.errors);
        }
        // Upstream prints the alert count and table even when there are none.
        out += &paint(33, count(report.alerts.len(), "alert"));
        out += &table(&report.alerts);
        if report.file_type.is_some() {
            out += &paint(36, count(report.extended.len(), "extended finding"));
            out += &table(&report.extended);
        }
    }
    print(&out);
    ExitCode::from(if report.valid() { 0 } else { 1 })
}

/// The arguments, or a usage error. `--help` and `--version` print and exit.
fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Args, String> {
    let mut positional = Vec::new();
    let (mut format, mut json, mut max_errors, mut extended) = (None, false, 1000, false);
    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy().into_owned();
        if text == "-" || !text.starts_with('-') {
            positional.push(arg);
            continue;
        }
        let (name, inline) = match text.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_string())),
            _ => (text.as_str(), None),
        };
        let mut value = |option: &str| {
            inline
                .clone()
                .or_else(|| args.next().map(|a| a.to_string_lossy().into_owned()))
                .ok_or(format!("option '{option}' argument missing"))
        };
        let invalid = |option: &str, value: &str, why: &str| {
            format!("option '{option}' argument '{value}' is invalid. {why}")
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
            "--extended" => extended = true,
            "-f" | "--format" => {
                format = Some(match value(FORMAT)?.as_str() {
                    "csv" => Format::Csv,
                    "json" => Format::Json,
                    v => return Err(invalid(FORMAT, v, "Allowed choices are csv, json.")),
                })
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
                max_errors = error_limit(&v).map_err(|why| invalid(LIMIT, &v, why))?;
            }
            _ => return Err(format!("unknown option '{text}'")),
        }
    }
    let [path, version] = <[OsString; 2]>::try_from(positional).map_err(|p| match p.len() {
        0 => "missing required argument 'filepath'".to_string(),
        1 => "missing required argument 'version'".to_string(),
        n => format!("too many arguments. Expected 2 arguments but got {n}."),
    })?;
    let version = version.to_string_lossy();
    let Some(version) = adjust_version(&version) else {
        return Err(format!(
            "command-argument value '{version}' is invalid for argument 'version'. \
             Allowed choices are v2.1, v2.2, v3.0."
        ));
    };
    Ok(Args {
        path,
        version,
        format,
        json,
        max_errors,
        extended,
    })
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

/// Major and minor of `semver.coerce`: the first run of at most 16 digits, then `.` and
/// at most 16 digits if they follow.
fn coerce(text: &str) -> Option<(u64, u64)> {
    let b = text.as_bytes();
    let digits = |i: usize| {
        b[i.min(b.len())..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count()
    };
    let mut i = 0;
    while i < b.len() {
        let n = digits(i);
        if (1..=16).contains(&n) {
            let m = digits(i + n + 1);
            let minor = match b.get(i + n) {
                Some(b'.') if (1..=16).contains(&m) => {
                    text[i + n + 1..i + n + 1 + m].parse().ok()?
                }
                _ => 0,
            };
            return Some((text[i..i + n].parse().ok()?, minor));
        }
        i += n.max(1);
    }
    None
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

/// The format from the extension, after a `.gz` suffix is removed.
fn format_of(path: &OsStr) -> Option<Format> {
    let path = path.to_string_lossy();
    let path = Path::new(path.strip_suffix(".gz").unwrap_or(&path));
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "csv" => Some(Format::Csv),
        "json" => Some(Format::Json),
        _ => None,
    }
}

// ponytail: upstream's gunzip hands the parser 16 KiB chunks, not 64 KiB, which moves the
// position in a JSON syntax error message for a .gz file past 16 KiB; read in 16 KiB chunks
// if that matters.
fn open(path: &OsStr) -> io::Result<Box<dyn Read>> {
    let input: Box<dyn Read> = match path == "-" {
        true => Box::new(io::stdin().lock()),
        false => Box::new(File::open(path)?),
    };
    Ok(match path.to_string_lossy().ends_with(".gz") {
        true => Box::new(MultiGzDecoder::new(input)),
        false => input,
    })
}

/// Writes to standard output; a closed pipe is not an error worth reporting.
fn print(text: &str) {
    let _ = io::stdout().lock().write_all(text.as_bytes());
}

/// `path.resolve`: absolute, with `.` and `..` resolved by name.
fn resolve(path: &OsStr) -> String {
    let mut resolved = PathBuf::new();
    for part in std::path::absolute(path).unwrap_or_default().components() {
        match part {
            Component::ParentDir => drop(resolved.pop()),
            part => resolved.push(part),
        }
    }
    resolved.to_string_lossy().into_owned()
}

fn name(file_type: FileType) -> &'static str {
    match file_type {
        FileType::Json => "json",
        FileType::Tall => "tall",
        FileType::Wide => "wide",
        _ => "unknown",
    }
}

/// `new Date().toISOString()`.
fn now() -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let (secs, ms) = (t.as_secs() as i64, t.subsec_millis());
    // Days to a civil date, from Howard Hinnant's `civil_from_days`.
    let z = secs.div_euclid(86400) + 719468;
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (
        doy - (153 * mp + 2) / 5 + 1,
        if mp < 10 { mp + 3 } else { mp - 9 },
    );
    let y = yoe + z.div_euclid(146097) * 400 + (m <= 2) as i64;
    let s = secs.rem_euclid(86400);
    let (h, mi, s) = (s / 3600, s / 60 % 60, s % 60);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{ms:03}Z")
}

/// A JSON string as `JSON.stringify` writes it.
fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c < ' ' => write!(out, "\\u{:04x}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out + "\""
}

/// Compact JSON laid out as `JSON.stringify(value, null, 2)` does.
fn pretty(json: &str) -> String {
    let mut out = String::new();
    let (mut depth, mut string, mut escaped) = (0, false, false);
    let newline = |out: &mut String, depth: usize| {
        out.push('\n');
        out.push_str(&"  ".repeat(depth));
    };
    let mut chars = json.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if string {
            (string, escaped) = (escaped || c != '"', !escaped && c == '\\');
            continue;
        }
        match c {
            '"' => string = true,
            '{' | '[' if matches!(chars.peek(), Some('}' | ']')) => out.extend(chars.next()),
            '{' | '[' => {
                depth += 1;
                newline(&mut out, depth);
            }
            '}' | ']' => {
                out.pop();
                depth -= 1;
                newline(&mut out, depth);
                out.push(c);
            }
            ',' => newline(&mut out, depth),
            ':' => out.push(' '),
            _ => {}
        }
    }
    out
}

/// Node's `console.table(findings)` without colours: an `(index)` column, then one column
/// per upstream key in the order keys first appear, cells as `util.inspect` shows them.
/// Upstream's table has no `code`; Extended adds `line`.
fn table(findings: &[Finding]) -> String {
    let mut head = vec!["(index)"];
    let mut columns = vec![
        (0..findings.len())
            .map(|i| i.to_string())
            .collect::<Vec<_>>(),
    ];
    for (row, finding) in findings.iter().enumerate() {
        for (key, cell) in cells(finding) {
            let column = head.iter().position(|&h| h == key).unwrap_or_else(|| {
                head.push(key);
                columns.push(Vec::new());
                head.len() - 1
            });
            columns[column].resize(row, String::new());
            columns[column].push(cell);
        }
    }
    let widths: Vec<usize> = (head.iter().zip(&mut columns))
        .map(|(h, column)| {
            column.resize(findings.len(), String::new());
            column.iter().map(|s| width(s)).fold(width(h), usize::max)
        })
        .collect();
    let rule = |left: &str, middle: &str, right: &str| {
        let parts: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
        format!("{left}{}{right}\n", parts.join(middle))
    };
    let row = |cells: Vec<&str>| {
        let parts: Vec<String> = (cells.iter().zip(&widths))
            .map(|(cell, w)| format!("{cell}{}", " ".repeat(w - width(cell))))
            .collect();
        format!("│ {} │\n", parts.join(" │ "))
    };
    let mut out = rule("┌", "┬", "┐") + &row(head.clone()) + &rule("├", "┼", "┤");
    for i in 0..findings.len() {
        out += &row(columns.iter().map(|c| c[i].as_str()).collect());
    }
    out + &rule("└", "┴", "┘")
}

/// Columns `s` takes, by the rules of Node's `getStringWidth` in builds without ICU: wide
/// and fullwidth East Asian characters take 2, controls and combining marks 0.
// ponytail: Node's ICU builds also give 2 to emoji with Emoji_Presentation and 0 to every
// nonspacing mark and format character; port those tables if such text misaligns a table.
fn width(s: &str) -> usize {
    const WIDE: &[(u32, u32)] = &[
        (0x1100, 0x115f),
        (0x2329, 0x232a),
        (0x2e80, 0x303e),
        (0x3040, 0x3247),
        (0x3250, 0x4dbf),
        (0x4e00, 0xa4c6),
        (0xa960, 0xa97c),
        (0xac00, 0xd7a3),
        (0xf900, 0xfaff),
        (0xfe10, 0xfe19),
        (0xfe30, 0xfe6b),
        (0xff01, 0xff60),
        (0xffe0, 0xffe6),
        (0x1b000, 0x1b001),
        (0x1f200, 0x1f251),
        (0x1f300, 0x1f64f),
        (0x20000, 0x3fffd),
    ];
    const ZERO: &[(u32, u32)] = &[
        (0, 0x1f),
        (0x7f, 0x9f),
        (0x300, 0x36f),
        (0x200b, 0x200f),
        (0x20d0, 0x20ff),
        (0xfe00, 0xfe0f),
        (0xfe20, 0xfe2f),
        (0xe0100, 0xe01ef),
    ];
    let within =
        |table: &[(u32, u32)], c: char| table.iter().any(|&(a, b)| (a..=b).contains(&(c as u32)));
    s.chars()
        .map(|c| {
            if within(WIDE, c) {
                2
            } else if within(ZERO, c) {
                0
            } else {
                1
            }
        })
        .sum()
}

/// A finding's upstream keys, in upstream's order, with `util.inspect`'s rendering.
fn cells(f: &Finding) -> Vec<(&'static str, String)> {
    let mut cells = vec![("path", inspect(&f.path)), ("message", inspect(&f.message))];
    for (key, value) in [
        ("field", &f.field),
        ("columnName", &f.column_name),
        ("value", &f.value),
    ] {
        if let Some(value) = value {
            cells.push((key, inspect(value)));
        }
    }
    for (key, values) in [
        ("allowedValues", f.allowed_values),
        ("versions", f.versions),
    ] {
        if let Some(values) = values {
            // `console.table` inspects with maxArrayLength 3.
            let mut items: Vec<String> = values.iter().take(3).map(|v| inspect(v)).collect();
            match values.len().saturating_sub(3) {
                0 => {}
                1 => items.push("... 1 more item".into()),
                n => items.push(format!("... {n} more items")),
            }
            let list = if items.is_empty() {
                "[]".into()
            } else {
                format!("[ {} ]", items.join(", "))
            };
            cells.push((key, list));
        }
    }
    if let Some(column) = f.column {
        cells.push(("column", column.to_string()));
    }
    if f.code == Code::JsonInvalid {
        cells.push(("originalError", "{}".into()));
    }
    if let Some(line) = f.line {
        cells.push(("line", line.to_string()));
    }
    cells
}

/// `util.inspect` of a string: in single quotes, or double quotes or backticks when the
/// text has a single quote, with Node's escapes.
fn inspect(s: &str) -> String {
    let quote = match (
        s.contains('\''),
        s.contains('"'),
        s.contains('`') || s.contains("${"),
    ) {
        (false, _, _) | (true, true, true) => '\'',
        (true, false, _) => '"',
        (true, true, false) => '`',
    };
    let mut out = String::from(quote);
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '\'' if quote == '\'' => out.push_str("\\'"),
            c if c < ' ' || ('\u{7f}'..='\u{9f}').contains(&c) => {
                write!(out, "\\x{:02X}", c as u32).unwrap()
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}
