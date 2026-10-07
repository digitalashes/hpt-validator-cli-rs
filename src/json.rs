//! `-o json`: the object upstream prints with `JSON.stringify(result, null, 2)`.

use crate::output::{file_type, now};
use hpt_validator::{Finding, Report, VERSION};
use std::fmt::Write as _;

/// The result object for `file` validated against `version`.
pub fn render(report: &Report, file: &str, version: &str) -> String {
    let mut out = String::new();
    let list = |out: &mut String, findings: &[Finding]| {
        out.push('[');
        for (i, finding) in findings.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&finding.to_json());
        }
        out.push(']');
    };
    write!(
        out,
        "{{\"file\":{},\"validatorVersion\":\"{VERSION}\",\"dataDictionaryVersion\":\"{version}\",\
         \"timestamp\":\"{}\",\"valid\":{},\"errorCount\":{},\"alertCount\":{},\"errors\":",
        json_str(file),
        now(),
        report.valid(),
        report.errors.len(),
        report.alerts.len(),
    )
    .unwrap();
    list(&mut out, &report.errors);
    out.push_str(",\"alerts\":");
    list(&mut out, &report.alerts);
    if let Some(t) = report.file_type {
        write!(out, ",\"fileType\":\"{}\",\"extended\":", file_type(t)).unwrap();
        list(&mut out, &report.extended);
    }
    out.push('}');
    pretty(&out)
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

/// Compact JSON laid out as `JSON.stringify(value, null, 2)` does, and a newline.
fn pretty(json: &str) -> String {
    let mut out = String::with_capacity(json.len() * 2);
    let (mut depth, mut string, mut escaped) = (0, false, false);
    let newline = |out: &mut String, depth: usize| {
        out.push('\n');
        (0..depth).for_each(|_| out.push_str("  "));
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
    out.push('\n');
    out
}
