//! Tauri entry point for the breadman manager window. `main.rs` handles the
//! headless CLI paths (`done`, `upcoming --plain`) and calls [`run`] for the
//! GUI.

mod commands;
mod state;
mod theme;

use breadpad_shared::config::Config;
use breadpad_shared::store::Store;
use state::AppState;
use std::path::PathBuf;
use std::time::Duration;
use tauri::Manager;

/// A `--screenshot <view> --output <path>` capture run, driven by
/// `bread-capture`. `view` doubles as `initial_view` (see [`run`]), matching
/// every other bread-ecosystem app's `--screenshot` convention.
#[derive(Clone)]
pub struct ScreenshotRequest {
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
}

/// The frontend gets this long to report its first paint (`notify_ui_ready`)
/// before a capture proceeds on a fixed delay anyway, so a render-blocking
/// frontend error can't hang `bread-capture` indefinitely.
const UI_READY_TIMEOUT: Duration = Duration::from_secs(10);

/// Launch the manager window, opening on `initial_view` (or "all"). When
/// `screenshot` is set, the window is resized/positioned to match the
/// requested capture canvas, and the process exits as soon as one frame is
/// grabbed via `bread_screenshots::capture_region` (compositor-side, same
/// primitive every other app's `--screenshot` mode uses; `bread-capture`
/// runs the whole app inside an isolated, fixed-size headless compositor,
/// so `(0, 0)` is always the window's own top-left corner).
pub fn run(initial_view: Option<String>, screenshot: Option<ScreenshotRequest>) {
    let cfg = Config::load().unwrap_or_else(|e| {
        tracing::warn!("failed to load config, using defaults: {e}");
        Config::default()
    });
    let store = Store::new().expect("cannot open the breadpad note store");

    // No archive rotation here: breadman is a viewer/manager and should not
    // rewrite the store on launch. The Archive view already merges done
    // notes still in notes.jsonl with archive.jsonl.
    let app_state = AppState::new(store, cfg, initial_view);

    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            theme::spawn_watcher(app.handle().clone());

            if let Some(req) = screenshot.clone() {
                let window = app
                    .get_webview_window("main")
                    .expect("breadman: no main window to screenshot");
                window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                    width: req.width,
                    height: req.height,
                }))?;
                window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x: 0,
                    y: 0,
                }))?;

                let (tx, rx) = std::sync::mpsc::channel();
                app.state::<AppState>().set_ui_ready_sender(tx);

                std::thread::spawn(move || {
                    // Ignore the Err (timed out with no signal): the fixed
                    // settle delay below still runs, same as a normal app
                    // that never wires readiness up at all.
                    let _ = rx.recv_timeout(UI_READY_TIMEOUT);
                    std::thread::sleep(bread_utils::screenshot_cli::SETTLE_DELAY);
                    let result = bread_screenshots::capture_region(
                        0,
                        0,
                        req.width as i32,
                        req.height as i32,
                        &req.output,
                    );
                    match result {
                        Ok(()) => std::process::exit(0),
                        Err(e) => {
                            eprintln!("breadman: screenshot capture failed: {e}");
                            std::process::exit(1);
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_initial_view,
            commands::list_notes,
            commands::list_archive,
            commands::get_config,
            commands::save_config,
            commands::list_errors,
            commands::classify_preview,
            commands::create_note,
            commands::update_note,
            commands::set_done,
            commands::delete_note,
            commands::notify_ui_ready,
            theme::get_theme_css,
        ])
        .run(tauri::generate_context!())
        .expect("error while running breadman");
}
