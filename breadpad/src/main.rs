use anyhow::Result;
use breadpad_shared::{
    calendar::CalDavClient,
    classifier::Classifier,
    config::{style_css_path, Config},
    scheduler::Scheduler,
    store::Store,
    theme::{build_css, load_palette},
    types::{Note, NoteType},
};
use gtk4::{glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

mod args {
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
    let cfg = Config::load()?;

    if args.status {
        return cmd_status(&cfg);
    }
    if args.download_model {
        return cmd_download_model();
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

    run_popup(args.note_type, args.no_classify, cfg)
}

fn cmd_status(cfg: &Config) -> Result<()> {
    let store = Store::new()?;
    let notes = store.load_all()?;
    let classifier = Classifier::load(&cfg.model.execution_provider, &cfg.reminders.default_morning);
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
    let classifier = Classifier::load(&cfg.model.execution_provider, &cfg.reminders.default_morning);
    println!("model path: {:?}", classifier.model_path);
    println!("execution provider: {}", classifier.active_provider.as_str());
    println!(
        "model available: {}",
        if classifier.model_available() { "yes" } else { "no" }
    );
    Ok(())
}

fn cmd_download_model() -> Result<()> {
    // Placeholder — a real implementation would download a quantised ONNX model.
    // The exact model URL is left for the user to configure.
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.local/share"))
        .join("breadpad")
        .join("model");
    std::fs::create_dir_all(&dir)?;
    println!("Model directory: {}", dir.display());
    println!("Place classifier.onnx and tokenizer.json in that directory.");
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
    let store = Store::new()?;
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

    // Send notification via notify-send
    let title = format!("[{}] breadpad reminder", note.note_type);

    let mut cmd = std::process::Command::new("notify-send");
    cmd.arg("--urgency=normal")
        .arg(format!("--app-name=breadpad"))
        .arg(&title)
        .arg(&note.body);
    for opt in &cfg.settings.snooze_options {
        cmd.arg(format!("--action=snooze_{}={}", opt, humanize_snooze(opt)));
    }
    let output = cmd.output()?;

    // If the user clicked a snooze action, notify-send prints the action key
    if let Ok(action) = String::from_utf8(output.stdout) {
        let action = action.trim();
        if action.starts_with("snooze_") {
            let key = action.trim_start_matches("snooze_");
            if let Some(until) = resolve_snooze(key, cfg) {
                let mut updated = note.clone();
                store.update_note({
                    updated.snoozed_until = Some(until);
                    &updated
                })?;
                Scheduler::schedule(&updated)?;
                return Ok(());
            }
        }
    }

    // Handle recurrence
    if note.rrule.is_some() {
        if let Some(next) = Scheduler::next_recurrence(&note, &cfg.reminders.default_morning) {
            let mut updated = note.clone();
            updated.time = Some(next);
            updated.snoozed_until = None;
            store.update_note(&updated)?;
            Scheduler::schedule(&updated)?;
        }
    }

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
            Some(naive.and_local_timezone(chrono::Local).unwrap().with_timezone(&chrono::Utc))
        }
        _ => None,
    }
}

fn run_popup(preset_type: Option<String>, no_classify: bool, cfg: Config) -> Result<()> {
    // Try to get current Hyprland workspace
    let workspace = get_active_workspace();

    let app = gtk4::Application::builder()
        .application_id("com.breadway.breadpad")
        .build();

    let cfg = Arc::new(cfg);

    app.connect_activate(move |app| {
        let cfg = cfg.clone();
        let workspace = workspace.clone();
        let preset_type = preset_type.clone();
        build_window(app, cfg, workspace, preset_type, no_classify);
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

    // Confirm button
    let confirm_btn = gtk4::Button::builder()
        .label("✓")
        .css_classes(["confirm-button"])
        .build();

    let bottom_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .build();
    bottom_row.append(&chip_box);

    let spacer = gtk4::Box::builder().hexpand(true).build();
    bottom_row.append(&spacer);
    bottom_row.append(&confirm_btn);

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

            // Classify and save synchronously. Tier 1 + 2 finish in <100ms.
            // Tier 3 (Ollama) only fires for ambiguous inputs; the brief pause
            // is acceptable since the user has already committed the note.
            save_note_classified(&text, note_type, no_classify, cfg.clone(), workspace.clone());
            win.close();
        }
    };

    // Confirm button click
    {
        let save = save_and_close.clone();
        confirm_btn.connect_clicked(move |_| save());
    }

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
        let mut classifier = Classifier::load(
            &cfg.model.execution_provider,
            &cfg.reminders.default_morning,
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
    if note.time.is_some() {
        if let Err(e) = Scheduler::schedule(&note) {
            tracing::warn!("failed to schedule reminder: {}", e);
        }
    }
}

fn apply_css(_cfg: &Config) {
    let palette = load_palette();
    let user_css = std::fs::read_to_string(style_css_path()).ok();
    let css = build_css(&palette, user_css.as_deref());

    let provider = gtk4::CssProvider::new();
    provider.load_from_string(&css);
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().unwrap(),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
