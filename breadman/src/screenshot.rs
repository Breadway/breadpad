//! `--screenshot` CLI mode: render the named view, capture it via
//! `bread-screenshots`, then exit — driven by `bread-ecosystem`'s
//! `bread-capture` orchestrator, or run standalone for one-off captures.
//!
//! No clap here, same reasoning as breadpad: extends breadman's own
//! hand-rolled `mod args` instead of bolting on a second parser that would
//! reject its real flags (`--view`, `done`, `upcoming --plain`).
//!
//! `--screenshot <view>` doubles as the view selector for every named stack
//! page ("all", "upcoming", "todo", ...) — it's passed through as
//! `initial_view` (the same field `--view` already sets) rather than
//! needing a separate mechanism, since breadman already supports opening
//! directly to a named stack page.
//!
//! One view isn't a stack page at all: "editor" opens the per-note editor
//! dialog (`editor::open_editor`), normally only reachable by clicking a
//! real note row's edit button. Screenshot mode calls the same builder
//! function directly against the first real note in the store (bypassing
//! the button/click-handler entirely), with no-op save/delete/error
//! callbacks since nothing here should actually persist a change.

use gtk4::prelude::*;
use libadwaita::prelude::*;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// Extra settle time after `map` for the first frame to actually paint
/// before grim runs — `map` fires once the surface exists, not once
/// anything has been drawn into it.
const SETTLE_DELAY: Duration = Duration::from_millis(300);

/// Delay before popping the editor popover open — same reasoning as every
/// other app's PRE_POPUP_DELAY: the parent window's own layout needs a beat
/// to settle first.
const PRE_POPUP_DELAY: Duration = Duration::from_millis(300);

#[derive(Clone)]
pub struct ScreenshotRequest {
    pub view: String,
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
}

/// Wire up the given view's screenshot sequence against an already-built,
/// not-yet-presented window. Every path here ends by exiting the process —
/// it never returns control to the normal note-manager UI.
///
/// Unlike the other apps' `dispatch`, this doesn't validate `req.view`
/// against a known-views list for the stack-page case — an invalid name
/// just falls through to breadman's own `unwrap_or("all")` default (see
/// `build_app_window`), same as `--view` already behaves for a normal run.
pub fn dispatch(
    window: &gtk4::ApplicationWindow,
    req: ScreenshotRequest,
    state: crate::AppState,
) {
    let output = req.output;
    let (width, height) = (req.width as i32, req.height as i32);

    if req.view == "new-note" {
        window.connect_map(move |root| {
            let output = output.clone();
            let root = root.clone();
            let state = state.clone();
            gtk4::glib::timeout_add_local_once(PRE_POPUP_DELAY, move || {
                crate::show_add_note_window(&root, state, breadpad_shared::types::NoteType::Note, move |dialog| {
                    let output = output.clone();
                    dialog.connect_map(move |_| {
                        let output = output.clone();
                        gtk4::glib::timeout_add_local_once(SETTLE_DELAY, move || {
                            finish(bread_screenshots::capture_region(0, 0, width, height, &output));
                        });
                    });
                });
            });
        });
        return;
    }

    if req.view == "editor" {
        window.connect_map(move |root| {
            let output = output.clone();
            let state = state.clone();
            let root = root.clone();
            gtk4::glib::timeout_add_local_once(PRE_POPUP_DELAY, move || {
                let Some(note) = state.notes.borrow().first().cloned() else {
                    eprintln!("breadman: no notes in the store to build the editor view from");
                    std::process::exit(1);
                };
                let morning = state.cfg.borrow().reminders.default_morning.clone();
                let store = Arc::new(state.write_store());
                // AdwDialog handles its own presentation/centering - no more
                // manual popover anchor/position/autohide juggling. Must
                // connect `map` BEFORE presenting, or the signal (which can
                // fire synchronously inside `present`) is missed entirely.
                let dialog = crate::editor::open_editor(
                    &note,
                    store,
                    morning,
                    Rc::new(|_| {}),
                    Rc::new(|| {}),
                    Rc::new(|_| {}),
                );
                let output = output.clone();
                dialog.connect_map(move |_| {
                    let output = output.clone();
                    gtk4::glib::timeout_add_local_once(SETTLE_DELAY, move || {
                        finish(bread_screenshots::capture_region(0, 0, width, height, &output));
                    });
                });
                dialog.present(Some(root.upcast_ref::<gtk4::Widget>()));
            });
        });
        return;
    }

    window.connect_map(move |_| {
        let output = output.clone();
        gtk4::glib::timeout_add_local_once(SETTLE_DELAY, move || {
            finish(bread_screenshots::capture_region(0, 0, width, height, &output));
        });
    });
}

fn finish(result: anyhow::Result<()>) {
    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("breadman: screenshot capture failed: {e}");
            std::process::exit(1);
        }
    }
}
