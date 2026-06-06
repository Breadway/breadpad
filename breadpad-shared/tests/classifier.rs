use breadpad_shared::classifier::{Classifier, ExecutionProvider};
use breadpad_shared::types::NoteType;
use chrono::Timelike;

fn cl() -> Classifier {
    Classifier::load("08:00")
}

#[test]
fn active_provider_is_valid() {
    // The active provider depends on the host: a machine with the ONNX model present and
    // a working ROCm iGPU loads `Gpu`, otherwise `Cpu`. Either is valid — but when no
    // model is available we must be on CPU (no session => no GPU EP in use).
    let c = cl();
    assert!(matches!(
        c.active_provider,
        ExecutionProvider::Cpu | ExecutionProvider::Gpu
    ));
    if !c.model_available() {
        assert!(
            matches!(c.active_provider, ExecutionProvider::Cpu),
            "no model loaded but provider was not CPU"
        );
    }
}

#[test]
fn classify_falls_back_to_rule_based() {
    let mut c = cl();
    let r = c.classify("buy milk");
    assert_eq!(r.note_type, NoteType::Todo);
    assert!(r.time.is_none());
}

#[test]
fn classify_todo_via_fallback() {
    let mut c = cl();
    assert_eq!(c.classify("fix the segfault").note_type, NoteType::Todo);
}

#[test]
fn classify_reminder_via_fallback() {
    let mut c = cl();
    let r = c.classify("call mum at 6pm");
    assert_eq!(r.note_type, NoteType::Reminder);
    assert!(r.time.is_some());
}

#[test]
fn classify_idea_via_fallback() {
    let mut c = cl();
    assert_eq!(c.classify("what if we added a calendar view").note_type, NoteType::Idea);
}

#[test]
fn classify_question_via_fallback() {
    let mut c = cl();
    assert_eq!(c.classify("why does this fail?").note_type, NoteType::Question);
}

#[test]
fn classify_note_via_fallback() {
    let mut c = cl();
    assert_eq!(c.classify("meeting went well today").note_type, NoteType::Note);
}

#[test]
fn classify_recurrence_via_fallback() {
    let mut c = cl();
    let r = c.classify("standup every monday at 9am");
    assert!(r.rrule.is_some(), "expected rrule from fallback parser");
    assert_eq!(r.note_type, NoteType::Reminder);
}

#[test]
fn classify_custom_morning_time() {
    let mut c = Classifier::load("07:15");
    let r = c.classify("sync tomorrow morning");
    let t = r.time.expect("should have a time for tomorrow morning");
    let local: chrono::DateTime<chrono::Local> = t.into();
    assert_eq!(local.hour(), 7);
    assert_eq!(local.minute(), 15);
}

#[test]
fn classify_empty_string_does_not_panic() {
    let mut c = cl();
    let _ = c.classify("");
}

#[test]
fn classify_whitespace_only_does_not_panic() {
    let mut c = cl();
    let _ = c.classify("   ");
}

#[test]
fn classify_in_duration_sets_time() {
    let mut c = cl();
    let r = c.classify("take a break in 30 minutes");
    assert!(r.time.is_some(), "should have a time for 'in 30 minutes'");
    assert_eq!(r.note_type, NoteType::Reminder);
}

#[test]
fn classify_tomorrow_sets_time() {
    let mut c = cl();
    let r = c.classify("submit the invoice tomorrow");
    assert!(r.time.is_some(), "tomorrow should produce a scheduled time");
}

#[test]
fn classify_returns_cleaned_body() {
    let mut c = cl();
    let r = c.classify("call mum at 6pm");
    assert!(r.body.contains("call mum"), "body: {}", r.body);
    assert!(!r.body.contains("6pm"), "time phrase should be stripped from body: {}", r.body);
}

#[test]
fn model_path_points_to_expected_location() {
    let c = cl();
    assert!(
        c.model_path.to_str().unwrap().contains("breadpad"),
        "model path: {:?}",
        c.model_path
    );
    assert!(c.model_path.to_str().unwrap().ends_with("classifier.onnx"));
}
