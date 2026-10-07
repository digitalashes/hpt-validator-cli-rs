//! What both output formats share.

use hpt_validator::FileType;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

/// Writes to standard output; a closed pipe is not an error worth reporting.
pub fn print(text: &str) {
    let _ = io::stdout().lock().write_all(text.as_bytes());
}

/// The Extended file type as the output names it.
pub fn file_type(file_type: FileType) -> &'static str {
    match file_type {
        FileType::Json => "json",
        FileType::Tall => "tall",
        FileType::Wide => "wide",
        FileType::Unknown => "unknown",
    }
}

/// `new Date().toISOString()`.
pub fn now() -> String {
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
