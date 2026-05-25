use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteType {
    Todo,
    Reminder,
    Idea,
    Note,
    Question,
    #[serde(untagged)]
    Tag(String),
}

impl NoteType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "todo" => NoteType::Todo,
            "reminder" => NoteType::Reminder,
            "idea" => NoteType::Idea,
            "note" => NoteType::Note,
            "question" => NoteType::Question,
            other => NoteType::Tag(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            NoteType::Todo => "todo",
            NoteType::Reminder => "reminder",
            NoteType::Idea => "idea",
            NoteType::Note => "note",
            NoteType::Question => "question",
            NoteType::Tag(s) => s.as_str(),
        }
    }

    pub fn all_builtin() -> &'static [&'static str] {
        &["todo", "reminder", "idea", "note", "question"]
    }
}

impl fmt::Display for NoteType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurrenceRule(pub String);

impl RecurrenceRule {
    pub fn new(rrule: impl Into<String>) -> Self {
        RecurrenceRule(rrule.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub body: String,
    #[serde(rename = "type")]
    pub note_type: NoteType,
    pub time: Option<DateTime<Utc>>,
    pub rrule: Option<RecurrenceRule>,
    pub done: bool,
    pub workspace: Option<String>,
    pub created: DateTime<Utc>,
    pub snoozed_until: Option<DateTime<Utc>>,
    pub completed: Option<DateTime<Utc>>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub caldav_uid: Option<String>,
}

impl Note {
    pub fn new(body: String, note_type: NoteType, workspace: Option<String>) -> Self {
        Note {
            id: uuid::Uuid::new_v4()
                .to_string()
                .chars()
                .take(6)
                .collect(),
            body,
            note_type,
            time: None,
            rrule: None,
            done: false,
            workspace,
            created: Utc::now(),
            snoozed_until: None,
            completed: None,
            tags: Vec::new(),
            caldav_uid: None,
        }
    }

    pub fn effective_time(&self) -> Option<DateTime<Utc>> {
        self.snoozed_until.or(self.time)
    }

    pub fn mark_done(&mut self) {
        self.done = true;
        self.completed = Some(Utc::now());
    }
}

#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub note_type: NoteType,
    pub time: Option<DateTime<Utc>>,
    pub rrule: Option<RecurrenceRule>,
    pub body: String,
    pub confidence: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    // ---- NoteType ----

    #[test]
    fn note_type_from_str_all_builtins() {
        assert_eq!(NoteType::from_str("todo"), NoteType::Todo);
        assert_eq!(NoteType::from_str("reminder"), NoteType::Reminder);
        assert_eq!(NoteType::from_str("idea"), NoteType::Idea);
        assert_eq!(NoteType::from_str("note"), NoteType::Note);
        assert_eq!(NoteType::from_str("question"), NoteType::Question);
    }

    #[test]
    fn note_type_from_str_case_insensitive() {
        assert_eq!(NoteType::from_str("TODO"), NoteType::Todo);
        assert_eq!(NoteType::from_str("Reminder"), NoteType::Reminder);
        assert_eq!(NoteType::from_str("IDEA"), NoteType::Idea);
        assert_eq!(NoteType::from_str("Note"), NoteType::Note);
        assert_eq!(NoteType::from_str("QUESTION"), NoteType::Question);
    }

    #[test]
    fn note_type_custom_tag_preserved() {
        let nt = NoteType::from_str("standup");
        assert!(matches!(nt, NoteType::Tag(ref s) if s == "standup"));
        assert_eq!(nt.as_str(), "standup");
    }

    #[test]
    fn note_type_empty_string_becomes_tag() {
        let nt = NoteType::from_str("");
        assert!(matches!(nt, NoteType::Tag(ref s) if s.is_empty()));
    }

    #[test]
    fn note_type_all_builtin_round_trip() {
        for &s in NoteType::all_builtin() {
            assert_eq!(NoteType::from_str(s).as_str(), s, "round-trip failed for '{}'", s);
        }
    }

    #[test]
    fn note_type_display_matches_as_str() {
        for &s in NoteType::all_builtin() {
            let nt = NoteType::from_str(s);
            assert_eq!(nt.to_string(), nt.as_str());
        }
        let tag = NoteType::Tag("weekly".into());
        assert_eq!(tag.to_string(), "weekly");
    }

    #[test]
    fn note_type_serializes_lowercase() {
        let json = serde_json::to_string(&NoteType::Todo).unwrap();
        assert_eq!(json, r#""todo""#);
        let json = serde_json::to_string(&NoteType::Reminder).unwrap();
        assert_eq!(json, r#""reminder""#);
    }

    #[test]
    fn note_type_tag_serializes_as_string() {
        let json = serde_json::to_string(&NoteType::Tag("meeting".into())).unwrap();
        assert_eq!(json, r#""meeting""#);
    }

    #[test]
    fn note_type_deserializes_from_string() {
        let nt: NoteType = serde_json::from_str(r#""todo""#).unwrap();
        assert_eq!(nt, NoteType::Todo);
        let nt: NoteType = serde_json::from_str(r#""question""#).unwrap();
        assert_eq!(nt, NoteType::Question);
    }

    #[test]
    fn note_type_unknown_deserializes_as_tag() {
        let nt: NoteType = serde_json::from_str(r#""standup""#).unwrap();
        assert_eq!(nt, NoteType::Tag("standup".into()));
    }

    // ---- RecurrenceRule ----

    #[test]
    fn recurrence_rule_new_stores_value() {
        let r = RecurrenceRule::new("RRULE:FREQ=DAILY");
        assert_eq!(r.as_str(), "RRULE:FREQ=DAILY");
    }

    #[test]
    fn recurrence_rule_serde_round_trip() {
        let r = RecurrenceRule::new("RRULE:FREQ=WEEKLY;BYDAY=FR");
        let json = serde_json::to_string(&r).unwrap();
        let decoded: RecurrenceRule = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.as_str(), r.as_str());
    }

    #[test]
    fn recurrence_rule_from_string_owned() {
        let s = String::from("RRULE:FREQ=MONTHLY");
        let r = RecurrenceRule::new(s);
        assert_eq!(r.as_str(), "RRULE:FREQ=MONTHLY");
    }

    // ---- Note::new ----

    #[test]
    fn note_new_defaults() {
        let note = Note::new("body text".into(), NoteType::Note, Some("3".into()));
        assert_eq!(note.body, "body text");
        assert_eq!(note.note_type, NoteType::Note);
        assert_eq!(note.workspace, Some("3".into()));
        assert!(!note.done);
        assert!(note.completed.is_none());
        assert!(note.time.is_none());
        assert!(note.rrule.is_none());
        assert!(note.snoozed_until.is_none());
        assert!(note.tags.is_empty());
    }

    #[test]
    fn note_new_without_workspace() {
        let note = Note::new("x".into(), NoteType::Idea, None);
        assert!(note.workspace.is_none());
    }

    #[test]
    fn note_id_is_six_chars() {
        for _ in 0..50 {
            let note = Note::new("x".into(), NoteType::Note, None);
            assert_eq!(note.id.len(), 6, "id '{}' is not 6 chars", note.id);
        }
    }

    #[test]
    fn note_id_is_unique() {
        let ids: Vec<String> = (0..100).map(|_| Note::new("x".into(), NoteType::Note, None).id).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
        assert_eq!(unique.len(), 100, "found duplicate IDs in 100 notes");
    }

    #[test]
    fn note_created_is_recent() {
        let before = Utc::now();
        let note = Note::new("x".into(), NoteType::Note, None);
        let after = Utc::now();
        assert!(note.created >= before && note.created <= after);
    }

    // ---- Note::mark_done ----

    #[test]
    fn note_mark_done_sets_done_and_completed() {
        let before = Utc::now();
        let mut note = Note::new("task".into(), NoteType::Todo, None);
        note.mark_done();
        let after = Utc::now();

        assert!(note.done);
        let completed = note.completed.expect("completed should be set after mark_done");
        assert!(completed >= before && completed <= after);
    }

    #[test]
    fn note_mark_done_twice_updates_timestamp() {
        let mut note = Note::new("task".into(), NoteType::Todo, None);
        note.mark_done();
        let first = note.completed.unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        note.mark_done();
        let second = note.completed.unwrap();
        assert!(second >= first);
    }

    // ---- Note::effective_time ----

    #[test]
    fn effective_time_none_when_nothing_set() {
        let note = Note::new("x".into(), NoteType::Note, None);
        assert_eq!(note.effective_time(), None);
    }

    #[test]
    fn effective_time_returns_time_when_no_snooze() {
        let mut note = Note::new("x".into(), NoteType::Reminder, None);
        let t = Utc::now() + Duration::hours(1);
        note.time = Some(t);
        assert_eq!(note.effective_time(), Some(t));
    }

    #[test]
    fn effective_time_prefers_snoozed_over_time() {
        let mut note = Note::new("x".into(), NoteType::Reminder, None);
        let original = Utc::now() + Duration::hours(1);
        let snoozed = Utc::now() + Duration::hours(2);
        note.time = Some(original);
        note.snoozed_until = Some(snoozed);
        assert_eq!(note.effective_time(), Some(snoozed));
    }

    #[test]
    fn effective_time_snoozed_without_original() {
        let mut note = Note::new("x".into(), NoteType::Reminder, None);
        let snoozed = Utc::now() + Duration::hours(3);
        note.snoozed_until = Some(snoozed);
        assert_eq!(note.effective_time(), Some(snoozed));
    }

    // ---- Note serde ----

    #[test]
    fn note_serde_round_trip_minimal() {
        let note = Note::new("buy milk".into(), NoteType::Todo, None);
        let json = serde_json::to_string(&note).unwrap();
        let decoded: Note = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, note.id);
        assert_eq!(decoded.body, note.body);
        assert_eq!(decoded.note_type, note.note_type);
        assert!(!decoded.done);
        assert!(decoded.time.is_none());
        assert!(decoded.rrule.is_none());
        assert!(decoded.tags.is_empty());
    }

    #[test]
    fn note_serde_with_rrule_and_workspace() {
        let mut note = Note::new("standup".into(), NoteType::Reminder, Some("1".into()));
        note.rrule = Some(RecurrenceRule::new("RRULE:FREQ=WEEKLY;BYDAY=MO"));
        let json = serde_json::to_string(&note).unwrap();
        let decoded: Note = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.workspace, Some("1".into()));
        assert_eq!(decoded.rrule.unwrap().as_str(), "RRULE:FREQ=WEEKLY;BYDAY=MO");
    }

    #[test]
    fn note_serde_done_with_completed() {
        let mut note = Note::new("chore".into(), NoteType::Todo, None);
        note.mark_done();
        let json = serde_json::to_string(&note).unwrap();
        let decoded: Note = serde_json::from_str(&json).unwrap();
        assert!(decoded.done);
        assert!(decoded.completed.is_some());
    }

    #[test]
    fn note_serde_with_tags() {
        let mut note = Note::new("x".into(), NoteType::Note, None);
        note.tags = vec!["work".into(), "urgent".into()];
        let json = serde_json::to_string(&note).unwrap();
        let decoded: Note = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.tags, vec!["work", "urgent"]);
    }

    #[test]
    fn note_json_uses_type_key() {
        let note = Note::new("x".into(), NoteType::Reminder, None);
        let json = serde_json::to_string(&note).unwrap();
        assert!(json.contains(r#""type":"reminder""#), "json: {}", json);
    }

    #[test]
    fn note_json_missing_tags_defaults_to_empty() {
        // Older stored notes may not have tags field
        let json = r#"{"id":"abc123","body":"test","type":"note","time":null,"rrule":null,"done":false,"workspace":null,"created":"2026-01-01T00:00:00Z","snoozed_until":null,"completed":null}"#;
        let note: Note = serde_json::from_str(json).unwrap();
        assert!(note.tags.is_empty());
    }

    #[test]
    fn note_full_jsonl_example_from_readme() {
        let line = r#"{"id":"a1b2c3","body":"Pack calculator in bag","type":"reminder","time":"2026-05-25T19:00:00Z","rrule":null,"done":false,"workspace":"1","created":"2026-05-25T18:45:00Z","snoozed_until":null,"completed":null}"#;
        let note: Note = serde_json::from_str(line).unwrap();
        assert_eq!(note.id, "a1b2c3");
        assert_eq!(note.note_type, NoteType::Reminder);
        assert_eq!(note.workspace, Some("1".into()));
        assert!(!note.done);
        assert!(note.time.is_some());
    }
}
