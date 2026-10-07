//! Reading the input: format, decompression, standard input, the path to report.

use flate2::read::MultiGzDecoder;
use hpt_validator::Format;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

/// The format from the extension.
pub fn format_of(path: &str) -> Option<Format> {
    match Path::new(path).extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "csv" => Some(Format::Csv),
        "json" => Some(Format::Json),
        _ => None,
    }
}

// ponytail: upstream's gunzip hands the parser 16 KiB chunks, not 64 KiB, which moves the
// position in a JSON syntax error message for a .gz file past 16 KiB; read in 16 KiB chunks
// if that matters.
pub fn open(path: &OsStr, gz: bool) -> io::Result<Box<dyn Read>> {
    let input: Box<dyn Read> =
        if path == "-" { Box::new(io::stdin().lock()) } else { Box::new(File::open(path)?) };
    Ok(if gz { Box::new(MultiGzDecoder::new(input)) } else { input })
}

/// `path.resolve`: absolute, with `.` and `..` resolved by name.
pub fn resolve(path: &OsStr) -> String {
    let mut resolved = PathBuf::new();
    for part in std::path::absolute(path).unwrap_or_default().components() {
        match part {
            Component::ParentDir => {
                resolved.pop();
            }
            part => resolved.push(part),
        }
    }
    resolved.to_string_lossy().into_owned()
}
