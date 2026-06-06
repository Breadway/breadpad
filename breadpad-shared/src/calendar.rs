use crate::config::CalendarConfig;
use crate::types::Note;
use anyhow::{Context, Result};
use ical::IcalParser;
use std::io::BufReader;

pub struct CalDavClient {
    config: CalendarConfig,
    client: reqwest::Client,
}

pub struct CalDavEventInfo {
    pub uid: String,
    pub summary: String,
}

impl CalDavClient {
    pub fn new(config: CalendarConfig) -> Self {
        // `reqwest::Client::builder().build()` can only fail if the TLS backend can't be
        // initialised; fall back to `Client::new()` semantics rather than panicking.
        let client = reqwest::Client::builder()
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!("falling back to default HTTP client: {}", e);
                reqwest::Client::new()
            });
        CalDavClient { config, client }
    }

    pub async fn test_connection(&self) -> Result<()> {
        let body = r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:displayname/></d:prop></d:propfind>"#;
        let resp = self
            .client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
                &self.config.url,
            )
            .basic_auth(&self.config.username, Some(&self.config.password))
            .header("Depth", "0")
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body)
            .send()
            .await
            .context("CalDAV PROPFIND request failed")?;

        let status = resp.status();
        if status.is_success() || status.as_u16() == 207 {
            Ok(())
        } else {
            anyhow::bail!("CalDAV server returned {}", status);
        }
    }

    pub async fn push_event(&self, note: &Note) -> Result<String> {
        let uid = caldav_uid(note);
        let ical = build_ical(note, &uid);
        let url = event_url(&self.config.url, &uid);

        let resp = self
            .client
            .put(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .header("Content-Type", "text/calendar; charset=utf-8")
            .body(ical)
            .send()
            .await
            .context("CalDAV PUT request failed")?;

        let status = resp.status();
        if status.is_success() || status.as_u16() == 201 || status.as_u16() == 204 {
            Ok(uid)
        } else {
            anyhow::bail!("CalDAV PUT returned {}", status);
        }
    }

    pub async fn delete_event(&self, uid: &str) -> Result<()> {
        let url = event_url(&self.config.url, uid);

        let resp = self
            .client
            .delete(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await
            .context("CalDAV DELETE request failed")?;

        let status = resp.status();
        if status.is_success() || status.as_u16() == 204 || status.as_u16() == 404 {
            Ok(())
        } else {
            anyhow::bail!("CalDAV DELETE returned {}", status);
        }
    }

    pub async fn list_events(&self) -> Result<Vec<CalDavEventInfo>> {
        let body = r#"<?xml version="1.0"?>
<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">
  <d:prop>
    <d:getetag/>
    <c:calendar-data/>
  </d:prop>
  <c:filter>
    <c:comp-filter name="VCALENDAR">
      <c:comp-filter name="VEVENT"/>
    </c:comp-filter>
  </c:filter>
</c:calendar-query>"#;

        let resp = self
            .client
            .request(
                reqwest::Method::from_bytes(b"REPORT").unwrap(),
                &self.config.url,
            )
            .basic_auth(&self.config.username, Some(&self.config.password))
            .header("Depth", "1")
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body)
            .send()
            .await
            .context("CalDAV REPORT request failed")?;

        let status = resp.status();
        if !status.is_success() && status.as_u16() != 207 {
            anyhow::bail!("CalDAV REPORT returned {}", status);
        }

        let xml = resp.text().await.context("failed to read CalDAV REPORT body")?;
        parse_report_response(&xml)
    }
}

pub fn caldav_uid(note: &Note) -> String {
    note.caldav_uid
        .clone()
        .unwrap_or_else(|| format!("{}@breadpad", note.id))
}

fn event_url(base: &str, uid: &str) -> String {
    format!("{}/{}.ics", base.trim_end_matches('/'), uid)
}

fn build_ical(note: &Note, uid: &str) -> String {
    let dt = note.time.unwrap_or(note.created);
    let dtstart = dt.format("%Y%m%dT%H%M%SZ").to_string();
    let dtstamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();

    let mut lines: Vec<String> = vec![
        "BEGIN:VCALENDAR".into(),
        "VERSION:2.0".into(),
        "PRODID:-//breadpad//EN".into(),
        "BEGIN:VEVENT".into(),
        format!("UID:{}", uid),
        fold_line(&format!("SUMMARY:{}", escape_ical(&note.body))),
        format!("DTSTART:{}", dtstart),
        format!("DTEND:{}", dtstart),
        format!("DTSTAMP:{}", dtstamp),
        fold_line(&format!("DESCRIPTION:{}", escape_ical(&format!("type={}", note.note_type.as_str())))),
    ];

    if let Some(rrule) = &note.rrule {
        lines.push(rrule.as_str().to_string());
    }

    lines.push("END:VEVENT".into());
    lines.push("END:VCALENDAR".into());

    lines.join("\r\n") + "\r\n"
}

/// Fold an iCal property line per RFC 5545 §3.1: lines longer than 75 octets
/// are split with CRLF + a single space continuation character.
fn fold_line(line: &str) -> String {
    let bytes = line.as_bytes();
    if bytes.len() <= 75 {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len() + line.len() / 75 * 3);
    let mut pos = 0;
    let mut first = true;
    while pos < bytes.len() {
        if !first {
            out.push_str("\r\n ");
        }
        let limit = if first { 75 } else { 74 }; // continuation lines lose 1 octet to the space
        let mut end = (pos + limit).min(bytes.len());
        // Step back if we landed in the middle of a multi-byte UTF-8 sequence.
        while end > pos && end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
            end -= 1;
        }
        out.push_str(std::str::from_utf8(&bytes[pos..end]).unwrap_or(""));
        pos = end;
        first = false;
    }
    out
}

fn escape_ical(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

fn parse_report_response(xml: &str) -> Result<Vec<CalDavEventInfo>> {
    let mut events = Vec::new();
    let mut search_from = 0;

    while let Some(start) = xml[search_from..].find("BEGIN:VCALENDAR") {
        let abs_start = search_from + start;
        let tail = &xml[abs_start..];
        let end = match tail.find("END:VCALENDAR") {
            Some(e) => abs_start + e + "END:VCALENDAR".len(),
            None => break,
        };
        let ical_block = &xml[abs_start..end];
        events.extend(parse_ical_block(ical_block));
        search_from = end;
    }

    Ok(events)
}

fn parse_ical_block(data: &str) -> Vec<CalDavEventInfo> {
    let reader = BufReader::new(data.as_bytes());
    let parser = IcalParser::new(reader);
    let mut out = Vec::new();

    for item in parser {
        match item {
            Ok(cal) => {
                for event in cal.events {
                    let uid = event
                        .properties
                        .iter()
                        .find(|p| p.name == "UID")
                        .and_then(|p| p.value.clone())
                        .unwrap_or_default();
                    let summary = event
                        .properties
                        .iter()
                        .find(|p| p.name == "SUMMARY")
                        .and_then(|p| p.value.clone())
                        .unwrap_or_default();
                    out.push(CalDavEventInfo { uid, summary });
                }
            }
            Err(e) => tracing::warn!("CalDAV: failed to parse VCALENDAR block: {}", e),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Note, NoteType, RecurrenceRule};
    use chrono::{TimeZone, Utc};

    fn reminder(body: &str) -> Note {
        let mut n = Note::new(body.into(), NoteType::Reminder, None);
        n.time = Some(Utc::now());
        n
    }

    // ---- escape_ical ----

    #[test]
    fn escape_ical_clean_string_unchanged() {
        assert_eq!(escape_ical("hello world"), "hello world");
    }

    #[test]
    fn escape_ical_empty_string() {
        assert_eq!(escape_ical(""), "");
    }

    #[test]
    fn escape_ical_escapes_backslash() {
        assert_eq!(escape_ical("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn escape_ical_escapes_semicolon() {
        assert_eq!(escape_ical("a;b"), "a\\;b");
    }

    #[test]
    fn escape_ical_escapes_comma() {
        assert_eq!(escape_ical("apples,oranges"), "apples\\,oranges");
    }

    #[test]
    fn escape_ical_escapes_newline() {
        assert_eq!(escape_ical("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn escape_ical_multiple_special_chars() {
        assert_eq!(escape_ical("a;b,c\nd"), "a\\;b\\,c\\nd");
    }

    // ---- caldav_uid ----

    #[test]
    fn caldav_uid_uses_existing_field() {
        let mut n = Note::new("test".into(), NoteType::Reminder, None);
        n.caldav_uid = Some("my-custom-uid".into());
        assert_eq!(caldav_uid(&n), "my-custom-uid");
    }

    #[test]
    fn caldav_uid_falls_back_to_id_at_breadpad() {
        let n = Note::new("test".into(), NoteType::Reminder, None);
        assert_eq!(caldav_uid(&n), format!("{}@breadpad", n.id));
    }

    // ---- event_url ----

    #[test]
    fn event_url_with_trailing_slash() {
        let url = event_url("https://cloud.example.com/cal/", "abc@breadpad");
        assert_eq!(url, "https://cloud.example.com/cal/abc@breadpad.ics");
    }

    #[test]
    fn event_url_without_trailing_slash() {
        let url = event_url("https://cloud.example.com/cal", "abc@breadpad");
        assert_eq!(url, "https://cloud.example.com/cal/abc@breadpad.ics");
    }

    // ---- build_ical ----

    #[test]
    fn build_ical_contains_vcalendar_markers() {
        let n = reminder("team sync");
        let uid = caldav_uid(&n);
        let ical = build_ical(&n, &uid);
        assert!(ical.contains("BEGIN:VCALENDAR"), "missing BEGIN:VCALENDAR");
        assert!(ical.contains("END:VCALENDAR"), "missing END:VCALENDAR");
        assert!(ical.contains("BEGIN:VEVENT"), "missing BEGIN:VEVENT");
        assert!(ical.contains("END:VEVENT"), "missing END:VEVENT");
    }

    #[test]
    fn build_ical_contains_uid() {
        let n = reminder("team sync");
        let uid = caldav_uid(&n);
        let ical = build_ical(&n, &uid);
        assert!(ical.contains(&format!("UID:{}", uid)));
    }

    #[test]
    fn build_ical_contains_summary() {
        let n = reminder("team sync");
        let uid = caldav_uid(&n);
        let ical = build_ical(&n, &uid);
        assert!(ical.contains("SUMMARY:team sync"));
    }

    #[test]
    fn build_ical_description_contains_type() {
        let n = reminder("team sync");
        let uid = caldav_uid(&n);
        let ical = build_ical(&n, &uid);
        assert!(ical.contains("DESCRIPTION:type=reminder"));
    }

    #[test]
    fn build_ical_uses_note_time_for_dtstart() {
        let mut n = Note::new("dentist".into(), NoteType::Reminder, None);
        n.time = Some(Utc.with_ymd_and_hms(2026, 6, 15, 14, 30, 0).unwrap());
        let uid = caldav_uid(&n);
        let ical = build_ical(&n, &uid);
        assert!(ical.contains("DTSTART:20260615T143000Z"), "ical: {}", &ical[..400]);
    }

    #[test]
    fn build_ical_falls_back_to_created_when_no_time() {
        let n = Note::new("no time set".into(), NoteType::Reminder, None);
        let uid = caldav_uid(&n);
        let ical = build_ical(&n, &uid);
        assert!(ical.contains("DTSTART:"), "DTSTART should be present");
    }

    #[test]
    fn build_ical_includes_rrule_when_set() {
        let mut n = reminder("standup");
        n.rrule = Some(RecurrenceRule::new("RRULE:FREQ=WEEKLY;BYDAY=MO;BYHOUR=9;BYMINUTE=0;BYSECOND=0"));
        let uid = caldav_uid(&n);
        let ical = build_ical(&n, &uid);
        assert!(ical.contains("RRULE:FREQ=WEEKLY;BYDAY=MO"));
    }

    #[test]
    fn build_ical_no_rrule_when_not_set() {
        let n = reminder("one-off");
        let uid = caldav_uid(&n);
        let ical = build_ical(&n, &uid);
        assert!(!ical.contains("RRULE:"));
    }

    #[test]
    fn build_ical_escapes_special_chars_in_summary() {
        let n = Note::new("dentist; bring card, and ID".into(), NoteType::Reminder, None);
        let uid = caldav_uid(&n);
        let ical = build_ical(&n, &uid);
        assert!(ical.contains("SUMMARY:dentist\\; bring card\\, and ID"), "ical: {}", &ical[..400]);
    }

    #[test]
    fn build_ical_contains_dtstamp() {
        let n = reminder("team sync");
        let uid = caldav_uid(&n);
        let ical = build_ical(&n, &uid);
        assert!(ical.contains("DTSTAMP:"), "missing DTSTAMP in:\n{}", ical);
    }

    #[test]
    fn fold_line_short_unchanged() {
        let line = "SUMMARY:short";
        assert_eq!(fold_line(line), line);
    }

    #[test]
    fn fold_line_exactly_75_unchanged() {
        let line = "A".repeat(75);
        assert_eq!(fold_line(&line), line);
    }

    #[test]
    fn fold_line_76_chars_splits() {
        let line = "X".repeat(76);
        let folded = fold_line(&line);
        assert!(folded.contains("\r\n "), "expected fold in: {:?}", folded);
        // Reassembled content should equal the original.
        let rejoined: String = folded.split("\r\n ").collect();
        assert_eq!(rejoined, line);
    }

    #[test]
    fn build_ical_long_summary_is_folded() {
        let long_body = "a".repeat(200);
        let n = Note::new(long_body.clone(), NoteType::Reminder, None);
        let uid = caldav_uid(&n);
        let ical = build_ical(&n, &uid);
        // Every line (split on CRLF) must be at most 75 octets.
        for line in ical.split("\r\n") {
            assert!(
                line.len() <= 75,
                "line too long ({} octets): {:?}",
                line.len(),
                line
            );
        }
    }

    // ---- parse_report_response ----

    #[test]
    fn parse_report_response_empty_xml_returns_empty() {
        let events = parse_report_response("").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn parse_report_response_single_event() {
        let xml = "\
BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
BEGIN:VEVENT\r\n\
UID:abc123@breadpad\r\n\
SUMMARY:team sync\r\n\
DTSTART:20260615T140000Z\r\n\
DTEND:20260615T140000Z\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";
        let events = parse_report_response(xml).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].uid, "abc123@breadpad");
        assert_eq!(events[0].summary, "team sync");
    }

    #[test]
    fn parse_report_response_no_vcalendar_block_returns_empty() {
        let xml = "<multistatus><response><status>HTTP/1.1 200 OK</status></response></multistatus>";
        let events = parse_report_response(xml).unwrap();
        assert!(events.is_empty());
    }
}
