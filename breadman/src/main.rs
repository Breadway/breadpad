// Prevent a second console window on Windows in release. No-op on Linux, kept
// for parity with the Tauri project template.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use bread_utils::screenshot_cli::{validate_pair, DEFAULT_HEIGHT, DEFAULT_WIDTH};
use breadman_lib::ScreenshotRequest;
use breadpad_shared::store::Store;
use breadpad_shared::types::NoteType;
use chrono::Local;
use std::path::PathBuf;

/// Hand-rolled arg parsing, same call shape breadman has always accepted:
///
///   breadman                       # open the manager
///   breadman --view upcoming       # open straight to a named view
///   breadman done <id>             # mark a note done, no GUI
///   breadman upcoming --plain      # print pending reminders/todos, no GUI
///   breadman --screenshot <view> --output <path> [--width W] [--height H]
///
/// `--screenshot <view>` doubles as the view selector (same convention as
/// every other bread-ecosystem app's `--screenshot`), so it sets
/// `initial_view` too when `--view` isn't also given.
struct Args {
    view: Option<String>,
    done_id: Option<String>,
    upcoming_plain: bool,
    screenshot: Option<String>,
    output: Option<PathBuf>,
    width: u32,
    height: u32,
}

fn parse_args() -> Args {
    let mut args = Args {
        view: None,
        done_id: None,
        upcoming_plain: false,
        screenshot: None,
        output: None,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
    };
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--view" | "-v" => {
                i += 1;
                args.view = raw.get(i).cloned();
            }
            "done" => {
                i += 1;
                args.done_id = raw.get(i).cloned();
            }
            "upcoming" => {
                if raw.get(i + 1).map(String::as_str) == Some("--plain") {
                    args.upcoming_plain = true;
                    i += 1;
                }
                args.view = Some("upcoming".into());
            }
            "--screenshot" => {
                i += 1;
                args.screenshot = raw.get(i).cloned();
            }
            "--output" => {
                i += 1;
                args.output = raw.get(i).map(PathBuf::from);
            }
            "--width" => {
                i += 1;
                args.width = raw.get(i).and_then(|s| s.parse().ok()).unwrap_or(args.width);
            }
            "--height" => {
                i += 1;
                args.height = raw.get(i).and_then(|s| s.parse().ok()).unwrap_or(args.height);
            }
            _ => {}
        }
        i += 1;
    }
    args
}

impl Args {
    /// `Ok(None)` outside a screenshot run; `Err` if `--screenshot`/`--output`
    /// were given as an incomplete pair, checked before Tauri (or the store)
    /// ever opens.
    fn screenshot_request(&self) -> Result<Option<ScreenshotRequest>> {
        validate_pair(self.screenshot.as_deref(), self.output.as_deref())
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(self.output.as_ref().map(|output| ScreenshotRequest {
            output: output.clone(),
            width: self.width,
            height: self.height,
        }))
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("breadman=info".parse().unwrap()),
        )
        .init();

    let args = parse_args();

    if let Some(id) = &args.done_id {
        return cmd_done(id);
    }
    if args.upcoming_plain {
        return cmd_upcoming_plain();
    }

    let screenshot = args.screenshot_request()?;
    let initial_view = args.view.clone().or_else(|| args.screenshot.clone());

    breadman_lib::run(initial_view, screenshot);
    Ok(())
}

fn cmd_done(id: &str) -> Result<()> {
    let store = Store::new()?;
    let Some(mut note) = store.get_by_id(id)? else {
        anyhow::bail!("note {id} not found");
    };
    note.mark_done();
    store.update_note(&note)?;
    breadpad_shared::scheduler::Scheduler::cancel(&note.id).ok();
    println!("marked {id} as done");
    Ok(())
}

fn cmd_upcoming_plain() -> Result<()> {
    let store = Store::new()?;
    let mut notes: Vec<_> = store
        .load_all()?
        .into_iter()
        .filter(|n| {
            !n.done
                && matches!(n.note_type, NoteType::Reminder | NoteType::Todo)
                && n.effective_time().is_some()
        })
        .collect();
    notes.sort_by_key(|n| n.effective_time().expect("filtered by is_some above"));
    for note in &notes {
        let t = note.effective_time().expect("filtered by is_some above");
        let local: chrono::DateTime<Local> = t.into();
        println!(
            "[{}] {}: {}",
            note.id,
            local.format("%a %b %d %H:%M"),
            note.body
        );
    }
    Ok(())
}
