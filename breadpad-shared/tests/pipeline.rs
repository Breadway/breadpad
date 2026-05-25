// End-to-end pipeline tests: classify → save → reload
//
// These mirror what breadpad (capture) and breadman (display) do in production.
// Both apps share the same Store path; we prove here that a note typed in the
// popup survives the classify+save step and is visible to a fresh store handle,
// exactly as breadman would see it on startup.

use breadpad_shared::classifier::Classifier;
use breadpad_shared::store::Store;
use breadpad_shared::types::{Note, NoteType};
use chrono::Timelike;
use tempfile::TempDir;

// Mirrors commit_note() in breadpad/src/main.rs.
// `user_type` is the type the user selected in the chip row (default = NoteType::Note).
fn capture(store: &Store, text: &str, user_type: NoteType) -> Note {
    let mut classifier = Classifier::load("auto", "08:00");
    let result = classifier.classify(text);

    let mut note = Note::new(text.into(), user_type.clone(), None);

    // When the user left the type at the default, let the classifier override it.
    if user_type == NoteType::from_str("note") {
        note.note_type = result.note_type;
    }
    note.time = result.time;
    note.rrule = result.rrule;
    note.body = result.body;

    store.save_note(&note).unwrap();
    note
}

fn setup() -> (TempDir, Store) {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path()).unwrap();
    (dir, store)
}

// Open a second Store handle pointing at the same directory — this simulates
// breadman reading from the path that breadpad wrote to.
fn breadman_store(dir: &TempDir) -> Store {
    Store::from_dir(dir.path()).unwrap()
}

// ---- basic round-trip ----

#[test]
fn todo_note_appears_in_store() {
    let (dir, store) = setup();
    let saved = capture(&store, "buy groceries", NoteType::from_str("note"));

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, saved.id);
    assert_eq!(notes[0].note_type, NoteType::Todo);
    assert_eq!(notes[0].body, "buy groceries");
    assert!(!notes[0].done);
}

#[test]
fn idea_note_appears_in_store() {
    let (dir, store) = setup();
    capture(&store, "what if we added dark mode", NoteType::from_str("note"));

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].note_type, NoteType::Idea);
}

#[test]
fn question_note_appears_in_store() {
    let (dir, store) = setup();
    capture(&store, "why does the cache miss on cold start?", NoteType::from_str("note"));

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].note_type, NoteType::Question);
}

#[test]
fn plain_note_appears_in_store() {
    let (dir, store) = setup();
    capture(&store, "retro went well today", NoteType::from_str("note"));

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].note_type, NoteType::Note);
}

// ---- reminder with time ----

#[test]
fn reminder_has_time_set() {
    let (dir, store) = setup();
    capture(&store, "call mum at 6pm", NoteType::from_str("note"));

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes[0].note_type, NoteType::Reminder);
    assert!(notes[0].time.is_some(), "reminder should have a scheduled time");
    let local: chrono::DateTime<chrono::Local> = notes[0].time.unwrap().into();
    assert_eq!(local.hour(), 18);
}

#[test]
fn reminder_body_has_time_stripped() {
    let (dir, store) = setup();
    capture(&store, "call mum at 6pm", NoteType::from_str("note"));

    let notes = breadman_store(&dir).load_all().unwrap();
    assert!(!notes[0].body.contains("6pm"), "time phrase should be removed from body");
    assert!(notes[0].body.contains("call mum"));
}

#[test]
fn in_duration_reminder_has_time() {
    let (dir, store) = setup();
    capture(&store, "check on the build in 30 minutes", NoteType::from_str("note"));

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes[0].note_type, NoteType::Reminder);
    assert!(notes[0].time.is_some());
}

// ---- recurring reminder ----

#[test]
fn recurring_reminder_has_rrule() {
    let (dir, store) = setup();
    capture(&store, "standup every monday at 9am", NoteType::from_str("note"));

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes[0].note_type, NoteType::Reminder);
    let rrule = notes[0].rrule.as_ref().expect("should have rrule");
    assert!(rrule.as_str().contains("FREQ=WEEKLY"));
    assert!(rrule.as_str().contains("BYDAY=MO"));
}

#[test]
fn daily_reminder_has_rrule() {
    let (dir, store) = setup();
    capture(&store, "drink water every day at 8am", NoteType::from_str("note"));

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes[0].note_type, NoteType::Reminder);
    assert!(notes[0].rrule.as_ref().unwrap().as_str().contains("FREQ=DAILY"));
}

// ---- user-forced type is respected ----

#[test]
fn user_selected_type_overrides_classifier() {
    let (dir, store) = setup();
    // Text would classify as Todo, but user explicitly chose Idea
    capture(&store, "fix the login bug", NoteType::Idea);

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes[0].note_type, NoteType::Idea, "user chip selection should win over classifier");
}

#[test]
fn user_selected_reminder_overrides_classifier() {
    let (dir, store) = setup();
    capture(&store, "team meeting notes from today", NoteType::Reminder);

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes[0].note_type, NoteType::Reminder);
}

// ---- multiple notes all appear ----

#[test]
fn three_notes_all_visible_to_breadman() {
    let (dir, store) = setup();
    capture(&store, "buy milk", NoteType::from_str("note"));
    capture(&store, "what if we rewrote in Zig", NoteType::from_str("note"));
    capture(&store, "team standup went well", NoteType::from_str("note"));

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes.len(), 3);

    let types: Vec<NoteType> = notes.iter().map(|n| n.note_type.clone()).collect();
    assert!(types.contains(&NoteType::Todo));
    assert!(types.contains(&NoteType::Idea));
    assert!(types.contains(&NoteType::Note));
}

#[test]
fn notes_written_sequentially_all_survive() {
    let (dir, store) = setup();
    let n = 10u32;
    for i in 0..n {
        capture(&store, &format!("note number {}", i), NoteType::Note);
    }

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes.len() as u32, n);
}

// ---- note fields are fully preserved ----

#[test]
fn note_id_is_stable_after_reload() {
    let (dir, store) = setup();
    let saved = capture(&store, "check the logs", NoteType::Todo);

    let notes = breadman_store(&dir).load_all().unwrap();
    assert_eq!(notes[0].id, saved.id);
}

#[test]
fn note_created_timestamp_preserved() {
    let (dir, store) = setup();
    let saved = capture(&store, "morning standup", NoteType::Note);

    let notes = breadman_store(&dir).load_all().unwrap();
    // Timestamps should be equal within 1 second (serde round-trips subsecond precision)
    let diff = (notes[0].created - saved.created).num_seconds().abs();
    assert!(diff <= 1, "created timestamp drifted by {}s", diff);
}

// ---- store isolation: two separate runs don't bleed ----

#[test]
fn separate_store_dirs_are_isolated() {
    let (dir_a, store_a) = setup();
    let (dir_b, store_b) = setup();
    capture(&store_a, "note for session A", NoteType::Note);
    capture(&store_b, "note for session B", NoteType::Note);

    let notes_a = breadman_store(&dir_a).load_all().unwrap();
    let notes_b = breadman_store(&dir_b).load_all().unwrap();
    assert_eq!(notes_a.len(), 1);
    assert_eq!(notes_b.len(), 1);
    assert_ne!(notes_a[0].id, notes_b[0].id);
}
