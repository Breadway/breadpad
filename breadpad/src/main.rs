use anyhow::Result;
use breadpad_shared::{
    calendar::CalDavClient,
    classifier::Classifier,
    config::Config,
    parser::parse_rule_based,
    scheduler::Scheduler,
    store::Store,
    types::{Note, NoteType},
};
use bread_utils::bread_client::BreadClient;
use gtk4::{glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Once};

mod listen;
mod screenshot;

/// Sibling-app id in `bread_shared::apps::KNOWN_APPS`. Events are `bread.pad.*`.
const APP_ID: &str = "pad";

/// `bread.pad.captured` after a successful quick-capture save. Fire-and-forget:
/// `BreadClient::emit` is a silent no-op when breadd is down or missing.
fn emit_captured(id: &str) {
    BreadClient::connect(APP_ID).emit("bread.pad.captured", serde_json::json!({ "id": id }));
}

/// `bread.pad.reminder.due` when `breadpad fire <id>` actually shows a reminder
/// (the existing systemd-timer hook — not a new daemon). Same fail-silent emit.
fn emit_reminder_due(id: &str) {
    BreadClient::connect(APP_ID).emit("bread.pad.reminder.due", serde_json::json!({ "id": id }));
}

static ORT_INIT: Once = Once::new();

fn init_ort_once(cfg: &Config) {
    ORT_INIT.call_once(|| {
        let Some(path) = cfg.model.resolved_ort_dylib_path() else { return; };
        if !path.exists() {
            tracing::warn!("ORT dylib not found at {:?}; Tier 2 disabled", path);
            return;
        }
        tracing::info!("loading ONNX Runtime from {:?}", path);
        match ort::init_from(&path) {
            Ok(builder) => { builder.commit(); }
            Err(e) => tracing::warn!("ORT init failed: {}; Tier 2 disabled", e),
        }
    });
}

mod args {
    use bread_utils::screenshot_cli::{validate_pair, DEFAULT_HEIGHT, DEFAULT_WIDTH};
    use std::path::Path;

    #[derive(Debug)]
    pub struct Args {
        pub note_type: Option<String>,
        pub no_classify: bool,
        pub status: bool,
        pub fire_id: Option<String>,
        pub download_model: bool,
        pub model_info: bool,
        pub calendar_test: bool,
        pub calendar_list_uid: Option<String>,
        pub screenshot: Option<String>,
        pub output: Option<String>,
        pub width: u32,
        pub height: u32,
        pub listen: bool,
    }

    impl Args {
        /// `None` for a normal run. Exits the process with an error if the
        /// `--screenshot` / `--output` pair is incomplete, before any GTK
        /// setup happens.
        pub fn screenshot_request(&self) -> Option<crate::screenshot::ScreenshotRequest> {
            if let Err(e) = validate_pair(
                self.screenshot.as_deref(),
                self.output.as_deref().map(Path::new),
            ) {
                eprintln!("breadpad: {e}");
                std::process::exit(1);
            }
            Some(crate::screenshot::ScreenshotRequest {
                view: self.screenshot.clone()?,
                output: self.output.clone()?.into(),
                width: self.width,
                height: self.height,
            })
        }
    }

    pub fn parse() -> Args {
        let mut args = Args {
            note_type: None,
            no_classify: false,
            status: false,
            fire_id: None,
            download_model: false,
            model_info: false,
            calendar_test: false,
            calendar_list_uid: None,
            screenshot: None,
            output: None,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            listen: false,
        };
        let raw: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < raw.len() {
            match raw[i].as_str() {
                "--type" | "-t" => {
                    i += 1;
                    args.note_type = raw.get(i).cloned();
                }
                "--no-classify" => args.no_classify = true,
                "--status" => args.status = true,
                "download-model" => args.download_model = true,
                "model-info" => args.model_info = true,
                "listen" => args.listen = true,
                "fire" => {
                    i += 1;
                    args.fire_id = raw.get(i).cloned();
                }
                "calendar" => {
                    i += 1;
                    match raw.get(i).map(|s| s.as_str()) {
                        Some("test") => args.calendar_test = true,
                        Some("list-uid") => {
                            i += 1;
                            args.calendar_list_uid =
                                Some(raw.get(i).cloned().unwrap_or_default());
                        }
                        _ => {}
                    }
                }
                "--screenshot" => {
                    i += 1;
                    args.screenshot = raw.get(i).cloned();
                }
                "--output" => {
                    i += 1;
                    args.output = raw.get(i).cloned();
                }
                "--width" => {
                    i += 1;
                    if let Some(v) = raw.get(i).and_then(|s| s.parse().ok()) {
                        args.width = v;
                    }
                }
                "--height" => {
                    i += 1;
                    if let Some(v) = raw.get(i).and_then(|s| s.parse().ok()) {
                        args.height = v;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        args
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("breadpad=info".parse().unwrap()),
        )
        .init();

    let args = args::parse();
    if args.listen {
        return listen::run();
    }
    let cfg = Config::load()?;

    if args.status {
        return cmd_status(&cfg);
    }
    if args.download_model {
        return cmd_download_model(&cfg);
    }
    if args.model_info {
        return cmd_model_info(&cfg);
    }
    if let Some(id) = args.fire_id {
        return cmd_fire(&id, &cfg);
    }
    if args.calendar_test {
        return cmd_calendar_test(&cfg);
    }
    if let Some(note_id) = args.calendar_list_uid {
        return cmd_calendar_list_uid(&note_id, &cfg);
    }

    let screenshot_req = args.screenshot_request();
    if let Some(req) = &screenshot_req {
        if req.view == "reminder" || req.view == "reminder-snooze" {
            // The real path (`fire <id>`, above) needs a real due note from
            // the Store. A screenshot doesn't have one to work with — and
            // shouldn't wait for one — so it builds a throwaway sample
            // instead, never touching the Store at all.
            let mut sample = Note::new(
                "Sample reminder text".into(),
                NoteType::from_str("reminder"),
                None,
            );
            sample.time = Some(chrono::Utc::now());
            return run_reminder_window(sample, &cfg, screenshot_req);
        }
    }
    run_popup(args.note_type, args.no_classify, cfg, screenshot_req)
}

fn cmd_status(cfg: &Config) -> Result<()> {
    init_ort_once(cfg);
    let store = Store::new()?;
    let notes = store.load_all()?;
    let (model_path, tokenizer_path) = cfg.model.resolved_paths();
    let classifier = Classifier::load_with_paths(
        &cfg.reminders.default_morning,
        model_path,
        tokenizer_path,
    );
    println!("breadpad status");
    println!("  notes: {}", notes.len());
    println!(
        "  model: {}",
        if classifier.model_available() {
            format!("loaded ({:?})", classifier.model_path)
        } else {
            "not loaded — run 'breadpad download-model'".into()
        }
    );
    println!("  execution provider: {}", classifier.active_provider.as_str());
    Ok(())
}

fn cmd_model_info(cfg: &Config) -> Result<()> {
    init_ort_once(cfg);
    let (model_path, tokenizer_path) = cfg.model.resolved_paths();
    let classifier = Classifier::load_with_paths(
        &cfg.reminders.default_morning,
        model_path,
        tokenizer_path,
    );
    println!("model path: {:?}", classifier.model_path);
    println!("execution provider: {}", classifier.active_provider.as_str());
    println!(
        "model available: {}",
        if classifier.model_available() { "yes" } else { "no" }
    );
    Ok(())
}

fn cmd_download_model(cfg: &Config) -> Result<()> {
    // Placeholder — a real implementation would download a quantised ONNX model.
    // The exact model URL is left for the user to configure.
    let (model_path, tokenizer_path) = cfg.model.resolved_paths();
    if let Some(dir) = model_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if let Some(dir) = tokenizer_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    println!("Model path: {}", model_path.display());
    println!("Tokenizer path: {}", tokenizer_path.display());
    println!("Place the classifier ONNX and tokenizer JSON at those paths.");
    println!("(Automatic download not yet configured — set a model URL in breadpad.toml)");
    Ok(())
}

fn cmd_calendar_test(cfg: &Config) -> Result<()> {
    if !cfg.calendar.enabled {
        println!("Calendar integration is disabled. Set [calendar] enabled = true in breadpad.toml.");
        return Ok(());
    }
    let cal_cfg = cfg.calendar.clone();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let client = CalDavClient::new(cal_cfg);
        match client.test_connection().await {
            Ok(()) => println!("CalDAV connection OK"),
            Err(e) => println!("CalDAV connection failed: {}", e),
        }
    });
    Ok(())
}

fn cmd_calendar_list_uid(note_id: &str, cfg: &Config) -> Result<()> {
    use breadpad_shared::calendar::caldav_uid;

    if note_id.is_empty() {
        // List all notes that would have CalDAV events (have time or rrule)
        if cfg.calendar.enabled {
            let cal_cfg = cfg.calendar.clone();
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let events = rt.block_on(async {
                let client = CalDavClient::new(cal_cfg);
                client.list_events().await
            });
            match events {
                Ok(evs) => {
                    if evs.is_empty() {
                        println!("No events found on CalDAV server.");
                    } else {
                        for ev in &evs {
                            println!("{}\t{}", ev.uid, ev.summary);
                        }
                    }
                }
                Err(e) => println!("CalDAV list failed: {}", e),
            }
        } else {
            let store = Store::new()?;
            let notes = store.load_all()?;
            let scheduled: Vec<_> = notes
                .iter()
                .filter(|n| n.time.is_some() || n.rrule.is_some())
                .collect();
            if scheduled.is_empty() {
                println!("No notes with scheduled times or recurrence rules.");
            } else {
                for note in scheduled {
                    println!("{}\t{}", caldav_uid(note), note.body);
                }
            }
        }
    } else {
        let store = Store::new()?;
        match store.get_by_id(note_id)? {
            Some(note) => println!("{}", caldav_uid(&note)),
            None => println!("note '{}' not found", note_id),
        }
    }
    Ok(())
}

fn cmd_fire(id: &str, cfg: &Config) -> Result<()> {
    let store = Store::new()?.with_calendar_if_enabled(cfg);
    let note = match store.get_by_id(id)? {
        Some(n) => n,
        None => {
            tracing::error!("note {} not found", id);
            return Ok(());
        }
    };

    if !Scheduler::fire(&note, cfg.reminders.missed_grace_minutes) {
        return Ok(());
    }

    emit_reminder_due(&note.id);

    // Schedule next recurrence before showing UI
    if note.rrule.is_some() {
        if let Some(next) = Scheduler::next_recurrence(&note, &cfg.reminders.default_morning) {
            let mut updated = note.clone();
            updated.time = Some(next);
            updated.snoozed_until = None;
            store.update_note(&updated)?;
            Scheduler::schedule(&updated)?;
        }
    }

    run_reminder_window(note, cfg, None)
}

fn run_reminder_window(
    note: breadpad_shared::types::Note,
    cfg: &Config,
    screenshot_req: Option<screenshot::ScreenshotRequest>,
) -> Result<()> {
    let mut builder = gtk4::Application::builder().application_id("com.breadway.breadpad.reminder");
    if screenshot_req.is_some() {
        // Same reasoning as run_popup's NON_UNIQUE: a screenshot run must
        // get its own fresh window, never activate a real reminder that
        // happens to already be showing.
        builder = builder.flags(gtk4::gio::ApplicationFlags::NON_UNIQUE);
    }
    let app = builder.build();

    let note = Arc::new(note);
    let cfg = Arc::new(cfg.clone());

    app.connect_activate(move |app| {
        build_reminder_window(app, note.clone(), cfg.clone(), screenshot_req.clone());
    });

    app.run_with_args::<String>(&[]);
    Ok(())
}

fn humanize_snooze(s: &str) -> &str {
    match s {
        "15m" => "15 minutes",
        "1h" => "1 hour",
        "tomorrow_morning" => "Tomorrow morning",
        other => other,
    }
}

fn resolve_snooze(key: &str, cfg: &Config) -> Option<chrono::DateTime<chrono::Utc>> {
    let now = chrono::Utc::now();
    match key {
        "15m" => Some(now + chrono::Duration::minutes(15)),
        "1h" => Some(now + chrono::Duration::hours(1)),
        "tomorrow_morning" => {
            let local = chrono::Local::now();
            let parts: Vec<u32> = cfg
                .reminders
                .default_morning
                .split(':')
                .filter_map(|s| s.parse().ok())
                .collect();
            let h = parts.first().copied().unwrap_or(8);
            let m = parts.get(1).copied().unwrap_or(0);
            let tomorrow = local.date_naive() + chrono::Duration::days(1);
            let naive = tomorrow.and_hms_opt(h, m, 0)?;
            Some(breadpad_shared::util::local_naive_to_utc(naive))
        }
        _ => None,
    }
}

fn build_reminder_window(
    app: &gtk4::Application,
    note: Arc<breadpad_shared::types::Note>,
    cfg: Arc<Config>,
    screenshot_req: Option<screenshot::ScreenshotRequest>,
) {
    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("breadpad reminder")
        .default_width(420)
        .default_height(1)
        .decorated(false)
        .resizable(false)
        .build();

    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
    window.auto_exclusive_zone_enable();

    apply_css(&cfg);

    let type_emoji = match note.note_type.as_str() {
        "reminder" => "🔔",
        "todo"     => "✅",
        "idea"     => "💡",
        "question" => "❓",
        _          => "📝",
    };

    let outer = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(0)
        .css_classes(["reminder-window"])
        .build();

    // Header strip
    let header = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .margin_top(16)
        .margin_bottom(8)
        .margin_start(20)
        .margin_end(20)
        .build();

    header.append(
        &gtk4::Label::builder()
            .label(type_emoji)
            .css_classes(["reminder-emoji"])
            .build(),
    );
    header.append(
        &gtk4::Label::builder()
            .label("Reminder")
            .css_classes(["reminder-title"])
            .hexpand(true)
            .xalign(0.0)
            .build(),
    );

    // Optional time label
    if let Some(t) = note.effective_time() {
        let local: chrono::DateTime<chrono::Local> = t.into();
        header.append(
            &gtk4::Label::builder()
                .label(local.format("%H:%M").to_string())
                .css_classes(["reminder-time"])
                .build(),
        );
    }

    outer.append(&header);

    // Body
    let body_label = gtk4::Label::builder()
        .label(&note.body)
        .css_classes(["reminder-body"])
        .wrap(true)
        .xalign(0.0)
        .margin_start(20)
        .margin_end(20)
        .margin_bottom(16)
        .build();
    outer.append(&body_label);

    // Separator
    outer.append(&gtk4::Separator::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .build());

    // Button row — same inset as the header/body zone above (20px), which
    // used to be 16px here, visible as a step across the divider.
    let btn_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(20)
        .margin_end(20)
        .build();

    let dismiss_btn = gtk4::Button::builder()
        .label("Dismiss")
        .css_classes(["reminder-dismiss"])
        .build();

    // Snooze popover. No arrow and a matching flat border (see
    // popover.snooze-popover in the shared theme) so it reads as the same
    // elevation language as the reminder card, instead of GTK's default
    // arrow+drop-shadow chrome next to the card's flat 1px border.
    let snooze_popover = gtk4::Popover::new();
    snooze_popover.set_has_arrow(false);
    snooze_popover.add_css_class("snooze-popover");
    let snooze_vbox = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(0)
        .margin_top(4)
        .margin_bottom(4)
        .build();

    // Left-aligned row: a bare Button::builder().label() centers its text,
    // so each option gets an explicit xalign(0.0) label as its child and
    // hexpand(true) so the row fills the popover's full width instead of
    // shrinking to the longest label.
    let snooze_option_row = |label: &str| {
        gtk4::Button::builder()
            .child(&gtk4::Label::builder().label(label).xalign(0.0).build())
            .css_classes(["snooze-option"])
            .hexpand(true)
            .build()
    };

    for opt in &cfg.settings.snooze_options {
        let label = humanize_snooze(opt).to_string();
        let btn = snooze_option_row(&label);
        let key = opt.clone();
        let note_c = note.clone();
        let cfg_c = cfg.clone();
        let win_c = window.clone();
        let popover_c = snooze_popover.clone();
        btn.connect_clicked(move |_| {
            if let Some(until) = resolve_snooze(&key, &cfg_c) {
                if let Ok(store) = Store::new().map(|s| s.with_calendar_if_enabled(&cfg_c)) {
                    let mut updated = note_c.as_ref().clone();
                    updated.snoozed_until = Some(until);
                    let _ = store.update_note(&updated);
                    let _ = Scheduler::schedule(&updated);
                }
            }
            popover_c.popdown();
            win_c.close();
        });
        snooze_vbox.append(&btn);
    }

    // Custom… — reuses the same free-form time parsing breadman's dialogs
    // already use, rather than inventing a separate time-picker widget.
    let custom_entry = gtk4::Entry::builder()
        .placeholder_text("tomorrow 9am  /  in 45 minutes")
        .css_classes(["snooze-custom-entry"])
        .visible(false)
        .build();
    {
        let note_c = note.clone();
        let cfg_c = cfg.clone();
        let win_c = window.clone();
        let popover_c = snooze_popover.clone();
        let entry_c = custom_entry.clone();
        custom_entry.connect_activate(move |_| {
            let text = entry_c.text().to_string();
            let parsed = parse_rule_based(&text, &cfg_c.reminders.default_morning);
            if let Some(until) = parsed.time {
                if let Ok(store) = Store::new().map(|s| s.with_calendar_if_enabled(&cfg_c)) {
                    let mut updated = note_c.as_ref().clone();
                    updated.snoozed_until = Some(until);
                    let _ = store.update_note(&updated);
                    let _ = Scheduler::schedule(&updated);
                }
                popover_c.popdown();
                win_c.close();
            }
        });
    }
    let custom_btn = snooze_option_row("Custom\u{2026}");
    {
        let custom_entry_c = custom_entry.clone();
        custom_btn.connect_clicked(move |btn| {
            btn.set_visible(false);
            custom_entry_c.set_visible(true);
            custom_entry_c.grab_focus();
        });
    }
    snooze_vbox.append(&custom_btn);
    snooze_vbox.append(&custom_entry);

    snooze_popover.set_child(Some(&snooze_vbox));

    let snooze_btn = gtk4::MenuButton::builder()
        .label("Snooze")
        .css_classes(["reminder-snooze"])
        .popover(&snooze_popover)
        .build();

    let done_btn = gtk4::Button::builder()
        .label("Done  ✓")
        .css_classes(["confirm-button", "reminder-done"])
        .hexpand(true)
        .build();

    {
        let note_c = note.clone();
        let cfg_c = cfg.clone();
        let win_c = window.clone();
        done_btn.connect_clicked(move |_| {
            if let Ok(store) = Store::new().map(|s| s.with_calendar_if_enabled(&cfg_c)) {
                let mut updated = note_c.as_ref().clone();
                updated.mark_done();
                let _ = store.update_note(&updated);
            }
            win_c.close();
        });
    }

    {
        let win_c = window.clone();
        dismiss_btn.connect_clicked(move |_| { win_c.close(); });
    }

    btn_row.append(&dismiss_btn);
    btn_row.append(&snooze_btn);
    btn_row.append(&done_btn);
    outer.append(&btn_row);

    window.set_child(Some(&outer));

    if let Some(req) = screenshot_req {
        if req.view == "reminder-snooze" {
            screenshot::capture_with_snooze_open(&window, &req, snooze_popover.clone());
        } else {
            screenshot::capture_window(&window, &req);
        }
    }

    window.present();
}

fn run_popup(
    preset_type: Option<String>,
    no_classify: bool,
    cfg: Config,
    screenshot_req: Option<screenshot::ScreenshotRequest>,
) -> Result<()> {
    // Try to get current Hyprland workspace
    let workspace = get_active_workspace();

    let mut builder = gtk4::Application::builder().application_id("com.breadway.breadpad");
    if screenshot_req.is_some() {
        // GApplication is single-instance by default; this machine typically
        // already has a real breadpad instance, so without this a
        // screenshot run would just toggle-close the *existing* instance's
        // window instead of starting a fresh one that ever sees
        // `screenshot_req` (see the `app.windows().first()` toggle below).
        builder = builder.flags(gtk4::gio::ApplicationFlags::NON_UNIQUE);
    }
    let app = builder.build();

    let cfg = Arc::new(cfg);

    app.connect_activate(move |app| {
        if screenshot_req.is_none() {
            if let Some(win) = app.windows().first().cloned() {
                win.close();
                return;
            }
        }
        build_window(app, cfg.clone(), workspace.clone(), preset_type.clone(), no_classify, screenshot_req.clone());
    });

    let code = app.run_with_args::<String>(&[]);
    if code != glib::ExitCode::SUCCESS {
        anyhow::bail!("GTK application exited with error");
    }
    Ok(())
}

fn get_active_workspace() -> Option<String> {
    // Use hyprctl via CLI since the async API would require a runtime here
    let out = std::process::Command::new("hyprctl")
        .args(["activeworkspace", "-j"])
        .output()
        .ok()?;
    let val: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    val.get("id").and_then(|v| v.as_i64()).map(|id| id.to_string())
}

fn build_window(
    app: &gtk4::Application,
    cfg: Arc<Config>,
    workspace: Option<String>,
    preset_type: Option<String>,
    no_classify: bool,
    screenshot_req: Option<screenshot::ScreenshotRequest>,
) {
    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("breadpad")
        .default_width(600)
        .default_height(1)
        .decorated(false)
        .resizable(false)
        .build();

    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
    window.auto_exclusive_zone_enable();
    window.set_anchor(Edge::Top, false);
    window.set_anchor(Edge::Bottom, false);
    window.set_anchor(Edge::Left, false);
    window.set_anchor(Edge::Right, false);

    apply_css(&cfg);

    let vbox = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(8)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    let entry = gtk4::Entry::builder()
        .placeholder_text("What's on your mind?")
        .css_classes(["popup-entry"])
        .hexpand(true)
        .build();

    let selected_type: Rc<RefCell<NoteType>> = Rc::new(RefCell::new(
        preset_type
            .as_deref()
            .map(NoteType::from_str)
            .unwrap_or(NoteType::from_str(&cfg.settings.default_type)),
    ));

    // Type chip row
    let chip_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(4)
        .build();

    let chips: Vec<(gtk4::Button, NoteType)> = NoteType::all_builtin()
        .iter()
        .map(|&name| {
            let btn = gtk4::Button::builder()
                .label(name)
                .css_classes(["type-chip"])
                .build();
            (btn, NoteType::from_str(name))
        })
        .collect();

    for (btn, nt) in &chips {
        let selected_type_clone = selected_type.clone();
        let nt_clone = nt.clone();
        let chips_clone: Vec<gtk4::Button> = chips.iter().map(|(b, _)| b.clone()).collect();

        btn.connect_clicked(move |clicked| {
            *selected_type_clone.borrow_mut() = nt_clone.clone();
            for b in &chips_clone {
                b.remove_css_class("active");
            }
            clicked.add_css_class("active");
        });
        chip_box.append(btn);
    }

    // Mark the initial chip active
    {
        let current = selected_type.borrow().clone();
        for (btn, nt) in &chips {
            if *nt == current {
                btn.add_css_class("active");
            }
        }
    }

    // No submit button — this popup is keyboard-driven (grabs focus on
    // open, Escape closes it) and a bare accent-teal checkmark used to sit
    // next to the selected-type pill in the same accent teal, carrying two
    // different meanings in one colour. A hint is enough.
    let enter_hint = gtk4::Label::builder()
        .label("Press Enter to add")
        .css_classes(["dim-label"])
        .build();

    let bottom_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .build();
    bottom_row.append(&chip_box);

    let spacer = gtk4::Box::builder().hexpand(true).build();
    bottom_row.append(&spacer);
    bottom_row.append(&enter_hint);

    vbox.append(&entry);
    vbox.append(&bottom_row);
    window.set_child(Some(&vbox));

    let win_clone = window.clone();
    let entry_clone = entry.clone();
    let selected_type_clone = selected_type.clone();
    let cfg_clone = cfg.clone();
    let workspace_clone = workspace.clone();

    let save_and_close = {
        let win = win_clone.clone();
        let entry = entry_clone.clone();
        let selected_type = selected_type_clone.clone();
        let cfg = cfg_clone.clone();
        let workspace = workspace_clone.clone();

        move || {
            let text = entry.text().to_string();
            if text.trim().is_empty() {
                win.close();
                return;
            }
            let note_type = selected_type.borrow().clone();
            let cfg_c = cfg.clone();
            let ws_c = workspace.clone();
            // Close first so the popup disappears immediately, then save.
            win.close();
            glib::idle_add_local_once(move || {
                save_note_classified(&text, note_type, no_classify, cfg_c, ws_c);
            });
        }
    };

    // Entry activate (Enter key)
    {
        let save = save_and_close.clone();
        entry.connect_activate(move |_| save());
    }

    // Escape key
    let key_ctrl = gtk4::EventControllerKey::new();
    let win_for_key = window.clone();
    key_ctrl.connect_key_pressed(move |_, key, _, _| {
        if key == gtk4::gdk::Key::Escape {
            win_for_key.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key_ctrl);

    if let Some(req) = screenshot_req {
        screenshot::dispatch(&window, req);
    }

    window.present();
    entry.grab_focus();
}

fn save_note_classified(
    text: &str,
    user_type: NoteType,
    no_classify: bool,
    cfg: Arc<Config>,
    workspace: Option<String>,
) {
    let default_type = NoteType::from_str(&cfg.settings.default_type);
    let mut note = Note::new(text.into(), user_type.clone(), workspace);

    if !no_classify {
        init_ort_once(&cfg);
        let (model_path, tokenizer_path) = cfg.model.resolved_paths();
        let mut classifier = Classifier::load_with_paths(
            &cfg.reminders.default_morning,
            model_path,
            tokenizer_path,
        )
        .with_ollama(cfg.model.ollama.clone());
        let result = classifier.classify(text);
        if user_type == default_type {
            note.note_type = result.note_type;
        }
        note.time = result.time;
        note.rrule = result.rrule;
        note.body = result.body;
    }

    let store = match Store::new() {
        Ok(s) => {
            if cfg.calendar.enabled {
                s.with_calendar(cfg.calendar.clone())
            } else {
                s
            }
        }
        Err(e) => { tracing::error!("failed to open store: {}", e); return; }
    };
    if let Err(e) = store.save_note(&note) {
        tracing::error!("failed to save note: {}", e);
        return;
    }
    emit_captured(&note.id);
    if note.time.is_some() {
        if let Err(e) = Scheduler::schedule(&note) {
            tracing::warn!("failed to schedule reminder: {}", e);
        }
    }
}

fn apply_css(_cfg: &Config) {
    // Hot-reloads on `bread-theme reload` (recolours to the new pywal palette
    // and re-reads the user's style.css). See breadpad_shared::theme::apply_live.
    breadpad_shared::theme::apply_live();
}
