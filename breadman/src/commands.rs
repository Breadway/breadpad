//! The Tauri command surface. Every command is a thin wrapper over
//! `breadpad-shared` (store + parser + scheduler); no note logic lives here.

use crate::state::{AppState, ErrorEntry};
use breadpad_shared::config::Config;
use breadpad_shared::parser::parse_rule_based;
use breadpad_shared::scheduler::Scheduler;
use breadpad_shared::types::{Note, NoteType, RecurrenceRule};
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn opt(s: &str) -> Option<&str> {
    let t = s.trim();
    (!t.is_empty()).then_some(t)
}

/// Editor payload: the four fields the mockup's New note / Edit note surface
/// exposes. `time_text` is free text ("tomorrow 9am", "at 7pm", an ISO
/// stamp); `rrule` is a raw `RRULE:` string.
#[derive(serde::Deserialize)]
pub struct NoteDraft {
    pub body: String,
    #[serde(default)]
    pub note_type: String,
    #[serde(default)]
    pub time_text: String,
    #[serde(default)]
    pub rrule: String,
}

/// Live classification hint for the editor. Tier 1 (rule-based) only: the
/// ONNX / Ollama tiers are never run on a keystroke.
#[derive(serde::Serialize)]
pub struct ClassifyHint {
    pub note_type: String,
    pub time: Option<chrono::DateTime<chrono::Utc>>,
    pub rrule: Option<String>,
    pub confidence: f32,
    pub tier: &'static str,
}

#[tauri::command]
pub fn get_initial_view(state: State<AppState>) -> String {
    state.initial_view.clone()
}

/// Called by the frontend once it has painted `initial_view` (see
/// `App.svelte`'s bootstrap). Only `--screenshot` mode has anything
/// listening; every other run's call just finds nothing armed.
#[tauri::command]
pub fn notify_ui_ready(state: State<AppState>) {
    state.signal_ui_ready();
}

#[tauri::command]
pub fn list_notes(state: State<AppState>) -> CmdResult<Vec<Note>> {
    state.read_store().load_all().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_archive(state: State<AppState>) -> CmdResult<Vec<Note>> {
    state.read_store().load_archive().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> Config {
    state.cfg.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_config(state: State<AppState>, cfg: Config) -> CmdResult<()> {
    cfg.save().map_err(|e| e.to_string())?;
    *state.cfg.lock().unwrap() = cfg;
    Ok(())
}

#[tauri::command]
pub fn list_errors(state: State<AppState>) -> Vec<ErrorEntry> {
    state.errors_snapshot()
}

#[tauri::command]
pub fn classify_preview(state: State<AppState>, body: String) -> ClassifyHint {
    let parsed = parse_rule_based(&body, &state.default_morning());
    ClassifyHint {
        note_type: parsed.note_type.as_str().to_string(),
        time: parsed.time,
        rrule: parsed.rrule.map(|r| r.as_str().to_string()),
        confidence: parsed.confidence,
        tier: "rule-based",
    }
}

/// Resolve the editor's free-text time field to a concrete instant by
/// running it back through the rule-based parser.
fn parse_time_text(
    text: &str,
    morning: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, String> {
    match opt(text) {
        None => Ok(None),
        Some(t) => {
            let parsed = parse_rule_based(t, morning);
            parsed
                .time
                .map(Some)
                .ok_or_else(|| format!("couldn't read a time from \"{t}\""))
        }
    }
}

#[tauri::command]
pub fn create_note(state: State<AppState>, draft: NoteDraft) -> CmdResult<Note> {
    let morning = state.default_morning();
    let parsed = parse_rule_based(&draft.body, &morning);

    let note_type = match opt(&draft.note_type) {
        Some(t) => NoteType::from_str(t),
        None => parsed.note_type.clone(),
    };
    // Parser-cleaned body on create (strips "todo:" / an inline "tomorrow 9am"),
    // matching the GTK build's open_new_note.
    let body = if parsed.body.trim().is_empty() {
        draft.body.clone()
    } else {
        parsed.body.clone()
    };

    let mut note = Note::new(body, note_type, None);
    note.time = match opt(&draft.time_text) {
        Some(_) => parse_time_text(&draft.time_text, &morning)?,
        None => parsed.time,
    };
    note.rrule = match opt(&draft.rrule) {
        Some(r) => Some(RecurrenceRule::new(r)),
        None => parsed.rrule,
    };

    state
        .write_store()
        .save_note(&note)
        .map_err(|e| e.to_string())?;
    if note.time.is_some() || note.rrule.is_some() {
        if let Err(e) = Scheduler::schedule(&note) {
            state.log_error(format!("failed to schedule note {}: {e}", note.id));
        }
    }
    Ok(note)
}

#[tauri::command]
pub fn update_note(state: State<AppState>, id: String, draft: NoteDraft) -> CmdResult<Note> {
    let morning = state.default_morning();
    let store = state.write_store();
    let mut note = store
        .get_by_id(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("note {id} not found"))?;

    note.body = draft.body.clone();
    if let Some(t) = opt(&draft.note_type) {
        note.note_type = NoteType::from_str(t);
    }
    note.time = parse_time_text(&draft.time_text, &morning)?;
    note.rrule = opt(&draft.rrule).map(RecurrenceRule::new);

    store.update_note(&note).map_err(|e| e.to_string())?;
    Scheduler::cancel(&note.id).ok();
    if !note.done && (note.time.is_some() || note.rrule.is_some()) {
        if let Err(e) = Scheduler::schedule(&note) {
            state.log_error(format!("failed to reschedule note {}: {e}", note.id));
        }
    }
    Ok(note)
}

#[tauri::command]
pub fn set_done(state: State<AppState>, id: String, done: bool) -> CmdResult<Note> {
    let store = state.write_store();
    let mut note = store
        .get_by_id(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("note {id} not found"))?;

    if done {
        note.mark_done();
    } else {
        note.done = false;
        note.completed = None;
    }
    store.update_note(&note).map_err(|e| e.to_string())?;

    if done {
        Scheduler::cancel(&note.id).ok();
    } else if note.time.is_some() || note.rrule.is_some() {
        Scheduler::schedule(&note).ok();
    }
    Ok(note)
}

#[tauri::command]
pub fn delete_note(state: State<AppState>, id: String) -> CmdResult<()> {
    state
        .write_store()
        .delete_note(&id)
        .map_err(|e| e.to_string())?;
    Scheduler::cancel(&id).ok();
    Ok(())
}
