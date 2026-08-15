//! `--screenshot` CLI mode: render breadpad's compose popup, capture it via
//! `bread-screenshots`, then exit — driven by `bread-ecosystem`'s
//! `bread-capture` orchestrator, or run standalone for one-off captures.
//!
//! No clap here (unlike breadbar/breadbox/breadclip/breadsearch): breadpad
//! already has its own small hand-rolled flag parser (`mod args`) covering
//! `--type`/`--no-classify`/`--status`/`fire`/`calendar`/etc, and clap's
//! default "reject unknown flags" behavior would break every one of those
//! if bolted on as a second, separate parser. `--screenshot`/`--output`/
//! `--width`/`--height` are just three more fields on that same `Args`
//! struct instead.
//!
//! Three views: "popup" (the compose window from `run_popup`), "reminder"
//! (the alert window from `run_reminder_window`/`build_reminder_window`,
//! normally only reachable via a real due note through `fire <id>`, built
//! here against a fabricated sample `Note` instead — see `main`'s
//! `screenshot_req.view == "reminder"` branch, which skips the Store lookup
//! entirely), and "reminder-snooze" (the same window with its snooze
//! popover open).

use bread_utils::screenshot_cli::SETTLE_DELAY;
use gtk4::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

/// Delay before popping the snooze popover open — same reasoning as every
/// other app's PRE_POPUP_DELAY: the parent window's own layout needs a beat
/// to settle first.
const PRE_POPUP_DELAY: Duration = SETTLE_DELAY;

#[derive(Clone)]
pub struct ScreenshotRequest {
    pub view: String,
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
}

/// Wire up the given view's screenshot sequence against an already-built,
/// not-yet-presented window. Every path here ends by exiting the process —
/// it never returns control to the normal popup UI.
pub fn dispatch(window: &gtk4::ApplicationWindow, req: ScreenshotRequest) {
    match req.view.as_str() {
        "popup" => {
            let output = req.output;
            let (width, height) = (req.width as i32, req.height as i32);
            window.connect_map(move |_| {
                let output = output.clone();
                gtk4::glib::timeout_add_local_once(SETTLE_DELAY, move || {
                    finish(bread_screenshots::capture_region(0, 0, width, height, &output));
                });
            });
        }
        other => {
            eprintln!("breadpad: unknown screenshot view '{other}' (known: popup, reminder, reminder-snooze)");
            std::process::exit(1);
        }
    }
}

/// Same shape as `dispatch`'s "popup" arm, for the reminder window itself
/// (view "reminder") — pulled out since `build_reminder_window` calls this
/// directly rather than going through `dispatch` (the reminder window is
/// built via a completely separate `run_reminder_window` entry point, not
/// `run_popup`'s).
pub fn capture_window(window: &gtk4::ApplicationWindow, req: &ScreenshotRequest) {
    let output = req.output.clone();
    let (width, height) = (req.width as i32, req.height as i32);
    window.connect_map(move |_| {
        let output = output.clone();
        gtk4::glib::timeout_add_local_once(SETTLE_DELAY, move || {
            finish(bread_screenshots::capture_region(0, 0, width, height, &output));
        });
    });
}

/// View "reminder-snooze": force the snooze popover open shortly after the
/// window maps, then capture once *it* maps.
pub fn capture_with_snooze_open(
    window: &gtk4::ApplicationWindow,
    req: &ScreenshotRequest,
    snooze_popover: gtk4::Popover,
) {
    let output = req.output.clone();
    let (width, height) = (req.width as i32, req.height as i32);
    let popover_to_open = snooze_popover.clone();
    window.connect_map(move |_| {
        popover_to_open.set_autohide(false);
        let popover_to_open = popover_to_open.clone();
        gtk4::glib::timeout_add_local_once(PRE_POPUP_DELAY, move || {
            popover_to_open.popup();
        });
    });
    snooze_popover.connect_map(move |_| {
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
            eprintln!("breadpad: screenshot capture failed: {e}");
            std::process::exit(1);
        }
    }
}
