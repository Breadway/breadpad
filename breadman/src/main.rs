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
///
/// Kept in step with [`parse_args_from`] and the commands handled in `main`.
const USAGE: &str = concat!(
    "breadman ",
    env!("CARGO_PKG_VERSION"),
    "\n",
    "Breadpad manager for Bread OS\n\n",
    "Usage:\n",
    "  breadman [options]\n",
    "  breadman --view <view>\n",
    "  breadman done <id>\n",
    "  breadman upcoming [--plain]\n",
    "  breadman --screenshot <view> --output <path> [--width <px>] [--height <px>]\n\n",
    "Options:\n",
    "  -v, --view <view>        Open straight to a named view\n",
    "      --screenshot <view>  Render a view for screenshots and exit\n",
    "      --output <path>      Screenshot output path\n",
    "      --width <px>         Screenshot width\n",
    "      --height <px>        Screenshot height\n",
    "  -h, --help               Print this help and exit\n",
    "      --version            Print the version and exit\n\n",
    "Commands:\n",
    "  done <id>                Mark a note done, no GUI\n",
    "  upcoming [--plain]       Print pending reminders/todos, no GUI\n",
);

struct Args {
    view: Option<String>,
    done_id: Option<String>,
    upcoming_plain: bool,
    screenshot: Option<String>,
    output: Option<PathBuf>,
    width: u32,
    height: u32,
    help: bool,
    version: bool,
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    parse_args_from(&raw)
}

/// Split out from [`parse_args`] so the flags can be tested without touching
/// the process argv. `-h`/`--help` and `--version` are only recorded here:
/// `main` answers them before tracing, config, the single-instance lock or
/// Tauri.
fn parse_args_from(raw: &[String]) -> Args {
    let mut args = Args {
        view: None,
        done_id: None,
        upcoming_plain: false,
        screenshot: None,
        output: None,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        help: false,
        version: false,
    };
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "-h" | "--help" => args.help = true,
            "--version" => args.version = true,
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
                args.width = raw
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(args.width);
            }
            "--height" => {
                i += 1;
                args.height = raw
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(args.height);
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
    // Answer help and version before tracing, config loading, the
    // single-instance lock, the store or any Tauri/GTK setup. `--help` used
    // to fall through to the manager window, which looked like a hang and
    // left a process running.
    let args = parse_args();
    if args.help {
        print!("{USAGE}");
        return Ok(());
    }
    if args.version {
        println!("breadman {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("breadman=info".parse().unwrap()),
        )
        .init();

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

#[cfg(test)]
mod tests {
    use super::{parse_args_from, USAGE};

    fn v(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn help_and_version_flags_are_recognised() {
        assert!(parse_args_from(&v(&["-h"])).help);
        assert!(parse_args_from(&v(&["--help"])).help);
        assert!(parse_args_from(&v(&["--version"])).version);
        // Help/version do not swallow the rest of the arguments.
        let a = parse_args_from(&v(&["--version", "--view", "upcoming"]));
        assert!(a.version);
        assert_eq!(a.view.as_deref(), Some("upcoming"));
    }

    #[test]
    fn normal_args_still_parse() {
        let a = parse_args_from(&v(&[
            "--view",
            "upcoming",
            "--screenshot",
            "upcoming",
            "--output",
            "/tmp/opencode/breadman.png",
            "--width",
            "800",
            "--height",
            "600",
        ]));
        assert_eq!(a.view.as_deref(), Some("upcoming"));
        assert_eq!(a.screenshot.as_deref(), Some("upcoming"));
        assert_eq!(
            a.output.as_deref().map(|p| p.to_str().unwrap()),
            Some("/tmp/opencode/breadman.png")
        );
        assert_eq!((a.width, a.height), (800, 600));
        assert!(!a.help);
        assert!(!a.version);
    }

    #[test]
    fn commands_still_parse() {
        let done = parse_args_from(&v(&["done", "note-42"]));
        assert_eq!(done.done_id.as_deref(), Some("note-42"));

        let upcoming = parse_args_from(&v(&["upcoming", "--plain"]));
        assert!(upcoming.upcoming_plain);
        assert_eq!(upcoming.view.as_deref(), Some("upcoming"));
    }

    #[test]
    fn usage_lists_the_supported_surface() {
        for needle in [
            "--help",
            "--version",
            "--view",
            "-v",
            "--screenshot",
            "--output",
            "--width",
            "--height",
            "done",
            "upcoming",
            "--plain",
        ] {
            assert!(USAGE.contains(needle), "usage is missing {needle}");
        }
        assert!(USAGE.contains(env!("CARGO_PKG_VERSION")));
    }
}
