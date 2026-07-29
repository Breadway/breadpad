//! `--screenshot` CLI mode: render the named view, capture it via
//! `bread-screenshots`, then exit — driven by `bread-ecosystem`'s
//! `bread-capture` orchestrator, or run standalone for one-off captures.
//!
//! No clap here, same reasoning as breadpad: extends breadman's own
//! hand-rolled `mod args` instead of bolting on a second parser that would
//! reject its real flags (`--view`, `done`, `upcoming --plain`).
//!
//! `--screenshot <view>` doubles as the view selector — it's passed through
//! as `initial_view` (the same field `--view` already sets) rather than
//! needing a separate mechanism, since breadman already supports opening
//! directly to a named stack page.

use gtk4::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

/// Extra settle time after `map` for the first frame to actually paint
/// before grim runs — `map` fires once the surface exists, not once
/// anything has been drawn into it.
const SETTLE_DELAY: Duration = Duration::from_millis(300);

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
/// against a known-views list — an invalid name just falls through to
/// breadman's own `unwrap_or("all")` default (see `build_app_window`),
/// same as `--view` already behaves for a normal run.
pub fn dispatch(window: &gtk4::ApplicationWindow, req: ScreenshotRequest) {
    let output = req.output;
    let (width, height) = (req.width as i32, req.height as i32);
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
