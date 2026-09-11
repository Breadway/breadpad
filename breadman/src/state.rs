use breadpad_shared::config::Config;
use breadpad_shared::store::Store;
use chrono::{DateTime, Local};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

/// One recorded error/warning surfaced to the Errors view. Session-scoped,
/// same as the GTK build's in-memory log.
#[derive(Clone, serde::Serialize)]
pub struct ErrorEntry {
    /// `HH:MM:SS`, local time.
    pub time: String,
    pub message: String,
}

/// Shared backend state, managed by Tauri. Every field is behind its own
/// mutex so a slow note write doesn't block a config read.
pub struct AppState {
    store: Mutex<Store>,
    pub cfg: Mutex<Config>,
    errors: Mutex<Vec<ErrorEntry>>,
    /// The view name to open on first paint (`--view` / `breadman upcoming`).
    pub initial_view: String,
    /// Sent once by [`AppState::signal_ui_ready`] when the frontend reports
    /// its first real paint of `initial_view`. `None` outside `--screenshot`
    /// mode, where nothing is listening.
    ui_ready_tx: Mutex<Option<Sender<()>>>,
}

impl AppState {
    pub fn new(store: Store, cfg: Config, initial_view: Option<String>) -> Self {
        AppState {
            store: Mutex::new(store),
            cfg: Mutex::new(cfg),
            errors: Mutex::new(Vec::new()),
            initial_view: initial_view.unwrap_or_else(|| "all".into()),
            ui_ready_tx: Mutex::new(None),
        }
    }

    /// Arm the ready signal for `--screenshot` mode. A no-op call site (the
    /// frontend always calls `notify_ui_ready` after its first paint) stays
    /// harmless in a normal run: `signal_ui_ready` just finds nothing armed.
    pub fn set_ui_ready_sender(&self, tx: Sender<()>) {
        *self.ui_ready_tx.lock().unwrap() = Some(tx);
    }

    /// Fire the ready signal, if anyone armed one. `take()` so a second
    /// paint (e.g. a reactive re-render) doesn't try to send on a channel
    /// whose receiver may already be gone.
    pub fn signal_ui_ready(&self) {
        if let Some(tx) = self.ui_ready_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }

    /// A `Store` clone with CalDAV wired in when the current config enables
    /// it, mirroring the GTK build's `write_store()`.
    pub fn write_store(&self) -> Store {
        let store = self.store.lock().unwrap().clone();
        let cfg = self.cfg.lock().unwrap();
        store.with_calendar_if_enabled(&cfg)
    }

    pub fn read_store(&self) -> Store {
        self.store.lock().unwrap().clone()
    }

    pub fn log_error(&self, message: impl Into<String>) {
        let entry = ErrorEntry {
            time: Local::now().format("%H:%M:%S").to_string(),
            message: message.into(),
        };
        tracing::warn!("{}", entry.message);
        self.errors.lock().unwrap().push(entry);
    }

    pub fn errors_snapshot(&self) -> Vec<ErrorEntry> {
        self.errors.lock().unwrap().clone()
    }

    pub fn default_morning(&self) -> String {
        self.cfg.lock().unwrap().reminders.default_morning.clone()
    }
}

/// Local `HH:MM:SS` helper kept next to `ErrorEntry` for tests.
#[allow(dead_code)]
pub fn fmt_local(dt: DateTime<Local>) -> String {
    dt.format("%H:%M:%S").to_string()
}
