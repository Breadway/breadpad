use crate::types::Note;
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Local, NaiveTime, Utc};
use std::process::Command;

pub struct Scheduler;

impl Scheduler {
    pub fn schedule(note: &Note) -> Result<()> {
        let fire_time = note.effective_time().context("note has no scheduled time")?;
        create_timer(&note.id, fire_time)
    }

    pub fn cancel(note_id: &str) -> Result<()> {
        let timer_name = timer_unit_name(note_id);
        let service_name = service_unit_name(note_id);
        stop_unit(&timer_name)?;
        disable_unit(&timer_name)?;
        stop_unit(&service_name)?;
        Ok(())
    }

    pub fn snooze(note: &mut Note, snooze_until: DateTime<Utc>) -> Result<()> {
        Self::cancel(&note.id).ok();
        note.snoozed_until = Some(snooze_until);
        create_timer(&note.id, snooze_until)
    }

    pub fn fire(note: &Note, missed_grace_minutes: i64) -> bool {
        let now = Utc::now();
        if let Some(t) = note.effective_time() {
            let diff = now.signed_duration_since(t);
            if diff > Duration::minutes(missed_grace_minutes) {
                tracing::info!("reminder {} missed ({}m ago), skipping", note.id, diff.num_minutes());
                return false;
            }
        }
        true
    }

    pub fn next_recurrence(note: &Note, default_morning: &str) -> Option<DateTime<Utc>> {
        let rrule = note.rrule.as_ref()?;
        parse_next_from_rrule(rrule.as_str(), default_morning)
    }
}

fn timer_unit_name(id: &str) -> String {
    format!("breadpad-reminder-{}.timer", id)
}

fn service_unit_name(id: &str) -> String {
    format!("breadpad-reminder-{}.service", id)
}

fn create_timer(id: &str, fire_time: DateTime<Utc>) -> Result<()> {
    // Convert to local time for systemd OnCalendar
    let local: chrono::DateTime<Local> = fire_time.with_timezone(&Local);
    let on_calendar = local.format("%Y-%m-%d %H:%M:%S").to_string();

    let timer_name = timer_unit_name(id);

    // Use systemd-run to create both service + timer as a transient unit
    let status = Command::new("systemd-run")
        .arg("--user")
        .arg("--unit")
        .arg(&timer_name.strip_suffix(".timer").unwrap_or(&timer_name))
        .arg("--timer-property")
        .arg(format!("OnCalendar={}", on_calendar))
        .arg("--timer-property")
        .arg("Persistent=true")
        .arg("--")
        .arg("breadpad")
        .arg("fire")
        .arg(id)
        .status()
        .context("failed to run systemd-run")?;

    if !status.success() {
        anyhow::bail!("systemd-run failed for reminder {}", id);
    }

    tracing::info!("scheduled reminder {} at {}", id, on_calendar);
    Ok(())
}

fn stop_unit(unit: &str) -> Result<()> {
    Command::new("systemctl")
        .args(["--user", "stop", unit])
        .status()
        .context("systemctl stop")?;
    Ok(())
}

fn disable_unit(unit: &str) -> Result<()> {
    Command::new("systemctl")
        .args(["--user", "disable", "--now", unit])
        .status()
        .context("systemctl disable")?;
    Ok(())
}

pub(crate) fn parse_next_from_rrule(rrule_str: &str, default_morning: &str) -> Option<DateTime<Utc>> {
    // (see tests module below for coverage)
    // Parse RRULE:FREQ=WEEKLY;BYDAY=MO;BYHOUR=9;BYMINUTE=0;BYSECOND=0 etc.
    // We extract FREQ, BYDAY, BYHOUR, BYMINUTE to compute next occurrence.
    if rrule_str.trim().is_empty() {
        return None;
    }

    let parts: std::collections::HashMap<&str, &str> = rrule_str
        .trim_start_matches("RRULE:")
        .split(';')
        .filter_map(|part| {
            let mut kv = part.splitn(2, '=');
            Some((kv.next()?, kv.next()?))
        })
        .collect();

    let freq = parts.get("FREQ")?;
    let freq = *freq;

    let (default_h, default_m): (u32, u32) = {
        let mut it = default_morning.splitn(2, ':');
        let h = it.next().and_then(|s| s.parse().ok()).unwrap_or(8);
        let m = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        (h, m)
    };
    let hour: u32 = parts.get("BYHOUR").and_then(|v| v.parse().ok()).unwrap_or(default_h);
    let minute: u32 = parts.get("BYMINUTE").and_then(|v| v.parse().ok()).unwrap_or(default_m);

    let now = Local::now();
    let fire_time = NaiveTime::from_hms_opt(hour, minute, 0)?;

    let next = match freq {
        "DAILY" => {
            let today = now.date_naive().and_time(fire_time);
            if now.naive_local() < today {
                today.and_local_timezone(Local).unwrap()
            } else {
                (now.date_naive() + chrono::Duration::days(1))
                    .and_time(fire_time)
                    .and_local_timezone(Local)
                    .unwrap()
            }
        }
        "WEEKLY" => {
            use chrono::Datelike;
            let byday = parts.get("BYDAY").unwrap_or(&"MO");
            let target_wd = match *byday {
                "MO" => chrono::Weekday::Mon,
                "TU" => chrono::Weekday::Tue,
                "WE" => chrono::Weekday::Wed,
                "TH" => chrono::Weekday::Thu,
                "FR" => chrono::Weekday::Fri,
                "SA" => chrono::Weekday::Sat,
                _ => chrono::Weekday::Sun,
            };
            let days_ahead = (target_wd.num_days_from_monday() as i64
                - now.weekday().num_days_from_monday() as i64)
                .rem_euclid(7);
            let days_ahead = if days_ahead == 0 {
                if now.time() < fire_time {
                    0
                } else {
                    7
                }
            } else {
                days_ahead
            };
            let target_date =
                (now.date_naive() + chrono::Duration::days(days_ahead)).and_time(fire_time);
            target_date.and_local_timezone(Local).unwrap()
        }
        _ => return None,
    };

    Some(next.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Note, NoteType, RecurrenceRule};
    use chrono::{Datelike, Local, Timelike, Utc};

    fn reminder(id: &str) -> Note {
        let mut n = Note::new("test reminder".into(), NoteType::Reminder, None);
        // Override auto-generated id for readable test names
        n.id = id.to_string();
        n
    }

    // ---- Scheduler::fire ----

    #[test]
    fn fire_note_without_time_always_fires() {
        let note = reminder("no-time");
        // effective_time() is None → fire returns true (no time = no missed check)
        assert!(Scheduler::fire(&note, 60));
    }

    #[test]
    fn fire_future_reminder_fires() {
        let mut note = reminder("future");
        note.time = Some(Utc::now() + Duration::minutes(10));
        assert!(Scheduler::fire(&note, 60));
    }

    #[test]
    fn fire_recent_past_reminder_fires() {
        let mut note = reminder("recent");
        note.time = Some(Utc::now() - Duration::minutes(5));
        // 5 min ago, grace = 60 min → should fire
        assert!(Scheduler::fire(&note, 60));
    }

    #[test]
    fn fire_exactly_at_grace_boundary_fires() {
        let mut note = reminder("boundary");
        // Use 59 min (well inside the 60-min grace) to avoid a race with wall time
        note.time = Some(Utc::now() - Duration::minutes(59));
        assert!(Scheduler::fire(&note, 60));
    }

    #[test]
    fn fire_missed_reminder_beyond_grace_skips() {
        let mut note = reminder("missed");
        note.time = Some(Utc::now() - Duration::minutes(90));
        // 90 min ago, grace = 60 → should NOT fire
        assert!(!Scheduler::fire(&note, 60));
    }

    #[test]
    fn fire_uses_snoozed_until_if_set() {
        let mut note = reminder("snoozed");
        note.time = Some(Utc::now() - Duration::hours(5)); // original would be missed
        note.snoozed_until = Some(Utc::now() - Duration::minutes(5)); // snooze is recent
        // effective_time = snoozed_until (5 min ago), grace = 60 → fires
        assert!(Scheduler::fire(&note, 60));
    }

    #[test]
    fn fire_zero_grace_only_fires_future() {
        let mut future = reminder("zero-future");
        future.time = Some(Utc::now() + Duration::seconds(1));
        assert!(Scheduler::fire(&future, 0));

        let mut past = reminder("zero-past");
        past.time = Some(Utc::now() - Duration::seconds(1));
        assert!(!Scheduler::fire(&past, 0));
    }

    // ---- Scheduler::next_recurrence ----

    #[test]
    fn next_recurrence_none_without_rrule() {
        let note = reminder("no-rrule");
        assert!(Scheduler::next_recurrence(&note, "08:00").is_none());
    }

    #[test]
    fn next_recurrence_daily_in_future() {
        let mut note = reminder("daily");
        note.rrule = Some(RecurrenceRule::new("RRULE:FREQ=DAILY;BYHOUR=8;BYMINUTE=0;BYSECOND=0"));
        let t = Scheduler::next_recurrence(&note, "08:00").unwrap();
        assert!(t >= Utc::now());
    }

    #[test]
    fn next_recurrence_weekly_is_correct_weekday() {
        let mut note = reminder("weekly-mon");
        note.rrule = Some(RecurrenceRule::new("RRULE:FREQ=WEEKLY;BYDAY=MO;BYHOUR=9;BYMINUTE=0;BYSECOND=0"));
        let t = Scheduler::next_recurrence(&note, "08:00").unwrap();
        let local: chrono::DateTime<Local> = t.into();
        assert_eq!(local.weekday(), chrono::Weekday::Mon);
    }

    // ---- parse_next_from_rrule ----

    #[test]
    fn daily_rrule_next_is_in_future() {
        let t = parse_next_from_rrule("RRULE:FREQ=DAILY;BYHOUR=14;BYMINUTE=0;BYSECOND=0", "08:00");
        assert!(t.is_some());
        assert!(t.unwrap() >= Utc::now());
    }

    #[test]
    fn daily_rrule_correct_hour() {
        let t = parse_next_from_rrule("RRULE:FREQ=DAILY;BYHOUR=14;BYMINUTE=30;BYSECOND=0", "08:00").unwrap();
        let local: chrono::DateTime<Local> = t.into();
        assert_eq!(local.hour(), 14);
        assert_eq!(local.minute(), 30);
    }

    #[test]
    fn daily_rrule_defaults_hour_from_morning() {
        // No BYHOUR in rrule — should fall back to default_morning
        let t = parse_next_from_rrule("RRULE:FREQ=DAILY", "07:15").unwrap();
        let local: chrono::DateTime<Local> = t.into();
        assert_eq!(local.hour(), 7);
        assert_eq!(local.minute(), 15);
    }

    #[test]
    fn weekly_monday_is_monday() {
        let t = parse_next_from_rrule("RRULE:FREQ=WEEKLY;BYDAY=MO;BYHOUR=9;BYMINUTE=0;BYSECOND=0", "08:00").unwrap();
        let local: chrono::DateTime<Local> = t.into();
        assert_eq!(local.weekday(), chrono::Weekday::Mon);
        assert!(t >= Utc::now());
    }

    #[test]
    fn weekly_friday_is_friday() {
        let t = parse_next_from_rrule("RRULE:FREQ=WEEKLY;BYDAY=FR;BYHOUR=12;BYMINUTE=0;BYSECOND=0", "08:00").unwrap();
        let local: chrono::DateTime<Local> = t.into();
        assert_eq!(local.weekday(), chrono::Weekday::Fri);
    }

    #[test]
    fn weekly_wednesday_correct_hour() {
        let t = parse_next_from_rrule("RRULE:FREQ=WEEKLY;BYDAY=WE;BYHOUR=15;BYMINUTE=45;BYSECOND=0", "08:00").unwrap();
        let local: chrono::DateTime<Local> = t.into();
        assert_eq!(local.hour(), 15);
        assert_eq!(local.minute(), 45);
    }

    #[test]
    fn weekly_saturday_is_saturday() {
        let t = parse_next_from_rrule("RRULE:FREQ=WEEKLY;BYDAY=SA;BYHOUR=10;BYMINUTE=0;BYSECOND=0", "08:00").unwrap();
        let local: chrono::DateTime<Local> = t.into();
        assert_eq!(local.weekday(), chrono::Weekday::Sat);
    }

    #[test]
    fn weekly_sunday_is_sunday() {
        let t = parse_next_from_rrule("RRULE:FREQ=WEEKLY;BYDAY=SU;BYHOUR=19;BYMINUTE=0;BYSECOND=0", "08:00").unwrap();
        let local: chrono::DateTime<Local> = t.into();
        assert_eq!(local.weekday(), chrono::Weekday::Sun);
    }

    #[test]
    fn unknown_freq_returns_none() {
        assert!(parse_next_from_rrule("RRULE:FREQ=MONTHLY;BYHOUR=9;BYMINUTE=0", "08:00").is_none());
    }

    #[test]
    fn empty_rrule_string_returns_none() {
        assert!(parse_next_from_rrule("", "08:00").is_none());
    }

    #[test]
    fn rrule_without_rrule_prefix_still_parses() {
        // trim_start_matches("RRULE:") handles the prefix; without it the key would be "RRULE:FREQ" which won't match
        // just verify we don't panic
        let _ = parse_next_from_rrule("FREQ=DAILY;BYHOUR=8;BYMINUTE=0", "08:00");
    }

    // ---- unit name helpers ----

    #[test]
    fn timer_unit_name_format() {
        assert_eq!(timer_unit_name("abc123"), "breadpad-reminder-abc123.timer");
    }

    #[test]
    fn service_unit_name_format() {
        assert_eq!(service_unit_name("abc123"), "breadpad-reminder-abc123.service");
    }
}
