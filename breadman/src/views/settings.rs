use breadpad_shared::config::{
    CalendarConfig, Config, ModelConfig, OllamaConfig, RemindersConfig, Settings,
};
use bread_theme::adw;
use gtk4::prelude::*;
use libadwaita::prelude::*;

pub fn build(cfg: &Config, on_save: impl Fn(Config) + 'static) -> gtk4::ScrolledWindow {
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let page = libadwaita::PreferencesPage::new();

    // ── General ──────────────────────────────────────────────────
    let general_group = adw::preferences_group("General", None);

    let type_options = ["note", "todo", "reminder", "idea", "question"];
    let default_type_row = libadwaita::ComboRow::builder()
        .title("Default type")
        .model(&gtk4::StringList::new(&type_options))
        .build();
    let dt_idx = type_options
        .iter()
        .position(|&s| s == cfg.settings.default_type.as_str())
        .unwrap_or(0) as u32;
    default_type_row.set_selected(dt_idx);
    general_group.add(&default_type_row);

    let ws_tag_row = adw::toggle_row(
        "Workspace tag",
        Some("Tag new notes with the Hyprland workspace they were created on"),
        cfg.settings.workspace_tag,
    );
    general_group.add(&ws_tag_row);

    let archive_adj = gtk4::Adjustment::new(cfg.settings.archive_after_days as f64, 1.0, 365.0, 1.0, 7.0, 0.0);
    let archive_row = adw::spin_row("Archive after (days)", None, &archive_adj);
    general_group.add(&archive_row);

    let snooze_row = adw::action_row("Snooze options", Some("Comma-separated (e.g. 15m, 1h, tomorrow_morning)"));
    let snooze_entry = gtk4::Entry::builder()
        .text(cfg.settings.snooze_options.join(", "))
        .valign(gtk4::Align::Center)
        .build();
    snooze_row.add_suffix(&snooze_entry);
    general_group.add(&snooze_row);

    page.add(&general_group);

    // ── Reminders ────────────────────────────────────────────────
    let rem_group = adw::preferences_group("Reminders", None);

    let morning_row = adw::action_row("Default morning", Some("Used for \"tomorrow_morning\" snoozes and recurring reminders"));
    let morning_entry = gtk4::Entry::builder()
        .text(&cfg.reminders.default_morning)
        .placeholder_text("HH:MM")
        .valign(gtk4::Align::Center)
        .build();
    morning_row.add_suffix(&morning_entry);
    rem_group.add(&morning_row);

    let grace_adj = gtk4::Adjustment::new(cfg.reminders.missed_grace_minutes as f64, 0.0, 1440.0, 5.0, 30.0, 0.0);
    let grace_row = adw::spin_row("Missed grace (minutes)", Some("How late a reminder can fire before it's considered missed"), &grace_adj);
    rem_group.add(&grace_row);

    page.add(&rem_group);

    // ── Local classifier ───────────────────────────────────────────
    let model_group = adw::preferences_group(
        "Local Classifier",
        Some("Optional local ONNX model for classifying note type/time without a network round-trip."),
    );

    let model_path_row = adw::action_row("Model path", None);
    let model_path_entry = gtk4::Entry::builder().text(&cfg.model.path).hexpand(true).width_chars(36).valign(gtk4::Align::Center).build();
    model_path_row.add_suffix(&model_path_entry);
    model_group.add(&model_path_row);

    let tokenizer_row = adw::action_row("Tokenizer path", None);
    let tokenizer_entry = gtk4::Entry::builder().text(&cfg.model.tokenizer).hexpand(true).width_chars(36).valign(gtk4::Align::Center).build();
    tokenizer_row.add_suffix(&tokenizer_entry);
    model_group.add(&tokenizer_row);

    let ort_dylib_row = adw::action_row("Runtime library path", None);
    let ort_dylib_entry = gtk4::Entry::builder().text(&cfg.model.ort_dylib_path).hexpand(true).width_chars(36).valign(gtk4::Align::Center).build();
    ort_dylib_row.add_suffix(&ort_dylib_entry);
    model_group.add(&ort_dylib_row);

    page.add(&model_group);

    // ── AI classification (Ollama) ──────────────────────────────────
    let ollama_group = adw::preferences_group(
        "AI Classification",
        Some("Uses a local Ollama model as a fallback classifier when the ONNX model is unavailable or unsure."),
    );

    let ollama_enabled_row = adw::toggle_row("Enabled", None, cfg.model.ollama.enabled);
    ollama_group.add(&ollama_enabled_row);

    let ollama_endpoint_row = adw::action_row("Endpoint", None);
    let ollama_endpoint_entry = gtk4::Entry::builder().text(&cfg.model.ollama.endpoint).hexpand(true).width_chars(36).valign(gtk4::Align::Center).build();
    ollama_endpoint_row.add_suffix(&ollama_endpoint_entry);
    ollama_group.add(&ollama_endpoint_row);

    let ollama_model_row = adw::action_row("Model", None);
    let ollama_model_entry = gtk4::Entry::builder().text(&cfg.model.ollama.model).valign(gtk4::Align::Center).build();
    ollama_model_row.add_suffix(&ollama_model_entry);
    ollama_group.add(&ollama_model_row);

    let ollama_thresh_adj = gtk4::Adjustment::new(cfg.model.ollama.confidence_threshold as f64, 0.0, 1.0, 0.05, 0.1, 0.0);
    let ollama_thresh_row = adw::spin_row("Confidence threshold", None, &ollama_thresh_adj);
    if let Some(spin) = ollama_thresh_row.first_child().and_downcast::<gtk4::SpinButton>() {
        spin.set_digits(2);
    }
    ollama_group.add(&ollama_thresh_row);

    page.add(&ollama_group);

    // ── Calendar sync ────────────────────────────────────────────
    let cal_group = adw::preferences_group(
        "Calendar Sync",
        Some("Sync reminders to a Nextcloud calendar via CalDAV."),
    );

    let cal_enabled_row = adw::toggle_row("Enabled", None, cfg.calendar.enabled);
    cal_group.add(&cal_enabled_row);

    let cal_url_row = adw::action_row("Calendar URL", None);
    let cal_url = gtk4::Entry::builder()
        .text(&cfg.calendar.url)
        .placeholder_text("https://nextcloud.example.com/remote.php/dav/calendars/you/personal/")
        .hexpand(true)
        .width_chars(36)
        .valign(gtk4::Align::Center)
        .build();
    cal_url_row.add_suffix(&cal_url);
    cal_group.add(&cal_url_row);

    let cal_user_row = adw::action_row("Username", None);
    let cal_user = gtk4::Entry::builder().text(&cfg.calendar.username).valign(gtk4::Align::Center).build();
    cal_user_row.add_suffix(&cal_user);
    cal_group.add(&cal_user_row);

    let cal_pass_row = adw::action_row("App password", None);
    let cal_pass = gtk4::PasswordEntry::builder()
        .text(&cfg.calendar.password)
        .show_peek_icon(true)
        .valign(gtk4::Align::Center)
        .build();
    cal_pass_row.add_suffix(&cal_pass);
    cal_group.add(&cal_pass_row);

    page.add(&cal_group);

    // ── Save ──────────────────────────────────────────────────────
    let status_label = gtk4::Label::builder()
        .label("")
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    let save_btn = gtk4::Button::builder()
        .label("Save Settings")
        .css_classes(["confirm-button"])
        .halign(gtk4::Align::End)
        .build();

    {
        let dtc = default_type_row.clone();
        let wts = ws_tag_row.clone();
        let ars = archive_adj.clone();
        let sne = snooze_entry.clone();
        let moe = morning_entry.clone();
        let grs = grace_adj.clone();
        let mpe = model_path_entry.clone();
        let tke = tokenizer_entry.clone();
        let ode = ort_dylib_entry.clone();
        let oec = ollama_enabled_row.clone();
        let oee = ollama_endpoint_entry.clone();
        let ome = ollama_model_entry.clone();
        let ots = ollama_thresh_adj.clone();
        let cec = cal_enabled_row.clone();
        let cuc = cal_url.clone();
        let csc = cal_user.clone();
        let cpc = cal_pass.clone();
        let sl = status_label.clone();

        save_btn.connect_clicked(move |_| {
            let new_cfg = Config {
                settings: Settings {
                    default_type: type_options
                        .get(dtc.selected() as usize)
                        .copied()
                        .unwrap_or("note")
                        .to_string(),
                    workspace_tag: wts.is_active(),
                    snooze_options: sne
                        .text()
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                    archive_after_days: ars.value() as i64,
                },
                reminders: RemindersConfig {
                    default_morning: moe.text().to_string(),
                    missed_grace_minutes: grs.value() as i64,
                },
                model: ModelConfig {
                    path: mpe.text().to_string(),
                    tokenizer: tke.text().to_string(),
                    ort_dylib_path: ode.text().to_string(),
                    ollama: OllamaConfig {
                        enabled: oec.is_active(),
                        endpoint: oee.text().to_string(),
                        model: ome.text().to_string(),
                        confidence_threshold: ots.value() as f32,
                    },
                },
                calendar: CalendarConfig {
                    enabled: cec.is_active(),
                    url: cuc.text().to_string(),
                    username: csc.text().to_string(),
                    password: cpc.text().to_string(),
                },
            };
            match new_cfg.save() {
                Ok(()) => {
                    sl.set_label("Settings saved.");
                    on_save(new_cfg);
                }
                Err(e) => sl.set_label(&format!("Save failed: {}", e)),
            }
        });
    }

    let btn_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .margin_top(16)
        .margin_start(16)
        .margin_end(16)
        .margin_bottom(16)
        .build();
    btn_row.append(&status_label);
    btn_row.append(&gtk4::Box::builder().hexpand(true).build());
    btn_row.append(&save_btn);

    let outer = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).build();
    outer.append(&page);
    outer.append(&btn_row);

    scroll.set_child(Some(&outer));
    scroll
}
