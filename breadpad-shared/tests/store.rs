use breadpad_shared::store::Store;
use breadpad_shared::types::{Note, NoteType, RecurrenceRule};
use chrono::{Duration, Utc};
use std::fs;
use tempfile::TempDir;

fn mk() -> (TempDir, Store) {
    let dir = TempDir::new().unwrap();
    let store = Store::from_dir(dir.path()).unwrap();
    (dir, store)
}

fn note(body: &str, nt: NoteType) -> Note {
    Note::new(body.into(), nt, None)
}

// ---- Empty state ----

#[test]
fn empty_store_loads_empty_vec() {
    let (_dir, store) = mk();
    let notes = store.load_all().unwrap();
    assert!(notes.is_empty());
}

#[test]
fn empty_archive_loads_empty_vec() {
    let (_dir, store) = mk();
    let archive = store.load_archive().unwrap();
    assert!(archive.is_empty());
}

#[test]
fn get_by_id_returns_none_on_empty_store() {
    let (_dir, store) = mk();
    assert!(store.get_by_id("missing").unwrap().is_none());
}

// ---- save_note + load_all ----

#[test]
fn save_and_load_single() {
    let (_dir, store) = mk();
    let n = note("buy milk", NoteType::Todo);
    store.save_note(&n).unwrap();

    let loaded = store.load_all().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, n.id);
    assert_eq!(loaded[0].body, "buy milk");
    assert_eq!(loaded[0].note_type, NoteType::Todo);
    assert!(!loaded[0].done);
}

#[test]
fn save_three_notes_all_loaded() {
    let (_dir, store) = mk();
    let a = note("alpha", NoteType::Idea);
    let b = note("beta", NoteType::Note);
    let c = note("gamma", NoteType::Question);
    store.save_note(&a).unwrap();
    store.save_note(&b).unwrap();
    store.save_note(&c).unwrap();

    let loaded = store.load_all().unwrap();
    assert_eq!(loaded.len(), 3);
    let bodies: Vec<&str> = loaded.iter().map(|n| n.body.as_str()).collect();
    assert!(bodies.contains(&"alpha"));
    assert!(bodies.contains(&"beta"));
    assert!(bodies.contains(&"gamma"));
}

#[test]
fn saved_note_preserves_all_fields() {
    let (_dir, store) = mk();
    let mut n = Note::new("standup".into(), NoteType::Reminder, Some("2".into()));
    n.rrule = Some(RecurrenceRule::new("RRULE:FREQ=WEEKLY;BYDAY=MO"));
    n.tags = vec!["work".into()];
    let t = Utc::now();
    n.time = Some(t);
    store.save_note(&n).unwrap();

    let loaded = store.get_by_id(&n.id).unwrap().unwrap();
    assert_eq!(loaded.workspace, Some("2".into()));
    assert_eq!(loaded.rrule.unwrap().as_str(), "RRULE:FREQ=WEEKLY;BYDAY=MO");
    assert_eq!(loaded.tags, vec!["work"]);
    assert!(loaded.time.is_some());
}

// ---- update_note ----

#[test]
fn update_note_changes_body() {
    let (_dir, store) = mk();
    let n = note("original", NoteType::Note);
    store.save_note(&n).unwrap();
    let mut updated = n.clone();
    updated.body = "updated".into();
    store.update_note(&updated).unwrap();

    let loaded = store.load_all().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].body, "updated");
}

#[test]
fn update_note_changes_type() {
    let (_dir, store) = mk();
    let n = note("task", NoteType::Note);
    store.save_note(&n).unwrap();
    let mut updated = n.clone();
    updated.note_type = NoteType::Todo;
    store.update_note(&updated).unwrap();

    let loaded = store.get_by_id(&n.id).unwrap().unwrap();
    assert_eq!(loaded.note_type, NoteType::Todo);
}

#[test]
fn update_note_does_not_affect_other_notes() {
    let (_dir, store) = mk();
    let n1 = note("first", NoteType::Note);
    let n2 = note("second", NoteType::Todo);
    store.save_note(&n1).unwrap();
    store.save_note(&n2).unwrap();

    let mut updated = n1.clone();
    updated.body = "first-updated".into();
    store.update_note(&updated).unwrap();

    let second = store.get_by_id(&n2.id).unwrap().unwrap();
    assert_eq!(second.body, "second");
}

#[test]
fn update_nonexistent_id_leaves_store_intact() {
    let (_dir, store) = mk();
    let n = note("real", NoteType::Note);
    store.save_note(&n).unwrap();

    let mut ghost = n.clone();
    ghost.id = "ghost1".into();
    ghost.body = "ghost".into();
    store.update_note(&ghost).unwrap();

    let notes = store.load_all().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].body, "real");
}

// ---- mark_done via update ----

#[test]
fn mark_done_persists_through_update() {
    let (_dir, store) = mk();
    let n = note("finish task", NoteType::Todo);
    store.save_note(&n).unwrap();

    let mut done = n.clone();
    done.mark_done();
    store.update_note(&done).unwrap();

    let loaded = store.get_by_id(&n.id).unwrap().unwrap();
    assert!(loaded.done);
    assert!(loaded.completed.is_some());
}

// ---- delete_note ----

#[test]
fn delete_removes_only_target() {
    let (_dir, store) = mk();
    let keep = note("keep", NoteType::Note);
    let del = note("delete me", NoteType::Note);
    store.save_note(&keep).unwrap();
    store.save_note(&del).unwrap();

    store.delete_note(&del.id).unwrap();

    let loaded = store.load_all().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, keep.id);
}

#[test]
fn delete_all_leaves_empty_store() {
    let (_dir, store) = mk();
    let n = note("only note", NoteType::Note);
    store.save_note(&n).unwrap();
    store.delete_note(&n.id).unwrap();
    assert!(store.load_all().unwrap().is_empty());
}

#[test]
fn delete_nonexistent_id_is_noop() {
    let (_dir, store) = mk();
    let n = note("real note", NoteType::Note);
    store.save_note(&n).unwrap();
    store.delete_note("no-such-id").unwrap();
    assert_eq!(store.load_all().unwrap().len(), 1);
}

// ---- get_by_id ----

#[test]
fn get_by_id_finds_correct_note() {
    let (_dir, store) = mk();
    let a = note("alpha", NoteType::Idea);
    let b = note("beta", NoteType::Idea);
    store.save_note(&a).unwrap();
    store.save_note(&b).unwrap();

    let found = store.get_by_id(&a.id).unwrap().unwrap();
    assert_eq!(found.body, "alpha");
}

#[test]
fn get_by_id_returns_none_for_missing() {
    let (_dir, store) = mk();
    store.save_note(&note("x", NoteType::Note)).unwrap();
    assert!(store.get_by_id("nope").unwrap().is_none());
}

// ---- rotate_archive ----

#[test]
fn rotate_archive_moves_old_done_notes() {
    let (_dir, store) = mk();

    let mut old_done = note("old task", NoteType::Todo);
    old_done.done = true;
    old_done.completed = Some(Utc::now() - Duration::days(40));
    store.save_note(&old_done).unwrap();

    let mut recent_done = note("recent task", NoteType::Todo);
    recent_done.done = true;
    recent_done.completed = Some(Utc::now() - Duration::days(1));
    store.save_note(&recent_done).unwrap();

    let active = note("active task", NoteType::Todo);
    store.save_note(&active).unwrap();

    let moved = store.rotate_archive(30).unwrap();
    assert_eq!(moved, 1);

    let remaining = store.load_all().unwrap();
    assert_eq!(remaining.len(), 2);
    let remaining_ids: Vec<&str> = remaining.iter().map(|n| n.id.as_str()).collect();
    assert!(!remaining_ids.contains(&old_done.id.as_str()), "old note should be archived");
    assert!(remaining_ids.contains(&recent_done.id.as_str()));
    assert!(remaining_ids.contains(&active.id.as_str()));
}

#[test]
fn rotate_archive_writes_to_archive_file() {
    let (_dir, store) = mk();
    let mut old = note("archived task", NoteType::Todo);
    old.done = true;
    old.completed = Some(Utc::now() - Duration::days(35));
    store.save_note(&old).unwrap();

    store.rotate_archive(30).unwrap();

    let archived = store.load_archive().unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].id, old.id);
}

#[test]
fn rotate_archive_appends_to_existing_archive() {
    let (_dir, store) = mk();

    for i in 0..3u32 {
        let mut n = note(&format!("old {}", i), NoteType::Todo);
        n.done = true;
        n.completed = Some(Utc::now() - Duration::days(40));
        store.save_note(&n).unwrap();
    }

    store.rotate_archive(30).unwrap();

    // Add more old notes and rotate again
    for i in 3..5u32 {
        let mut n = note(&format!("old {}", i), NoteType::Todo);
        n.done = true;
        n.completed = Some(Utc::now() - Duration::days(40));
        store.save_note(&n).unwrap();
    }
    store.rotate_archive(30).unwrap();

    let archived = store.load_archive().unwrap();
    assert_eq!(archived.len(), 5);
}

#[test]
fn rotate_archive_zero_when_nothing_qualifies() {
    let (_dir, store) = mk();
    let n = note("active", NoteType::Note);
    store.save_note(&n).unwrap();
    assert_eq!(store.rotate_archive(30).unwrap(), 0);
    assert_eq!(store.load_all().unwrap().len(), 1);
}

#[test]
fn rotate_archive_note_just_inside_boundary_stays() {
    let (_dir, store) = mk();
    // 29 days ago — threshold is 30 — should NOT be archived
    let mut n = note("fresh enough", NoteType::Todo);
    n.done = true;
    n.completed = Some(Utc::now() - Duration::days(29));
    store.save_note(&n).unwrap();

    assert_eq!(store.rotate_archive(30).unwrap(), 0);
    assert_eq!(store.load_all().unwrap().len(), 1);
}

#[test]
fn rotate_archive_note_just_past_boundary_is_archived() {
    let (_dir, store) = mk();
    // 31 days ago — threshold is 30 — should be archived
    let mut n = note("old enough", NoteType::Todo);
    n.done = true;
    n.completed = Some(Utc::now() - Duration::days(31));
    store.save_note(&n).unwrap();

    assert_eq!(store.rotate_archive(30).unwrap(), 1);
    assert!(store.load_all().unwrap().is_empty());
    assert_eq!(store.load_archive().unwrap().len(), 1);
}

#[test]
fn rotate_archive_zero_day_threshold_archives_completed_notes() {
    let (_dir, store) = mk();
    let mut done = note("done a second ago", NoteType::Todo);
    done.done = true;
    done.completed = Some(Utc::now() - Duration::seconds(1));
    store.save_note(&done).unwrap();

    let undone = note("still active", NoteType::Todo);
    store.save_note(&undone).unwrap();

    assert_eq!(store.rotate_archive(0).unwrap(), 1);
    let remaining = store.load_all().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].body, "still active");
    assert_eq!(store.load_archive().unwrap().len(), 1);
}

#[test]
fn rotate_archive_ignores_undone_notes_no_matter_how_old() {
    let (_dir, store) = mk();
    let mut n = note("old but undone", NoteType::Todo);
    n.done = false;
    // Set created to far past but not done
    n.completed = Some(Utc::now() - Duration::days(100));
    store.save_note(&n).unwrap();
    assert_eq!(store.rotate_archive(30).unwrap(), 0);
}

// ---- Fault tolerance ----

#[test]
fn malformed_jsonl_line_is_skipped() {
    let dir = TempDir::new().unwrap();
    let notes_path = dir.path().join("notes.jsonl");

    let valid = note("valid note", NoteType::Note);
    let valid_line = serde_json::to_string(&valid).unwrap();
    fs::write(
        &notes_path,
        format!("{}\n{{not valid json}}\n{}\n", valid_line, valid_line),
    ).unwrap();

    let store = Store::from_dir(dir.path()).unwrap();
    let loaded = store.load_all().unwrap();
    // Two valid lines, one bad line skipped
    assert_eq!(loaded.len(), 2);
    assert!(loaded.iter().all(|n| n.body == "valid note"));
}

#[test]
fn blank_lines_in_jsonl_are_skipped() {
    let dir = TempDir::new().unwrap();
    let notes_path = dir.path().join("notes.jsonl");

    let n = note("hello", NoteType::Note);
    let line = serde_json::to_string(&n).unwrap();
    fs::write(&notes_path, format!("\n\n{}\n\n", line)).unwrap();

    let store = Store::from_dir(dir.path()).unwrap();
    let loaded = store.load_all().unwrap();
    assert_eq!(loaded.len(), 1);
}

// ---- Atomic write ----

#[test]
fn no_tmp_file_left_after_update() {
    let (dir, store) = mk();
    let n = note("task", NoteType::Todo);
    store.save_note(&n).unwrap();

    let mut updated = n.clone();
    updated.body = "updated".into();
    store.update_note(&updated).unwrap();

    let tmp = dir.path().join("notes.tmp");
    assert!(!tmp.exists(), ".tmp file should be renamed after write");
}

#[test]
fn update_writes_atomically_via_rename() {
    // Verify the file content is consistent after an update (no partial writes visible)
    let (_dir, store) = mk();
    for i in 0..10u32 {
        store.save_note(&note(&format!("note {}", i), NoteType::Note)).unwrap();
    }

    let first = store.load_all().unwrap()[0].clone();
    let mut updated = first.clone();
    updated.body = "modified".into();
    store.update_note(&updated).unwrap();

    let loaded = store.load_all().unwrap();
    assert_eq!(loaded.len(), 10);
    assert!(loaded.iter().any(|n| n.body == "modified"));
}
