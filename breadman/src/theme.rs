//! Palette bridge. breadman's frontend is styled entirely through CSS custom
//! properties; this module renders them from the live wallpaper-derived
//! palette (via `bread-theme`, the same crate the GTK apps use) and
//! re-renders on change, so a `bread-theme reload` / wallpaper switch
//! recolours the window with no restart, exactly like the GTK build's live
//! reload.

use notify::{RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// The `<style>` payload: bread-theme's shared `:root` custom properties
/// (`--bg`, `--accent`, `--on-surface`, …) plus the design tokens
/// (`--font-family`, `--space-*`, `--radius-*`), plus the user's optional
/// `~/.config/breadpad/style.css` override appended last.
pub fn render_css() -> String {
    let palette = bread_theme::load_palette();
    let mut css = String::new();
    css.push_str(&bread_theme::css_custom_properties(&palette));
    css.push('\n');
    css.push_str(&bread_theme::css_tokens());
    if let Ok(user) = std::fs::read_to_string(breadpad_shared::config::style_css_path()) {
        css.push_str("\n/* ~/.config/breadpad/style.css */\n");
        css.push_str(&user);
    }
    css
}

fn watched_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(bread_theme::palette_cache_dir());
    if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") {
        dirs.push(PathBuf::from(rt).join("bread"));
    }
    if let Some(cfg) = breadpad_shared::config::style_css_path().parent() {
        dirs.push(cfg.to_path_buf());
    }
    dirs.retain(|d| d.exists());
    dirs
}

/// Spawn a filesystem watcher that emits `theme://changed` (payload: the new
/// CSS string) whenever the palette or the user stylesheet changes. Events
/// are coalesced over a short window so a burst of writes triggers one emit.
pub fn spawn_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!("theme watcher: {e}");
                return;
            }
        };
        for dir in watched_dirs() {
            if let Err(e) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
                tracing::warn!("theme watcher: cannot watch {}: {e}", dir.display());
            }
        }

        loop {
            // Block for the first event, then drain anything that lands in
            // the next 150ms and emit once.
            if rx.recv().is_err() {
                return;
            }
            while rx.recv_timeout(Duration::from_millis(150)).is_ok() {}
            let _ = app.emit("theme://changed", render_css());
        }
    });
}

/// One-shot command: the frontend calls this on boot to get the initial CSS.
#[tauri::command]
pub fn get_theme_css() -> String {
    render_css()
}
