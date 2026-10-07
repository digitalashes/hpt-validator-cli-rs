//! `hpt-validator`: the command line of `@cmsgov/hpt-validator-cli` 1.10.8 over the
//! `hpt-validator` crate, plus `--extended`, `-` for standard input and exit codes.

mod args;
mod input;
mod json;
mod output;
mod table;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args = match args::parse(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("error: {e}\n\nRun hpt-validator --help for usage.");
            return ExitCode::from(2);
        }
    };
    let path = args.path.to_string_lossy();
    let gz = path.ends_with(".gz");
    let Some(format) = args
        .format
        .or_else(|| input::format_of(path.strip_suffix(".gz").unwrap_or(&path)))
    else {
        let message = "This is not a valid file type. Files must be in a required CMS template format (.json or .csv)";
        output::print(&if args.json {
            format!("{{\n  \"error\": \"{message}\"\n}}\n")
        } else {
            format!("{message}\n")
        });
        return ExitCode::from(2);
    };
    let unreadable = |e: &dyn std::fmt::Display| {
        eprintln!("error: {path}: {e}");
        ExitCode::from(3)
    };
    let input = match input::open(&args.path, gz) {
        Ok(input) => input,
        Err(e) => return unreadable(&e),
    };
    let file = if args.path == "-" {
        "-".to_string()
    } else {
        input::resolve(&args.path)
    };
    if !args.json {
        output::print(&table::preamble(&file, args.version));
    }
    let report = match hpt_validator::validate(input, format, args.version, &args.options) {
        Ok(report) => report,
        Err(e) => return unreadable(&e),
    };
    output::print(&if args.json {
        json::render(&report, &file, args.version)
    } else {
        table::render(&report)
    });
    if report.valid() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
