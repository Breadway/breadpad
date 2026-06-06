use chrono::{DateTime, Duration, Local, LocalResult, NaiveDateTime, TimeZone, Utc};

/// Resolve a naive *local* datetime to UTC without panicking on DST transitions.
///
/// `NaiveDateTime::and_local_timezone` (and `Local.from_local_datetime`) returns a
/// `LocalResult`, which is not always `Single`:
/// - `Single` — the normal case.
/// - `Ambiguous` (a fall-back hour that occurs twice) — pick the earliest instant.
/// - `None` (a spring-forward gap where the wall-clock time never happens) — advance
///   an hour at a time until a valid instant is found, then fall back to treating the
///   naive value as UTC.
///
/// Calling `.unwrap()` on the `None`/`Ambiguous` cases panics, which is what this helper
/// exists to avoid (it bit us on the ~2 DST transition days per year).
pub fn local_naive_to_utc(naive: NaiveDateTime) -> DateTime<Utc> {
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt.with_timezone(&Utc),
        LocalResult::Ambiguous(earliest, _latest) => earliest.with_timezone(&Utc),
        LocalResult::None => {
            let mut shifted = naive;
            for _ in 0..3 {
                shifted += Duration::hours(1);
                if let LocalResult::Single(dt) = Local.from_local_datetime(&shifted) {
                    return dt.with_timezone(&Utc);
                }
            }
            // Last resort: interpret the wall-clock value as UTC so we still return a time.
            Utc.from_utc_datetime(&naive)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn ordinary_time_round_trips() {
        let naive = NaiveDate::from_ymd_opt(2026, 6, 15)
            .unwrap()
            .and_hms_opt(9, 30, 0)
            .unwrap();
        let utc = local_naive_to_utc(naive);
        // Converting back to local should yield the same wall-clock time.
        let local: DateTime<Local> = utc.with_timezone(&Local);
        assert_eq!(local.naive_local(), naive);
    }

    #[test]
    fn never_panics_across_a_full_year_of_hours() {
        // Walk every hour of a year through the helper; it must never panic regardless
        // of the host timezone's DST rules.
        let mut dt = NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        for _ in 0..(366 * 24) {
            let _ = local_naive_to_utc(dt);
            dt += Duration::hours(1);
        }
    }
}
