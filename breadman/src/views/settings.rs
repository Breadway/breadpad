//! Settings screen. Deliberately plain GTK4, not libadwaita's AdwActionRow/
//! AdwSpinRow/AdwEntryRow family — those ran noticeably taller than the rest
//! of the app and don't expose a way to constrain the internal spin
//! button's width from the outside (it stretches to fill whatever room the
//! row has, leaving the digits and +/- buttons stranded behind a huge empty
//! bordered box once the row is wider than libadwaita's usual ~400-600px
//! home turf). Instead this mirrors bos-settings' own Row.svelte /
//! NumberField.svelte / TextField.svelte design exactly — same tokens
//! (12/16px row padding, ch-width inputs, transparent-at-rest border) — so
//! the two settings screens in the ecosystem actually agree with each other.

use breadpad_shared::config::{
    CalendarConfig, Config, ModelConfig, OllamaConfig, RemindersConfig, Settings,
};
use breadpad_shared::types::NoteType;
use gtk4::{glib, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;

/// A titled group: heading, optional description, then a `.boxed-list` of
/// `field_row`s (native GTK4 rounded-corner-run + divider styling).
fn field_group(title: &str, description: Option<&str>) -> (gtk4::Box, gtk4::ListBox) {
    let outer = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(8).build();

    let heading = gtk4::Label::builder().label(title).xalign(0.0).css_classes(["heading"]).build();
    outer.append(&heading);

    if let Some(desc) = description {
        let desc_label = gtk4::Label::builder()
            .label(desc)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        outer.append(&desc_label);
    }

    let list = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    outer.append(&list);

    (outer, list)
}

/// A single row: label (+ optional subtitle) on the left, one control on
/// the right — same shape as bos-settings' `Row.svelte`.
fn field_row(label: &str, subtitle: Option<&str>, control: &impl IsA<gtk4::Widget>) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::builder()
        .selectable(false)
        .activatable(false)
        .css_classes(["field-row"])
        .build();

    let hbox = gtk4::Box::builder().orientation(gtk4::Orientation::Horizontal).spacing(16).build();

    let label_box = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).hexpand(true).valign(gtk4::Align::Center).build();
    label_box.append(&gtk4::Label::builder().label(label).xalign(0.0).build());
    if let Some(sub) = subtitle {
        label_box.append(&gtk4::Label::builder().label(sub).xalign(0.0).wrap(true).css_classes(["field-row-subtitle"]).build());
    }
    hbox.append(&label_box);
    hbox.append(control);

    row.set_child(Some(&hbox));
    row
}

fn text_entry(text: &str, width_chars: i32) -> gtk4::Entry {
    gtk4::Entry::builder().text(text).width_chars(width_chars).valign(gtk4::Align::Center).css_classes(["field-input"]).build()
}

fn spin_button(value: f64, min: f64, max: f64, step: f64, page: f64, digits: u32) -> gtk4::SpinButton {
    let adj = gtk4::Adjustment::new(value, min, max, step, page, 0.0);
    gtk4::SpinButton::builder().adjustment(&adj).digits(digits).width_chars(8).valign(gtk4::Align::Center).css_classes(["field-input"]).build()
}

pub fn build(cfg: &Config, on_save: impl Fn(Config) + 'static) -> gtk4::ScrolledWindow {
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let content = gtk4::Box::builder().orientation(gtk4::Orientation::Vertical).spacing(24).build();

    // ── General ──────────────────────────────────────────────────
    let (general_group, general_list) = field_group("General", None);

    let type_pill_box = gtk4::Box::builder().orientation(gtk4::Orientation::Horizontal).spacing(4).valign(gtk4::Align::Center).build();
    let selected_type: Rc<RefCell<String>> = Rc::new(RefCell::new(cfg.settings.default_type.clone()));
    let type_pills: Vec<(gtk4::Button, &'static str)> = NoteType::all_builtin()
        .iter()
        .map(|&name| (crate::theme_widgets::chip(name), name))
        .collect();
    for (btn, name) in &type_pills {
        crate::theme_widgets::set_chip_active(btn, *name == selected_type.borrow().as_str());
        type_pill_box.append(btn);
    }
    general_list.append(&field_row("Default type", None, &type_pill_box));

    let ws_tag_switch = gtk4::Switch::builder().active(cfg.settings.workspace_tag).valign(gtk4::Align::Center).build();
    general_list.append(&field_row(
        "Workspace tag",
        Some("Tag new notes with the Hyprland workspace they were created on"),
        &ws_tag_switch,
    ));

    let archive_spin = spin_button(cfg.settings.archive_after_days as f64, 1.0, 365.0, 1.0, 7.0, 0);
    general_list.append(&field_row("Archive after (days)", None, &archive_spin));

    let snooze_entry = text_entry(&cfg.settings.snooze_options.join(", "), 24);
    general_list.append(&field_row("Snooze options", Some("Comma-separated (e.g. 15m, 1h, tomorrow_morning)"), &snooze_entry));

    content.append(&general_group);

    // ── Reminders ────────────────────────────────────────────────
    let (rem_group, rem_list) = field_group("Reminders", None);

    let morning_entry = text_entry(&cfg.reminders.default_morning, 10);
    rem_list.append(&field_row("Default morning", Some("Used for \"tomorrow_morning\" snoozes and recurring reminders"), &morning_entry));

    let grace_spin = spin_button(cfg.reminders.missed_grace_minutes as f64, 0.0, 1440.0, 5.0, 30.0, 0);
    rem_list.append(&field_row("Missed grace (minutes)", Some("How late a reminder can fire before it's considered missed"), &grace_spin));

    content.append(&rem_group);

    // ── Local classifier ───────────────────────────────────────────
    let (model_group, model_list) = field_group(
        "Local Classifier",
        Some("Optional local ONNX model for classifying note type/time without a network round-trip. These paths are shared with breadpad — both apps read the same model files."),
    );

    let model_path_entry = text_entry(&cfg.model.path, 30);
    model_list.append(&field_row("Model path", None, &model_path_entry));

    let tokenizer_entry = text_entry(&cfg.model.tokenizer, 30);
    model_list.append(&field_row("Tokenizer path", None, &tokenizer_entry));

    let ort_dylib_entry = text_entry(&cfg.model.ort_dylib_path, 30);
    model_list.append(&field_row("Runtime library path", None, &ort_dylib_entry));

    content.append(&model_group);

    // ── AI classification (Ollama) ──────────────────────────────────
    let (ollama_group, ollama_list) = field_group(
        "AI Classification",
        Some("Uses a local Ollama model as a fallback classifier when the ONNX model is unavailable or unsure."),
    );

    let ollama_enabled_switch = gtk4::Switch::builder().active(cfg.model.ollama.enabled).valign(gtk4::Align::Center).build();
    ollama_list.append(&field_row("Enabled", None, &ollama_enabled_switch));

    let ollama_endpoint_entry = text_entry(&cfg.model.ollama.endpoint, 24);
    ollama_list.append(&field_row("Endpoint", None, &ollama_endpoint_entry));

    let ollama_model_entry = text_entry(&cfg.model.ollama.model, 16);
    ollama_list.append(&field_row("Model", None, &ollama_model_entry));

    let ollama_thresh_spin = spin_button(cfg.model.ollama.confidence_threshold as f64, 0.0, 1.0, 0.05, 0.1, 2);
    ollama_list.append(&field_row("Confidence threshold", None, &ollama_thresh_spin));

    content.append(&ollama_group);

    // ── Calendar sync ────────────────────────────────────────────
    let (cal_group, cal_list) = field_group("Calendar Sync", Some("Sync reminders to a Nextcloud calendar via CalDAV."));

    let cal_enabled_switch = gtk4::Switch::builder().active(cfg.calendar.enabled).valign(gtk4::Align::Center).build();
    cal_list.append(&field_row("Enabled", None, &cal_enabled_switch));

    let cal_url_entry = text_entry(&cfg.calendar.url, 30);
    cal_list.append(&field_row("Calendar URL", None, &cal_url_entry));

    let cal_user_entry = text_entry(&cfg.calendar.username, 16);
    cal_list.append(&field_row("Username", None, &cal_user_entry));

    let cal_pass_entry = gtk4::PasswordEntry::builder().text(&cfg.calendar.password).show_peek_icon(true).valign(gtk4::Align::Center).css_classes(["field-input"]).build();
    cal_list.append(&field_row("App password", None, &cal_pass_entry));

    content.append(&cal_group);

    // ── Status (instant-apply — no Save button) ─────────────────
    let status_label = gtk4::Label::builder().label("").xalign(0.0).css_classes(["dim-label"]).margin_top(4).build();
    content.append(&status_label);

    // Reads every widget's current value and persists immediately. Every
    // control below calls this on its own "committed a change" signal
    // (switch/spin fire on change; entries fire on Enter or focus-out).
    let apply_now: Rc<dyn Fn()> = Rc::new({
        let selected_type = selected_type.clone();
        let ws_tag_switch = ws_tag_switch.clone();
        let archive_spin = archive_spin.clone();
        let snooze_entry = snooze_entry.clone();
        let morning_entry = morning_entry.clone();
        let grace_spin = grace_spin.clone();
        let model_path_entry = model_path_entry.clone();
        let tokenizer_entry = tokenizer_entry.clone();
        let ort_dylib_entry = ort_dylib_entry.clone();
        let ollama_enabled_switch = ollama_enabled_switch.clone();
        let ollama_endpoint_entry = ollama_endpoint_entry.clone();
        let ollama_model_entry = ollama_model_entry.clone();
        let ollama_thresh_spin = ollama_thresh_spin.clone();
        let cal_enabled_switch = cal_enabled_switch.clone();
        let cal_url_entry = cal_url_entry.clone();
        let cal_user_entry = cal_user_entry.clone();
        let cal_pass_entry = cal_pass_entry.clone();
        let status_label = status_label.clone();

        move || {
            let new_cfg = Config {
                settings: Settings {
                    default_type: selected_type.borrow().clone(),
                    workspace_tag: ws_tag_switch.is_active(),
                    snooze_options: snooze_entry
                        .text()
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                    archive_after_days: archive_spin.value() as i64,
                },
                reminders: RemindersConfig {
                    default_morning: morning_entry.text().to_string(),
                    missed_grace_minutes: grace_spin.value() as i64,
                },
                model: ModelConfig {
                    path: model_path_entry.text().to_string(),
                    tokenizer: tokenizer_entry.text().to_string(),
                    ort_dylib_path: ort_dylib_entry.text().to_string(),
                    ollama: OllamaConfig {
                        enabled: ollama_enabled_switch.is_active(),
                        endpoint: ollama_endpoint_entry.text().to_string(),
                        model: ollama_model_entry.text().to_string(),
                        confidence_threshold: ollama_thresh_spin.value() as f32,
                    },
                },
                calendar: CalendarConfig {
                    enabled: cal_enabled_switch.is_active(),
                    url: cal_url_entry.text().to_string(),
                    username: cal_user_entry.text().to_string(),
                    password: cal_pass_entry.text().to_string(),
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

    // Type pills, switches, spinners apply the moment they change.
    for (btn, name) in &type_pills {
        let apply_now = apply_now.clone();
        let sel = selected_type.clone();
        let name = *name;
        let all_btns: Vec<gtk4::Button> = type_pills.iter().map(|(b, _)| b.clone()).collect();
        btn.connect_clicked(move |clicked| {
            *sel.borrow_mut() = name.to_string();
            for b in &all_btns { crate::theme_widgets::set_chip_active(b, false); }
            crate::theme_widgets::set_chip_active(clicked, true);
            apply_now();
        });
    }
    macro_rules! apply_on_active {
        ($sw:expr) => {
            let apply_now = apply_now.clone();
            $sw.connect_state_set(move |_, _| { apply_now(); glib::Propagation::Proceed });
        };
    }
    apply_on_active!(ws_tag_switch);
    apply_on_active!(ollama_enabled_switch);
    apply_on_active!(cal_enabled_switch);
    macro_rules! apply_on_value_changed {
        ($spin:expr) => {
            let apply_now = apply_now.clone();
            $spin.connect_value_changed(move |_| apply_now());
        };
    }
    apply_on_value_changed!(archive_spin);
    apply_on_value_changed!(grace_spin);
    apply_on_value_changed!(ollama_thresh_spin);

    // Entries: apply on Enter, and on focus-out so a click-away doesn't
    // silently discard the edit.
    macro_rules! apply_on_entry {
        ($entry:expr) => {
            let apply_now_activate = apply_now.clone();
            $entry.connect_activate(move |_| apply_now_activate());
            let apply_now_focus = apply_now.clone();
            let focus = gtk4::EventControllerFocus::new();
            focus.connect_leave(move |_| apply_now_focus());
            $entry.add_controller(focus);
        };
    }
    apply_on_entry!(snooze_entry);
    apply_on_entry!(morning_entry);
    apply_on_entry!(model_path_entry);
    apply_on_entry!(tokenizer_entry);
    apply_on_entry!(ort_dylib_entry);
    apply_on_entry!(ollama_endpoint_entry);
    apply_on_entry!(ollama_model_entry);
    apply_on_entry!(cal_url_entry);
    apply_on_entry!(cal_user_entry);
    apply_on_entry!(cal_pass_entry);

    content.set_halign(gtk4::Align::Start);
    content.set_size_request(900, -1);

    let outer = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .margin_start(12)
        .margin_end(12)
        .margin_top(12)
        .margin_bottom(16)
        .build();
    outer.append(&content);

    scroll.set_child(Some(&outer));
    scroll
}
