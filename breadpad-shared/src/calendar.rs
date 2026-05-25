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
        let client = reqwest::Client::builder()
            .build()
            .expect("failed to build HTTP client");
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
    let summary = escape_ical(&note.body);
    let description = escape_ical(&format!("type={}", note.note_type.as_str()));

    let mut ical = format!(
        "BEGIN:VCALENDAR\r\n\
         VERSION:2.0\r\n\
         PRODID:-//breadpad//EN\r\n\
         BEGIN:VEVENT\r\n\
         UID:{uid}\r\n\
         SUMMARY:{summary}\r\n\
         DTSTART:{dtstart}\r\n\
         DTEND:{dtstart}\r\n\
         DESCRIPTION:{description}\r\n"
    );

    if let Some(rrule) = &note.rrule {
        ical.push_str(rrule.as_str());
        ical.push_str("\r\n");
    }

    ical.push_str("END:VEVENT\r\nEND:VCALENDAR\r\n");
    ical
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
