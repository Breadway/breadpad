//! Long-running command subscription for `bread.command.pad.*`.
//!
//! `breadpad` is still a one-shot capture popup by default. `breadpad listen`
//! is the optional persistent process that can honor bus commands. See
//! `EVENTS.md`.

use anyhow::Result;
use bread_utils::bread_client::{BreadClient, BreadEvent};

/// Sibling-app id in `bread_shared::apps::KNOWN_APPS`.
const APP_ID: &str = "pad";

/// Subscribe to `bread.command.pad.**` and block until the process is killed.
///
/// breadd being absent is not an error: [`BreadClient::subscribe`] reconnects
/// with backoff, and `on_event` simply isn't called until the daemon is up.
pub fn run() -> Result<()> {
    let client = BreadClient::connect(APP_ID);
    if client.health().is_none() {
        tracing::warn!("breadd unreachable; command subscription will connect when it comes back");
    }

    let _commands = client.subscribe("bread.command.pad.**", |event| {
        handle_command(&event);
    });

    tracing::info!("listening for bread.command.pad.**");
    loop {
        std::thread::park();
    }
}

/// Reacts to `bread.command.pad.*` verbs. Only `capture` is honored today —
/// other verbs are ignored, not stubbed as no-ops that pretend to succeed.
fn handle_command(event: &BreadEvent) {
    let Some(verb) = command_verb(&event.event) else {
        return;
    };
    match verb {
        "capture" => handle_capture(),
        other => {
            tracing::debug!("ignoring unrecognized command verb '{other}'");
        }
    }
}

fn handle_capture() {
    // Same as running `breadpad` with no args: open the capture popup.
    let result = spawn_self();
    let client = BreadClient::connect(APP_ID);
    match result {
        Ok(_) => client.emit("bread.pad.capture.done", serde_json::json!({})),
        Err(e) => {
            tracing::warn!("bread.command.pad.capture failed: {e}");
            client.emit(
                "bread.pad.capture.failed",
                serde_json::json!({ "error": e.to_string() }),
            );
        }
    }
}

fn spawn_self() -> std::io::Result<std::process::Child> {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("breadpad"));
    std::process::Command::new(exe).spawn()
}

fn command_verb(event_name: &str) -> Option<&str> {
    event_name.strip_prefix("bread.command.pad.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_verb_strips_pad_prefix() {
        assert_eq!(command_verb("bread.command.pad.capture"), Some("capture"));
        assert_eq!(command_verb("bread.command.pad.snooze"), Some("snooze"));
        assert_eq!(command_verb("bread.command.box.open"), None);
        assert_eq!(command_verb("bread.pad.captured"), None);
    }
}
