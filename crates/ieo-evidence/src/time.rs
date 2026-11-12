//! UTC timestamps without external crates.

use std::time::{SystemTime, UNIX_EPOCH};

/// RFC 3339 UTC with milliseconds, e.g. `2026-11-12T11:34:57.123Z`.
pub fn rfc3339(t: SystemTime) -> String {
    let d = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    let ms = d.subsec_millis();
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, m, day) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

pub fn now() -> String {
    rfc3339(SystemTime::now())
}

// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn known_instants() {
        assert_eq!(rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        // 2026-11-12T11:34:57Z = 1794483297
        let t = UNIX_EPOCH + Duration::from_millis(1_794_483_297_250);
        assert_eq!(rfc3339(t), "2026-11-12T11:34:57.250Z");
        // leap day
        let t = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(rfc3339(t), "2000-02-29T00:00:00.000Z");
    }
}
