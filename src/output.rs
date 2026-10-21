//! What both output formats share.

use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

/// Writes to standard output; a closed pipe is not an error worth reporting.
pub fn print(text: &str) {
    let _ = io::stdout().lock().write_all(text.as_bytes());
}

/// `new Date().toISOString()`.
pub fn now() -> String {
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    iso(t.as_secs().cast_signed(), t.subsec_millis())
}

/// `toISOString()` of the time `secs` and `ms` past the Unix epoch.
#[expect(clippy::many_single_char_names, reason = "the names of Hinnant's algorithm")]
fn iso(secs: i64, ms: u32) -> String {
    // Days to a civil date, from Howard Hinnant's `civil_from_days`.
    let z = secs.div_euclid(86400) + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let y = yoe + z.div_euclid(146_097) * 400 + i64::from(m <= 2);
    let s = secs.rem_euclid(86400);
    let (h, mi, s) = (s / 3600, s / 60 % 60, s % 60);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{ms:03}Z")
}

#[cfg(test)]
mod tests {
    /// Expected from Node's `new Date(ms).toISOString()`.
    #[test]
    fn iso() {
        for (secs, ms, want) in [
            (0, 0, "1970-01-01T00:00:00.000Z"),
            (951_868_799, 999, "2000-02-29T23:59:59.999Z"),
            (4_107_542_400, 0, "2100-03-01T00:00:00.000Z"),
            (1_234_567_890, 123, "2009-02-13T23:31:30.123Z"),
            (253_402_300_799, 999, "9999-12-31T23:59:59.999Z"),
        ] {
            assert_eq!(super::iso(secs, ms), want);
        }
    }
}
