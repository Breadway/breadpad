use crate::types::{ClassificationResult, NoteType, RecurrenceRule};
use crate::util::local_naive_to_utc;
use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, Timelike, Utc, Weekday};
use regex::Regex;
use std::sync::OnceLock;

struct Patterns {
    at_time: Regex,
    in_duration: Regex,
    in_duration_word: Regex,
    tomorrow: Regex,
    next_weekday: Regex,
    tonight: Regex,
    every_weekdays: Regex,
    every_weekday: Regex,
    every_week: Regex,
    every_day: Regex,
    morning_evening: Regex,
}

static PATTERNS: OnceLock<Patterns> = OnceLock::new();

fn patterns() -> &'static Patterns {
    PATTERNS.get_or_init(|| Patterns {
        at_time: Regex::new(r"(?i)\bat\s+(\d{1,2})(?::(\d{2}))?\s*(am|pm)?").unwrap(),
        in_duration: Regex::new(r"(?i)\bin\s+(\d+)\s+(second|minute|hour|day|week)s?").unwrap(),
        // Word-form durations: "in an hour", "in a couple of hours", "in half an hour"
        in_duration_word: Regex::new(
            r"(?i)\bin\s+(?:an?\s+hour|a\s+couple\s+of\s+hours?|a\s+few\s+hours?|half\s+an?\s+hour|an?\s+minutes?|a\s+couple\s+of\s+minutes?)"
        ).unwrap(),
        tomorrow: Regex::new(r"(?i)\btomorrow(?:\s+morning|\s+evening|\s+afternoon)?").unwrap(),
        next_weekday: Regex::new(r"(?i)\bnext\s+(monday|tuesday|wednesday|thursday|friday|saturday|sunday)").unwrap(),
        // "tonight" or "this evening" — maps to a fixed 21:00 anchor
        tonight: Regex::new(r"(?i)\b(?:tonight|this\s+evening)\b").unwrap(),
        // "every weekday [at H:MM|morning|afternoon|evening]" → Mon–Fri RRULE
        every_weekdays: Regex::new(
            r"(?i)\bevery\s+weekday(?:\s+(?:at\s+(\d{1,2})(?::(\d{2}))?\s*(am|pm)?|(morning|afternoon|evening)))?"
        ).unwrap(),
        every_weekday: Regex::new(r"(?i)\bevery\s+(monday|tuesday|wednesday|thursday|friday|saturday|sunday)(?:\s+at\s+(\d{1,2})(?::(\d{2}))?\s*(am|pm)?)?").unwrap(),
        // \bweek\b prevents "weekday" from being matched here
        every_week: Regex::new(r"(?i)\bevery\s+week\b(?:\s+at\s+(\d{1,2})(?::(\d{2}))?\s*(am|pm)?)?").unwrap(),
        every_day: Regex::new(r"(?i)\bevery\s+day(?:\s+at\s+(\d{1,2})(?::(\d{2}))?\s*(am|pm)?)?").unwrap(),
        // Strips stray time-of-day words after a time has been extracted.
        // Handles compound forms ("this morning") before bare words to avoid partial matches.
        morning_evening: Regex::new(
            r"(?i)\b(?:this\s+(?:morning|evening|afternoon|night)|tonight|morning|evening|afternoon|night)\b"
        ).unwrap(),
    })
}

fn parse_time_of_day(hour: &str, min: Option<&str>, ampm: Option<&str>) -> NaiveTime {
    let mut h: u32 = hour.parse().unwrap_or(9);
    let m: u32 = min.and_then(|s| s.parse().ok()).unwrap_or(0);
    if let Some(ap) = ampm {
        if ap.eq_ignore_ascii_case("pm") && h < 12 {
            h += 12;
        } else if ap.eq_ignore_ascii_case("am") && h == 12 {
            h = 0;
        }
    }
    NaiveTime::from_hms_opt(h, m, 0).unwrap_or(NaiveTime::from_hms_opt(9, 0, 0).unwrap())
}

fn weekday_from_str(s: &str) -> Weekday {
    match s.to_lowercase().as_str() {
        "monday" => Weekday::Mon,
        "tuesday" => Weekday::Tue,
        "wednesday" => Weekday::Wed,
        "thursday" => Weekday::Thu,
        "friday" => Weekday::Fri,
        "saturday" => Weekday::Sat,
        _ => Weekday::Sun,
    }
}

fn rrule_weekday(wd: Weekday) -> &'static str {
    match wd {
        Weekday::Mon => "MO",
        Weekday::Tue => "TU",
        Weekday::Wed => "WE",
        Weekday::Thu => "TH",
        Weekday::Fri => "FR",
        Weekday::Sat => "SA",
        Weekday::Sun => "SU",
    }
}

fn next_occurrence_of_weekday(wd: Weekday, time: NaiveTime) -> DateTime<Utc> {
    let local = Local::now();
    let days_ahead = (wd.num_days_from_monday() as i64
        - local.weekday().num_days_from_monday() as i64)
        .rem_euclid(7);
    let days_ahead = if days_ahead == 0 {
        if local.time() < time {
            0
        } else {
            7
        }
    } else {
        days_ahead
    };
    let target_date = local.date_naive() + Duration::days(days_ahead);
    let naive = target_date.and_time(time);
    local_naive_to_utc(naive)
}

pub fn parse_rule_based(text: &str, default_morning: &str) -> ClassificationResult {
    let p = patterns();
    let morning_time: NaiveTime = default_morning
        .split(':')
        .collect::<Vec<_>>()
        .as_slice()
        .get(..2)
        .and_then(|parts| {
            let h: u32 = parts[0].parse().ok()?;
            let m: u32 = parts[1].parse().ok()?;
            NaiveTime::from_hms_opt(h, m, 0)
        })
        .unwrap_or(NaiveTime::from_hms_opt(8, 0, 0).unwrap());

    let evening_time = NaiveTime::from_hms_opt(18, 0, 0).unwrap();

    let mut extracted_time: Option<DateTime<Utc>> = None;
    let mut rrule: Option<RecurrenceRule> = None;
    let mut cleaned = text.to_string();

    // Recurrence: every day
    if let Some(m) = p.every_day.find(text) {
        let caps = p.every_day.captures(text).unwrap();
        let t = if let (Some(h), mp, ap) = (caps.get(1), caps.get(2), caps.get(3)) {
            parse_time_of_day(h.as_str(), mp.map(|x| x.as_str()), ap.map(|x| x.as_str()))
        } else {
            morning_time
        };
        rrule = Some(RecurrenceRule::new(format!(
            "RRULE:FREQ=DAILY;BYHOUR={};BYMINUTE={};BYSECOND=0",
            t.hour(),
            t.minute()
        )));
        cleaned = cleaned.replacen(m.as_str(), "", 1).trim().to_string();
    }
    // Recurrence: every week
    else if let Some(m) = p.every_week.find(text) {
        let caps = p.every_week.captures(text).unwrap();
        let t = if let (Some(h), mp, ap) = (caps.get(1), caps.get(2), caps.get(3)) {
            parse_time_of_day(h.as_str(), mp.map(|x| x.as_str()), ap.map(|x| x.as_str()))
        } else {
            morning_time
        };
        let now = Local::now();
        let wd = rrule_weekday(now.weekday());
        rrule = Some(RecurrenceRule::new(format!(
            "RRULE:FREQ=WEEKLY;BYDAY={};BYHOUR={};BYMINUTE={};BYSECOND=0",
            wd,
            t.hour(),
            t.minute()
        )));
        cleaned = cleaned.replacen(m.as_str(), "", 1).trim().to_string();
    }
    // Recurrence: every weekday (Mon–Fri)
    else if let Some(caps) = p.every_weekdays.captures(text) {
        let t = if let Some(h) = caps.get(1) {
            parse_time_of_day(h.as_str(), caps.get(2).map(|x| x.as_str()), caps.get(3).map(|x| x.as_str()))
        } else {
            match caps.get(4).map(|x| x.as_str().to_lowercase()).as_deref() {
                Some("afternoon") => NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
                Some("evening") => evening_time,
                _ => morning_time,
            }
        };
        rrule = Some(RecurrenceRule::new(format!(
            "RRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR={};BYMINUTE={};BYSECOND=0",
            t.hour(),
            t.minute()
        )));
        let full_match = caps.get(0).unwrap().as_str();
        cleaned = cleaned.replacen(full_match, "", 1).trim().to_string();
    }
    // Recurrence: every <weekday>
    else if let Some(caps) = p.every_weekday.captures(text) {
        let wd_str = caps.get(1).unwrap().as_str();
        let wd = weekday_from_str(wd_str);
        let t = if let (Some(h), mp, ap) = (caps.get(2), caps.get(3), caps.get(4)) {
            parse_time_of_day(h.as_str(), mp.map(|x| x.as_str()), ap.map(|x| x.as_str()))
        } else {
            morning_time
        };
        rrule = Some(RecurrenceRule::new(format!(
            "RRULE:FREQ=WEEKLY;BYDAY={};BYHOUR={};BYMINUTE={};BYSECOND=0",
            rrule_weekday(wd),
            t.hour(),
            t.minute()
        )));
        extracted_time = Some(next_occurrence_of_weekday(wd, t));
        let full_match = caps.get(0).unwrap().as_str();
        cleaned = cleaned.replacen(full_match, "", 1).trim().to_string();
    }

    // One-off: at <time>
    if extracted_time.is_none() {
        if let Some(caps) = p.at_time.captures(text) {
            let t = parse_time_of_day(
                caps.get(1).unwrap().as_str(),
                caps.get(2).map(|x| x.as_str()),
                caps.get(3).map(|x| x.as_str()),
            );
            let local = Local::now();
            let naive = if local.time() < t {
                local.date_naive().and_time(t)
            } else {
                (local.date_naive() + Duration::days(1)).and_time(t)
            };
            extracted_time = Some(local_naive_to_utc(naive));
            let full_match = caps.get(0).unwrap().as_str();
            cleaned = cleaned.replacen(full_match, "", 1).trim().to_string();
        }
        // One-off: in <n> minutes/hours/days
        else if let Some(caps) = p.in_duration.captures(text) {
            let n: i64 = caps.get(1).unwrap().as_str().parse().unwrap_or(1);
            let unit = caps.get(2).unwrap().as_str().to_lowercase();
            let delta = match unit.as_str() {
                "second" => Duration::seconds(n),
                "minute" => Duration::minutes(n),
                "hour" => Duration::hours(n),
                "day" => Duration::days(n),
                "week" => Duration::weeks(n),
                _ => Duration::minutes(n),
            };
            extracted_time = Some(Utc::now() + delta);
            let full_match = caps.get(0).unwrap().as_str();
            cleaned = cleaned.replacen(full_match, "", 1).trim().to_string();
        }
        // One-off: word-form durations — "in an hour", "in a couple of hours", "in half an hour"
        else if let Some(m) = p.in_duration_word.find(text) {
            let phrase = m.as_str().to_lowercase();
            let delta = if phrase.contains("half") {
                Duration::minutes(30)
            } else if phrase.contains("couple") {
                if phrase.contains("hour") { Duration::hours(2) } else { Duration::minutes(2) }
            } else if phrase.contains("few") {
                Duration::hours(3)
            } else if phrase.contains("hour") {
                Duration::hours(1)
            } else {
                Duration::minutes(1)
            };
            extracted_time = Some(Utc::now() + delta);
            cleaned = cleaned.replacen(m.as_str(), "", 1).trim().to_string();
        }
        // One-off: tomorrow [morning/evening]
        else if let Some(m) = p.tomorrow.find(text) {
            let lower = m.as_str().to_lowercase();
            let t = if lower.contains("evening") || lower.contains("afternoon") {
                evening_time
            } else {
                morning_time
            };
            let local = Local::now();
            let target = (local.date_naive() + Duration::days(1)).and_time(t);
            extracted_time = Some(local_naive_to_utc(target));
            cleaned = cleaned.replacen(m.as_str(), "", 1).trim().to_string();
        }
        // One-off: next <weekday>
        else if let Some(caps) = p.next_weekday.captures(text) {
            let wd = weekday_from_str(caps.get(1).unwrap().as_str());
            extracted_time = Some(next_occurrence_of_weekday(wd, morning_time));
            let full_match = caps.get(0).unwrap().as_str();
            cleaned = cleaned.replacen(full_match, "", 1).trim().to_string();
        }
        // One-off: "tonight" / "this evening" — anchors to 21:00
        else if let Some(m) = p.tonight.find(text) {
            let local = Local::now();
            let anchor = NaiveTime::from_hms_opt(21, 0, 0).unwrap();
            let target = if local.time() < anchor {
                local.date_naive().and_time(anchor)
            } else {
                (local.date_naive() + Duration::days(1)).and_time(anchor)
            };
            extracted_time = Some(local_naive_to_utc(target));
            cleaned = cleaned.replacen(m.as_str(), "", 1).trim().to_string();
        }
    }

    // Strip stray time-of-day words once any time or recurrence signal was found
    if extracted_time.is_some() || rrule.is_some() {
        cleaned = p
            .morning_evening
            .replace_all(&cleaned, "")
            .trim()
            .to_string();
    }

    // Infer note type
    let note_type = infer_type(text, extracted_time.is_some(), rrule.is_some());

    // Trim artifacts
    cleaned = cleaned
        .trim_matches(|c: char| c.is_whitespace() || c == ',')
        .to_string();
    if cleaned.is_empty() {
        cleaned = text.to_string();
    }

    // Calibrated confidence: high when structural signals drove the decision,
    // low when we fell back to "note" with no positive evidence.
    let confidence = if rrule.is_some() || extracted_time.is_some() {
        0.95 // time/recurrence extraction is deterministic
    } else {
        match &note_type {
            NoteType::Todo | NoteType::Question => 0.88, // strong lexical anchors
            NoteType::Idea => 0.84,
            NoteType::Reminder => 0.95, // shouldn't reach here without time
            NoteType::Note => 0.45,     // no signal — Tier 2 should weigh in
            _ => 0.60,
        }
    };

    ClassificationResult {
        note_type,
        time: extracted_time,
        rrule,
        body: cleaned,
        confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Local, Timelike, Utc};

    fn p(text: &str) -> ClassificationResult {
        parse_rule_based(text, "08:00")
    }

    fn p_morning(text: &str, morning: &str) -> ClassificationResult {
        parse_rule_based(text, morning)
    }

    // ---- NoteType inference ----

    #[test]
    fn todo_buy() {
        assert_eq!(p("buy groceries").note_type, NoteType::Todo);
    }

    #[test]
    fn todo_pick_up() {
        assert_eq!(p("pick up dry cleaning").note_type, NoteType::Todo);
    }

    #[test]
    fn todo_fix() {
        assert_eq!(p("fix the broken test").note_type, NoteType::Todo);
    }

    #[test]
    fn todo_check() {
        assert_eq!(p("check the deploy status").note_type, NoteType::Todo);
    }

    #[test]
    fn todo_call() {
        assert_eq!(p("call the dentist").note_type, NoteType::Todo);
    }

    #[test]
    fn todo_finish() {
        assert_eq!(p("finish the PR description").note_type, NoteType::Todo);
    }

    #[test]
    fn todo_write() {
        assert_eq!(p("write release notes").note_type, NoteType::Todo);
    }

    #[test]
    fn todo_update() {
        assert_eq!(p("update breadman dependencies").note_type, NoteType::Todo);
    }

    #[test]
    fn question_why() {
        assert_eq!(p("why does nmcli drop on suspend").note_type, NoteType::Question);
    }

    #[test]
    fn question_how() {
        assert_eq!(p("how do I configure zbus async").note_type, NoteType::Question);
    }

    #[test]
    fn question_what_prefix() {
        assert_eq!(p("what is the difference between Arc and Rc").note_type, NoteType::Question);
    }

    #[test]
    fn question_ends_with_mark() {
        assert_eq!(p("is this thread safe?").note_type, NoteType::Question);
        assert_eq!(p("does GTK4 run on Wayland?").note_type, NoteType::Question);
    }

    #[test]
    fn idea_what_if() {
        assert_eq!(p("what if breadman had a calendar view").note_type, NoteType::Idea);
    }

    #[test]
    fn idea_prefix() {
        assert_eq!(p("idea: reactive state module in Lua").note_type, NoteType::Idea);
    }

    #[test]
    fn idea_maybe() {
        assert_eq!(p("maybe we could cache the ONNX model").note_type, NoteType::Idea);
    }

    #[test]
    fn idea_could() {
        assert_eq!(p("the sidebar could show counts per type").note_type, NoteType::Idea);
    }

    #[test]
    fn note_generic_observation() {
        assert_eq!(p("meeting went well").note_type, NoteType::Note);
        assert_eq!(p("the new keyboard feels great").note_type, NoteType::Note);
    }

    // ---- Time extraction: at <time> ----

    #[test]
    fn at_time_pm_is_reminder_type() {
        assert_eq!(p("pack bag at 7pm").note_type, NoteType::Reminder);
    }

    #[test]
    fn at_time_am_is_reminder_type() {
        assert_eq!(p("standup at 9am").note_type, NoteType::Reminder);
    }

    #[test]
    fn at_time_pm_correct_hour() {
        let r = p("dinner reservation at 7pm");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.hour(), 19);
    }

    #[test]
    fn at_time_am_correct_hour() {
        let r = p("meeting at 10am");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.hour(), 10);
    }

    #[test]
    fn at_time_noon() {
        let r = p("lunch at 12pm");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.hour(), 12);
    }

    #[test]
    fn at_time_with_minutes() {
        let r = p("call mum at 6:30pm");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.hour(), 18);
        assert_eq!(local.minute(), 30);
    }

    #[test]
    fn at_time_no_ampm_bare_number() {
        // "at 14" should parse as 14:00
        let r = p("check logs at 14");
        assert!(r.time.is_some());
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.hour(), 14);
    }

    #[test]
    fn at_time_is_in_the_future() {
        let r = p("check servers at 11pm");
        assert!(r.time.unwrap() > Utc::now());
    }

    // ---- Time extraction: in <duration> ----

    #[test]
    fn in_30_minutes_correct_delta() {
        let before = Utc::now();
        let r = p("take a break in 30 minutes");
        let t = r.time.unwrap();
        let delta = (t - before).num_seconds();
        assert!(delta >= 29 * 60 && delta <= 31 * 60, "delta was {}s", delta);
    }

    #[test]
    fn in_1_minute() {
        let before = Utc::now();
        let r = p("ping in 1 minute");
        let delta = (r.time.unwrap() - before).num_seconds();
        assert!(delta >= 55 && delta <= 65, "delta was {}s", delta);
    }

    #[test]
    fn in_2_hours_correct_delta() {
        let before = Utc::now();
        let r = p("review PR in 2 hours");
        let delta_min = (r.time.unwrap() - before).num_minutes();
        assert!(delta_min >= 119 && delta_min <= 121, "delta was {}min", delta_min);
    }

    #[test]
    fn in_3_days_correct_delta() {
        let before = Utc::now();
        let r = p("follow up in 3 days");
        let delta_h = (r.time.unwrap() - before).num_hours();
        assert!(delta_h >= 71 && delta_h <= 73, "delta was {}h", delta_h);
    }

    // ---- Time extraction: tomorrow ----

    #[test]
    fn tomorrow_morning_correct_date_and_hour() {
        let r = p("sync tomorrow morning");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        let expected = (Local::now() + Duration::days(1)).date_naive();
        assert_eq!(local.date_naive(), expected);
        assert_eq!(local.hour(), 8);
        assert_eq!(local.minute(), 0);
    }

    #[test]
    fn tomorrow_morning_respects_custom_morning_time() {
        let r = p_morning("standup tomorrow morning", "09:30");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.hour(), 9);
        assert_eq!(local.minute(), 30);
    }

    #[test]
    fn tomorrow_evening_hour() {
        let r = p("call family tomorrow evening");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        let expected = (Local::now() + Duration::days(1)).date_naive();
        assert_eq!(local.date_naive(), expected);
        assert_eq!(local.hour(), 18);
    }

    #[test]
    fn tomorrow_alone_uses_morning_time() {
        let r = p("dentist appointment tomorrow");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.hour(), 8);
    }

    #[test]
    fn tomorrow_type_is_reminder() {
        assert_eq!(p("gym tomorrow morning").note_type, NoteType::Reminder);
    }

    // ---- Time extraction: next <weekday> ----

    #[test]
    fn next_monday_is_monday() {
        let r = p("dentist next monday");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.weekday(), chrono::Weekday::Mon);
        assert!(r.time.unwrap() > Utc::now());
    }

    #[test]
    fn next_friday_is_friday() {
        let r = p("team lunch next friday");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.weekday(), chrono::Weekday::Fri);
    }

    #[test]
    fn next_weekday_at_morning_hour() {
        let r = p("meeting next wednesday");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.hour(), 8);
    }

    // ---- Recurrence rules ----

    #[test]
    fn every_day_daily_rrule() {
        let r = p("take medication every day");
        let rule = r.rrule.expect("rrule should be set");
        assert!(rule.as_str().contains("FREQ=DAILY"), "rule: {}", rule.as_str());
    }

    #[test]
    fn every_day_type_is_reminder() {
        assert_eq!(p("take vitamin every day").note_type, NoteType::Reminder);
    }

    #[test]
    fn every_day_at_time_byhour() {
        let r = p("stand every day at 7am");
        let rule = r.rrule.unwrap();
        assert!(rule.as_str().contains("BYHOUR=7"), "rule: {}", rule.as_str());
        assert!(rule.as_str().contains("BYMINUTE=0"), "rule: {}", rule.as_str());
    }

    #[test]
    fn every_monday_weekly_with_byday() {
        let r = p("standup every monday");
        let rule = r.rrule.unwrap();
        assert!(rule.as_str().contains("FREQ=WEEKLY"), "rule: {}", rule.as_str());
        assert!(rule.as_str().contains("BYDAY=MO"), "rule: {}", rule.as_str());
    }

    #[test]
    fn every_friday_at_time_rrule() {
        let r = p("team lunch every friday at 1pm");
        let rule = r.rrule.unwrap();
        assert!(rule.as_str().contains("BYDAY=FR"), "rule: {}", rule.as_str());
        assert!(rule.as_str().contains("BYHOUR=13"), "rule: {}", rule.as_str());
    }

    #[test]
    fn every_sunday_evening_rrule() {
        let r = p("family call every sunday at 7pm");
        let rule = r.rrule.unwrap();
        assert!(rule.as_str().contains("BYDAY=SU"), "rule: {}", rule.as_str());
        assert!(rule.as_str().contains("BYHOUR=19"), "rule: {}", rule.as_str());
    }

    #[test]
    fn every_wednesday_sets_first_time() {
        let r = p("review PRs every wednesday at 10am");
        assert!(r.time.is_some(), "recurrence should set initial time");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.weekday(), chrono::Weekday::Wed);
    }

    #[test]
    fn every_week_at_time_rrule() {
        let r = p("retro every week at 4pm");
        let rule = r.rrule.unwrap();
        assert!(rule.as_str().contains("FREQ=WEEKLY"), "rule: {}", rule.as_str());
        assert!(rule.as_str().contains("BYHOUR=16"), "rule: {}", rule.as_str());
    }

    // ---- Body cleaning ----

    #[test]
    fn at_time_removed_from_body() {
        let r = p("pack calculator in bag at 7pm");
        assert!(!r.body.to_lowercase().contains("at 7pm"), "body: {:?}", r.body);
        assert!(r.body.contains("pack calculator"), "body: {:?}", r.body);
    }

    #[test]
    fn in_duration_removed_from_body() {
        let r = p("call dentist in 30 minutes");
        assert!(!r.body.contains("in 30 minutes"), "body: {:?}", r.body);
        assert!(r.body.contains("call dentist"), "body: {:?}", r.body);
    }

    #[test]
    fn every_day_removed_from_body() {
        let r = p("take vitamin every day at 8am");
        assert!(!r.body.to_lowercase().contains("every day"), "body: {:?}", r.body);
        assert!(r.body.contains("take vitamin"), "body: {:?}", r.body);
    }

    #[test]
    fn body_never_empty_after_stripping() {
        for text in &["tomorrow morning", "every day at 8am", "at 9pm", "in 5 minutes"] {
            let r = p(text);
            assert!(!r.body.is_empty(), "body was empty for '{}'", text);
        }
    }

    #[test]
    fn plain_text_body_unchanged() {
        let r = p("meeting went well, follow up needed");
        assert_eq!(r.body, "meeting went well, follow up needed");
    }

    // ---- Edge cases ----

    #[test]
    fn empty_string_does_not_panic() {
        let r = p("");
        assert!(!r.body.is_empty() || r.body.is_empty()); // just don't panic
    }

    #[test]
    fn whitespace_only_does_not_panic() {
        let _ = p("   ");
    }

    #[test]
    fn confidence_is_in_range() {
        for text in &["buy milk", "why?", "idea: tabs", "meeting done"] {
            let r = p(text);
            assert!(r.confidence >= 0.0 && r.confidence <= 1.0, "confidence {} for '{}'", r.confidence, text);
        }
    }

    #[test]
    fn buy_with_at_store_not_parsed_as_time() {
        // "at the store" should NOT parse as a time expression since there is no digit after "at"
        let r = p("buy milk at the store");
        // Should still be a Todo (no time extracted)
        assert_eq!(r.note_type, NoteType::Todo);
    }

    // ---- Word-form durations (in_duration_word) ----

    #[test]
    fn in_an_hour_sets_time() {
        let before = Utc::now();
        let r = p("check on the server in an hour");
        let delta_min = (r.time.unwrap() - before).num_minutes();
        assert!(delta_min >= 59 && delta_min <= 61, "delta was {}min", delta_min);
    }

    #[test]
    fn in_an_hour_type_is_reminder() {
        assert_eq!(p("check on the deployment in an hour").note_type, NoteType::Reminder);
    }

    #[test]
    fn in_an_hour_stripped_from_body() {
        let r = p("check on the deployment in an hour");
        assert!(!r.body.to_lowercase().contains("in an hour"), "body: {:?}", r.body);
        assert!(r.body.contains("check on the deployment"), "body: {:?}", r.body);
    }

    #[test]
    fn in_a_couple_of_hours_sets_two_hours() {
        let before = Utc::now();
        let r = p("in a couple of hours remind me to check the oven");
        let delta_min = (r.time.unwrap() - before).num_minutes();
        assert!(delta_min >= 119 && delta_min <= 121, "delta was {}min", delta_min);
    }

    #[test]
    fn in_a_couple_of_hours_is_reminder() {
        assert_eq!(p("in a couple of hours remind me to check the oven").note_type, NoteType::Reminder);
    }

    #[test]
    fn in_a_few_hours_sets_three_hours() {
        let before = Utc::now();
        let r = p("in a few hours we need to submit this");
        let delta_h = (r.time.unwrap() - before).num_hours();
        assert_eq!(delta_h, 3, "expected 3h, got {}", delta_h);
    }

    #[test]
    fn in_half_an_hour_sets_thirty_minutes() {
        let before = Utc::now();
        let r = p("in half an hour submit the report");
        let delta_min = (r.time.unwrap() - before).num_minutes();
        assert!(delta_min >= 29 && delta_min <= 31, "delta was {}min", delta_min);
    }

    // ---- Tonight / this evening ----

    #[test]
    fn tonight_type_is_reminder() {
        assert_eq!(p("tonight watch the football").note_type, NoteType::Reminder);
    }

    #[test]
    fn tonight_anchors_to_21h() {
        let r = p("tonight put the bins out");
        let local: chrono::DateTime<Local> = r.time.unwrap().into();
        assert_eq!(local.hour(), 21);
    }

    #[test]
    fn tonight_stripped_from_body() {
        let r = p("tonight put the bins out");
        assert!(!r.body.to_lowercase().contains("tonight"), "body: {:?}", r.body);
        assert!(r.body.contains("put the bins out"), "body: {:?}", r.body);
    }

    #[test]
    fn this_evening_type_is_reminder() {
        assert_eq!(p("this evening water the plants").note_type, NoteType::Reminder);
    }

    #[test]
    fn this_evening_stripped_from_body() {
        let r = p("this evening water the plants");
        assert!(!r.body.to_lowercase().contains("this evening"), "body: {:?}", r.body);
        assert!(r.body.contains("water the plants"), "body: {:?}", r.body);
    }

    // ---- Every weekday (Mon–Fri) ----

    #[test]
    fn every_weekday_is_not_matched_by_every_week() {
        let r = p("every weekday morning check email");
        let rule = r.rrule.unwrap();
        assert!(rule.as_str().contains("BYDAY=MO,TU,WE,TH,FR"), "rule: {}", rule.as_str());
    }

    #[test]
    fn every_weekday_type_is_reminder() {
        assert_eq!(p("every weekday morning check email").note_type, NoteType::Reminder);
    }

    #[test]
    fn every_weekday_morning_uses_morning_time() {
        let r = p("every weekday morning standup");
        let rule = r.rrule.unwrap();
        assert!(rule.as_str().contains("BYHOUR=8"), "rule: {}", rule.as_str());
    }

    #[test]
    fn every_weekday_at_time_uses_explicit_hour() {
        let r = p("every weekday at 9am standup");
        let rule = r.rrule.unwrap();
        assert!(rule.as_str().contains("BYDAY=MO,TU,WE,TH,FR"), "rule: {}", rule.as_str());
        assert!(rule.as_str().contains("BYHOUR=9"), "rule: {}", rule.as_str());
    }

    #[test]
    fn every_weekday_body_cleaned() {
        let r = p("every weekday morning check email");
        assert!(!r.body.to_lowercase().contains("every weekday"), "body: {:?}", r.body);
        assert!(!r.body.to_lowercase().contains("morning"), "body: {:?}", r.body);
        assert!(r.body.contains("check email"), "body: {:?}", r.body);
    }

    #[test]
    fn every_week_not_confused_by_weekday() {
        // "every weekday" should NOT produce a BYDAY equal to the current weekday
        let r = p("every weekday morning check email");
        let rule = r.rrule.unwrap();
        assert!(!rule.as_str().contains("BYDAY=MO;"), "every_week matched weekday: {}", rule.as_str());
    }

    #[test]
    fn every_week_still_works_after_fix() {
        let r = p("retro every week at 4pm");
        let rule = r.rrule.unwrap();
        assert!(rule.as_str().contains("FREQ=WEEKLY"), "rule: {}", rule.as_str());
        assert!(rule.as_str().contains("BYHOUR=16"), "rule: {}", rule.as_str());
    }
}

fn infer_type(text: &str, has_time: bool, has_rrule: bool) -> NoteType {
    let lower = text.to_lowercase();
    if has_rrule || has_time {
        return NoteType::Reminder;
    }
    if lower.contains("buy ")
        || lower.contains("pick up")
        || lower.contains("clean ")
        || lower.starts_with("call ")
        || lower.starts_with("email ")
        || lower.starts_with("fix ")
        || lower.starts_with("check ")
        || lower.starts_with("finish ")
        || lower.starts_with("write ")
        || lower.starts_with("update ")
        || lower.starts_with("prepare ")
        || lower.starts_with("schedule ")
        || lower.starts_with("organize ")
        || lower.starts_with("deploy ")
        || lower.starts_with("install ")
        || lower.starts_with("send ")
        || lower.starts_with("submit ")
        || lower.starts_with("create ")
        || lower.starts_with("setup ")
        || lower.starts_with("restore ")
        || lower.starts_with("archive ")
        || lower.starts_with("export ")
        || lower.starts_with("import ")
        || lower.starts_with("approve ")
        || lower.starts_with("configure ")
        || lower.starts_with("refactor ")
        || lower.starts_with("review ")
    {
        return NoteType::Todo;
    }
    if lower.starts_with("what if ")
        || lower.starts_with("idea:")
        || lower.contains("could ")
        || lower.contains("maybe ")
        || lower.starts_with("should we ")
    {
        return NoteType::Idea;
    }
    if lower.starts_with("why ")
        || lower.starts_with("how ")
        || (lower.starts_with("what ") && !lower.starts_with("what if "))
        || lower.starts_with("when ")
        || lower.starts_with("where ")
        || lower.starts_with("who ")
        || lower.starts_with("will ")
        || lower.starts_with("is ")
        || lower.starts_with("are ")
        || lower.starts_with("did ")
        || lower.starts_with("does ")
        || lower.ends_with('?')
    {
        return NoteType::Question;
    }
    NoteType::Note
}
