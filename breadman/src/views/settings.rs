use breadpad_shared::config::{
    CalendarConfig, Config, ModelConfig, OllamaConfig, RemindersConfig, Settings,
};
use breadpad_shared::types::NoteType;
use bread_theme::adw;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn build(cfg: &Config, on_save: impl Fn(Config) + 'static) -> gtk4::ScrolledWindow {
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    // Plain vertical box (not AdwPreferencesPage) wrapped in our own Clamp —
    // PreferencesPage's built-in clamp caps out around ~600px, far narrower
    // than the rest of breadman's edge-to-edge views. 900px + left-aligned
    // keeps forms readable without the settings screen reading as a
    // different, narrower app bolted onto the side.
    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(24)
        .build();

    // ── General ──────────────────────────────────────────────────
    let general_group = adw::preferences_group("General", None);

    let default_type_row = adw::action_row("Default type", None);
    let type_pill_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(4)
        .valign(gtk4::Align::Center)
        .build();
    let selected_type: Rc<RefCell<String>> = Rc::new(RefCell::new(cfg.settings.default_type.clone()));
    let type_pills: Vec<(gtk4::Button, &'static str)> = NoteType::all_builtin()
        .iter()
        .map(|&name| (bread_theme::gtk::chip(name), name))
        .collect();
    for (btn, name) in &type_pills {
        bread_theme::gtk::set_chip_active(btn, *name == selected_type.borrow().as_str());
        type_pill_box.append(btn);
    }
    default_type_row.add_suffix(&type_pill_box);
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

    let snooze_row = libadwaita::EntryRow::builder()
        .title("Snooze options")
        .show_apply_button(true)
        .build();
    snooze_row.set_text(&cfg.settings.snooze_options.join(", "));
    general_group.add(&snooze_row);

    content.append(&general_group);

    // ── Reminders ────────────────────────────────────────────────
    let rem_group = adw::preferences_group("Reminders", None);

    let morning_row = libadwaita::EntryRow::builder()
        .title("Default morning (used for \"tomorrow_morning\" snoozes and recurring reminders)")
        .show_apply_button(true)
        .build();
    morning_row.set_text(&cfg.reminders.default_morning);
    rem_group.add(&morning_row);

    let grace_adj = gtk4::Adjustment::new(cfg.reminders.missed_grace_minutes as f64, 0.0, 1440.0, 5.0, 30.0, 0.0);
    let grace_row = adw::spin_row("Missed grace (minutes)", Some("How late a reminder can fire before it's considered missed"), &grace_adj);
    rem_group.add(&grace_row);

    content.append(&rem_group);

    // ── Local classifier ───────────────────────────────────────────
    let model_group = adw::preferences_group(
        "Local Classifier",
        Some("Optional local ONNX model for classifying note type/time without a network round-trip. These paths are shared with breadpad — both apps read the same model files."),
    );

    let model_path_row = libadwaita::EntryRow::builder().title("Model path").show_apply_button(true).build();
    model_path_row.set_text(&cfg.model.path);
    model_group.add(&model_path_row);

    let tokenizer_row = libadwaita::EntryRow::builder().title("Tokenizer path").show_apply_button(true).build();
    tokenizer_row.set_text(&cfg.model.tokenizer);
    model_group.add(&tokenizer_row);

    let ort_dylib_row = libadwaita::EntryRow::builder().title("Runtime library path").show_apply_button(true).build();
    ort_dylib_row.set_text(&cfg.model.ort_dylib_path);
    model_group.add(&ort_dylib_row);

    content.append(&model_group);

    // ── AI classification (Ollama) ──────────────────────────────────
    let ollama_group = adw::preferences_group(
        "AI Classification",
        Some("Uses a local Ollama model as a fallback classifier when the ONNX model is unavailable or unsure."),
    );

    let ollama_enabled_row = adw::toggle_row("Enabled", None, cfg.model.ollama.enabled);
    ollama_group.add(&ollama_enabled_row);

    let ollama_endpoint_row = libadwaita::EntryRow::builder().title("Endpoint").show_apply_button(true).build();
    ollama_endpoint_row.set_text(&cfg.model.ollama.endpoint);
    ollama_group.add(&ollama_endpoint_row);

    let ollama_model_row = libadwaita::EntryRow::builder().title("Model").show_apply_button(true).build();
    ollama_model_row.set_text(&cfg.model.ollama.model);
    ollama_group.add(&ollama_model_row);

    let ollama_thresh_adj = gtk4::Adjustment::new(cfg.model.ollama.confidence_threshold as f64, 0.0, 1.0, 0.05, 0.1, 0.0);
    let ollama_thresh_row = adw::spin_row("Confidence threshold", None, &ollama_thresh_adj);
    if let Some(spin) = ollama_thresh_row.first_child().and_downcast::<gtk4::SpinButton>() {
        spin.set_digits(2);
    }
    ollama_group.add(&ollama_thresh_row);

    content.append(&ollama_group);

    // ── Calendar sync ────────────────────────────────────────────
    let cal_group = adw::preferences_group(
        "Calendar Sync",
        Some("Sync reminders to a Nextcloud calendar via CalDAV."),
    );

    let cal_enabled_row = adw::toggle_row("Enabled", None, cfg.calendar.enabled);
    cal_group.add(&cal_enabled_row);

    let cal_url_row = libadwaita::EntryRow::builder().title("Calendar URL").show_apply_button(true).build();
    cal_url_row.set_text(&cfg.calendar.url);
    cal_group.add(&cal_url_row);

    let cal_user_row = libadwaita::EntryRow::builder().title("Username").show_apply_button(true).build();
    cal_user_row.set_text(&cfg.calendar.username);
    cal_group.add(&cal_user_row);

    let cal_pass_row = libadwaita::PasswordEntryRow::builder().title("App password").build();
    cal_pass_row.set_text(&cfg.calendar.password);
    cal_group.add(&cal_pass_row);

    content.append(&cal_group);

    // ── Status (instant-apply — no Save button) ─────────────────
    let status_label = gtk4::Label::builder()
        .label("")
        .xalign(0.0)
        .css_classes(["dim-label"])
        .margin_top(4)
        .build();
    content.append(&status_label);

    // Reads every widget's current value and persists immediately. Every
    // control below calls this on its own "committed a change" signal
    // (switch/combo/spin fire on change; entry rows fire on Enter or their
    // apply-button, via show_apply_button) rather than a single Save button —
    // AdwSwitchRow's whole design language implies changes take effect now.
    let apply_now: Rc<dyn Fn()> = Rc::new({
        let selected_type = selected_type.clone();
        let ws_tag_row = ws_tag_row.clone();
        let archive_adj = archive_adj.clone();
        let snooze_row = snooze_row.clone();
        let morning_row = morning_row.clone();
        let grace_adj = grace_adj.clone();
        let model_path_row = model_path_row.clone();
        let tokenizer_row = tokenizer_row.clone();
        let ort_dylib_row = ort_dylib_row.clone();
        let ollama_enabled_row = ollama_enabled_row.clone();
        let ollama_endpoint_row = ollama_endpoint_row.clone();
        let ollama_model_row = ollama_model_row.clone();
        let ollama_thresh_adj = ollama_thresh_adj.clone();
        let cal_enabled_row = cal_enabled_row.clone();
        let cal_url_row = cal_url_row.clone();
        let cal_user_row = cal_user_row.clone();
        let cal_pass_row = cal_pass_row.clone();
        let status_label = status_label.clone();

        move || {
            let new_cfg = Config {
                settings: Settings {
                    default_type: selected_type.borrow().clone(),
                    workspace_tag: ws_tag_row.is_active(),
                    snooze_options: snooze_row
                        .text()
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                    archive_after_days: archive_adj.value() as i64,
                },
                reminders: RemindersConfig {
                    default_morning: morning_row.text().to_string(),
                    missed_grace_minutes: grace_adj.value() as i64,
                },
                model: ModelConfig {
                    path: model_path_row.text().to_string(),
                    tokenizer: tokenizer_row.text().to_string(),
                    ort_dylib_path: ort_dylib_row.text().to_string(),
                    ollama: OllamaConfig {
                        enabled: ollama_enabled_row.is_active(),
                        endpoint: ollama_endpoint_row.text().to_string(),
                        model: ollama_model_row.text().to_string(),
                        confidence_threshold: ollama_thresh_adj.value() as f32,
                    },
                },
                calendar: CalendarConfig {
                    enabled: cal_enabled_row.is_active(),
                    url: cal_url_row.text().to_string(),
                    username: cal_user_row.text().to_string(),
                    password: cal_pass_row.text().to_string(),
                },
            };
            match new_cfg.save() {
                Ok(()) => {
                    status_label.set_label("Saved.");
                    on_save(new_cfg);
                }
                Err(e) => status_label.set_label(&format!("Save failed: {}", e)),
            }
        }
    });

    // Switches / combo(-pills) / spinners apply the moment they change.
    for (btn, name) in &type_pills {
        let apply_now = apply_now.clone();
        let sel = selected_type.clone();
        let name = *name;
        let all_btns: Vec<gtk4::Button> = type_pills.iter().map(|(b, _)| b.clone()).collect();
        btn.connect_clicked(move |clicked| {
            *sel.borrow_mut() = name.to_string();
            for b in &all_btns { bread_theme::gtk::set_chip_active(b, false); }
            bread_theme::gtk::set_chip_active(clicked, true);
            apply_now();
        });
    }
    macro_rules! apply_on_active {
        ($row:expr) => {
            let apply_now = apply_now.clone();
            $row.connect_active_notify(move |_| apply_now());
        };
    }
    apply_on_active!(ws_tag_row);
    apply_on_active!(ollama_enabled_row);
    apply_on_active!(cal_enabled_row);
    macro_rules! apply_on_value_changed {
        ($adj:expr) => {
            let apply_now = apply_now.clone();
            $adj.connect_value_changed(move |_| apply_now());
        };
    }
    apply_on_value_changed!(archive_adj);
    apply_on_value_changed!(grace_adj);
    apply_on_value_changed!(ollama_thresh_adj);
    // Entry rows: `apply` fires on Enter or the inline apply-button
    // (show_apply_button), which only appears once the text has actually
    // changed — the standard libadwaita instant-apply text-field idiom.
    macro_rules! apply_on_entry {
        ($row:expr) => {
            let apply_now = apply_now.clone();
            $row.connect_apply(move |_| apply_now());
        };
    }
    apply_on_entry!(snooze_row);
    apply_on_entry!(morning_row);
    apply_on_entry!(model_path_row);
    apply_on_entry!(tokenizer_row);
    apply_on_entry!(ort_dylib_row);
    apply_on_entry!(ollama_endpoint_row);
    apply_on_entry!(ollama_model_row);
    apply_on_entry!(cal_url_row);
    apply_on_entry!(cal_user_row);
    apply_on_entry!(cal_pass_row);

    let clamp = libadwaita::Clamp::builder()
        .maximum_size(900)
        .tightening_threshold(700)
        .halign(gtk4::Align::Start)
        .build();
    clamp.set_child(Some(&content));

    let outer = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .margin_start(12)
        .margin_end(12)
        .margin_top(12)
        .margin_bottom(16)
        .build();
    outer.append(&clamp);

    scroll.set_child(Some(&outer));
    scroll
}
