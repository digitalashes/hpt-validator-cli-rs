//! `-o table`, the default: upstream's lines around `console.table` of the findings.

use crate::output::{file_type, now};
use hpt_validator::{Code, Finding, Report, VERSION};
use std::fmt::Write as _;
use std::io::{self, IsTerminal};

/// What upstream prints before it starts validating.
pub fn preamble(file: &str, version: &str) -> String {
    let effective = match version {
        "2.1.0" => "effective July 1, 2024",
        "2.2.0" => "effective January 1, 2025",
        _ => "effective January 1, 2026 and enforced April 1, 2026",
    };
    format!(
        "Validating file: {file}\nUsing hpt-validator version {VERSION}\n\
         Using data dictionary version {version} - {effective}\n\
         Validator run started at {}\n",
        now()
    )
}

/// The counts and tables of errors, alerts and, in Extended, the extended findings.
pub fn render(report: &Report) -> String {
    let mut out = String::new();
    let color = io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let paint = |out: &mut String, code: u8, text: &str| {
        if color {
            writeln!(out, "\x1b[{code}m{text}\x1b[39m").unwrap();
        } else {
            writeln!(out, "{text}").unwrap();
        }
    };
    let count = |n: usize, what: &str| match n {
        1 => format!("1 {what} found"),
        n => format!("{n} {what}s found"),
    };
    writeln!(out, "Validator run completed at {}", now()).unwrap();
    if let Some(t) = report.file_type {
        writeln!(out, "File type: {}", file_type(t)).unwrap();
    }
    if report.errors.is_empty() {
        paint(&mut out, 32, "No errors found");
    } else {
        paint(&mut out, 31, &count(report.errors.len(), "error"));
        table(&mut out, &report.errors);
    }
    // Upstream prints the alert count and table even when there are none.
    paint(&mut out, 33, &count(report.alerts.len(), "alert"));
    table(&mut out, &report.alerts);
    if report.file_type.is_some() {
        paint(
            &mut out,
            36,
            &count(report.extended.len(), "extended finding"),
        );
        table(&mut out, &report.extended);
    }
    out
}

/// Node's `console.table(findings)` without colours: an `(index)` column, then one column
/// per upstream key in the order keys first appear, cells as `util.inspect` shows them.
/// Upstream's table has no `code`; Extended adds `line`.
fn table(out: &mut String, findings: &[Finding]) {
    fn cell<'a>(row: &'a [(&str, String)], key: &str) -> &'a str {
        row.iter().find(|(k, _)| *k == key).map_or("", |(_, c)| c)
    }
    let rows: Vec<Vec<(&str, String)>> = (findings.iter().enumerate())
        .map(|(i, f)| {
            [("(index)", i.to_string())]
                .into_iter()
                .chain(cells(f))
                .collect()
        })
        .collect();
    let mut head = vec!["(index)"];
    for &(key, _) in rows.iter().flatten() {
        if !head.contains(&key) {
            head.push(key);
        }
    }
    let widths: Vec<usize> = (head.iter())
        .map(|key| {
            rows.iter()
                .map(|row| width(cell(row, key)))
                .fold(width(key), usize::max)
        })
        .collect();
    let rule = |out: &mut String, [left, middle, right]: [&str; 3]| {
        let parts: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
        writeln!(out, "{left}{}{right}", parts.join(middle)).unwrap();
    };
    let line = |out: &mut String, cells: Vec<&str>| {
        for (i, (cell, w)) in cells.iter().zip(&widths).enumerate() {
            out.push_str(if i == 0 { "│ " } else { " │ " });
            out.push_str(cell);
            (width(cell)..*w).for_each(|_| out.push(' '));
        }
        out.push_str(" │\n");
    };
    rule(out, ["┌", "┬", "┐"]);
    line(out, head.clone());
    rule(out, ["├", "┼", "┤"]);
    for row in &rows {
        line(out, head.iter().map(|key| cell(row, key)).collect());
    }
    rule(out, ["└", "┴", "┘"]);
}

/// Columns `s` takes, by the rules of Node's `getStringWidth` in builds without ICU: wide
/// and fullwidth East Asian characters take 2, controls and combining marks 0.
// ponytail: Node's ICU builds also give 2 to emoji with Emoji_Presentation and 0 to every
// nonspacing mark and format character; port those tables if such text misaligns a table.
fn width(s: &str) -> usize {
    if s.is_ascii() {
        return s.bytes().filter(|b| !b.is_ascii_control()).count();
    }
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
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
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
