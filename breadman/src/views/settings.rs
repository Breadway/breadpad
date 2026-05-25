use breadpad_shared::config::{
    CalendarConfig, Config, ModelConfig, OllamaConfig, RemindersConfig, Settings,
};
use gtk4::prelude::*;

pub fn build(cfg: &Config, on_save: impl Fn(Config) + 'static) -> gtk4::ScrolledWindow {
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let outer = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(16)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    // ── General ──────────────────────────────────────────────────
    let (general_frame, general_grid) = make_section("General");

    let type_options = ["note", "todo", "reminder", "idea", "question"];
    let default_type_combo = gtk4::DropDown::from_strings(&type_options);
    let dt_idx = type_options
        .iter()
        .position(|&s| s == cfg.settings.default_type.as_str())
        .unwrap_or(0) as u32;
    default_type_combo.set_selected(dt_idx);
    attach_row(&general_grid, 0, "Default type", &default_type_combo);

    let ws_tag_switch = gtk4::Switch::builder()
        .active(cfg.settings.workspace_tag)
        .valign(gtk4::Align::Center)
        .build();
    attach_row(&general_grid, 1, "Workspace tag", &ws_tag_switch);

    let archive_spin = gtk4::SpinButton::with_range(1.0, 365.0, 1.0);
    archive_spin.set_value(cfg.settings.archive_after_days as f64);
    attach_row(&general_grid, 2, "Archive after (days)", &archive_spin);

    let snooze_entry = gtk4::Entry::builder()
        .text(&cfg.settings.snooze_options.join(", "))
        .hexpand(true)
        .build();
    attach_row(&general_grid, 3, "Snooze options", &snooze_entry);

    outer.append(&general_frame);

    // ── Reminders ────────────────────────────────────────────────
    let (rem_frame, rem_grid) = make_section("Reminders");

    let morning_entry = gtk4::Entry::builder()
        .text(&cfg.reminders.default_morning)
        .placeholder_text("HH:MM")
        .build();
    attach_row(&rem_grid, 0, "Default morning", &morning_entry);

    let grace_spin = gtk4::SpinButton::with_range(0.0, 1440.0, 5.0);
    grace_spin.set_value(cfg.reminders.missed_grace_minutes as f64);
    attach_row(&rem_grid, 1, "Missed grace (minutes)", &grace_spin);

    outer.append(&rem_frame);

    // ── Model ─────────────────────────────────────────────────────
    let (model_frame, model_grid) = make_section("Model (Tier 2 ONNX)");

    let model_path_entry = gtk4::Entry::builder()
        .text(&cfg.model.path)
        .hexpand(true)
        .build();
    attach_row(&model_grid, 0, "ONNX path", &model_path_entry);

    let tokenizer_entry = gtk4::Entry::builder()
        .text(&cfg.model.tokenizer)
        .hexpand(true)
        .build();
    attach_row(&model_grid, 1, "Tokenizer path", &tokenizer_entry);

    let ep_options = ["auto", "npu", "vulkan", "cpu"];
    let ep_combo = gtk4::DropDown::from_strings(&ep_options);
    let ep_idx = ep_options
        .iter()
        .position(|&s| s == cfg.model.execution_provider.as_str())
        .unwrap_or(0) as u32;
    ep_combo.set_selected(ep_idx);
    attach_row(&model_grid, 2, "Execution provider", &ep_combo);

    outer.append(&model_frame);

    // ── Ollama (Tier 3) ───────────────────────────────────────────
    let (ollama_frame, ollama_grid) = make_section("Ollama (Tier 3)");

    let ollama_enabled = gtk4::Switch::builder()
        .active(cfg.model.ollama.enabled)
        .valign(gtk4::Align::Center)
        .build();
    attach_row(&ollama_grid, 0, "Enabled", &ollama_enabled);

    let ollama_endpoint = gtk4::Entry::builder()
        .text(&cfg.model.ollama.endpoint)
        .hexpand(true)
        .build();
    attach_row(&ollama_grid, 1, "Endpoint", &ollama_endpoint);

    let ollama_model = gtk4::Entry::builder()
        .text(&cfg.model.ollama.model)
        .build();
    attach_row(&ollama_grid, 2, "Model", &ollama_model);

    let ollama_thresh = gtk4::SpinButton::with_range(0.0, 1.0, 0.05);
    ollama_thresh.set_value(cfg.model.ollama.confidence_threshold as f64);
    ollama_thresh.set_digits(2);
    attach_row(&ollama_grid, 3, "Confidence threshold", &ollama_thresh);

    outer.append(&ollama_frame);

    // ── Calendar ─────────────────────────────────────────────────
    let (cal_frame, cal_grid) = make_section("Nextcloud Calendar (CalDAV)");

    let cal_enabled = gtk4::Switch::builder()
        .active(cfg.calendar.enabled)
        .valign(gtk4::Align::Center)
        .build();
    attach_row(&cal_grid, 0, "Enabled", &cal_enabled);

    let cal_url = gtk4::Entry::builder()
        .text(&cfg.calendar.url)
        .placeholder_text("https://nextcloud.example.com/remote.php/dav/calendars/you/personal/")
        .hexpand(true)
        .build();
    attach_row(&cal_grid, 1, "Calendar URL", &cal_url);

    let cal_user = gtk4::Entry::builder()
        .text(&cfg.calendar.username)
        .build();
    attach_row(&cal_grid, 2, "Username", &cal_user);

    let cal_pass = gtk4::Entry::builder()
        .text(&cfg.calendar.password)
        .input_purpose(gtk4::InputPurpose::Password)
        .visibility(false)
        .build();
    attach_row(&cal_grid, 3, "App password", &cal_pass);

    outer.append(&cal_frame);

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
        let dtc = default_type_combo.clone();
        let wts = ws_tag_switch.clone();
        let ars = archive_spin.clone();
        let sne = snooze_entry.clone();
        let moe = morning_entry.clone();
        let grs = grace_spin.clone();
        let mpe = model_path_entry.clone();
        let tke = tokenizer_entry.clone();
        let epc = ep_combo.clone();
        let oec = ollama_enabled.clone();
        let oee = ollama_endpoint.clone();
        let ome = ollama_model.clone();
        let ots = ollama_thresh.clone();
        let cec = cal_enabled.clone();
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
                    execution_provider: ep_options
                        .get(epc.selected() as usize)
                        .copied()
                        .unwrap_or("auto")
                        .to_string(),
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
        .build();
    btn_row.append(&status_label);
    btn_row.append(&gtk4::Box::builder().hexpand(true).build());
    btn_row.append(&save_btn);
    outer.append(&btn_row);

    scroll.set_child(Some(&outer));
    scroll
}

fn make_section(title: &str) -> (gtk4::Frame, gtk4::Grid) {
    let frame = gtk4::Frame::builder().label(title).build();
    let grid = gtk4::Grid::builder()
        .row_spacing(8)
        .column_spacing(16)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    frame.set_child(Some(&grid));
    (frame, grid)
}

fn attach_row(grid: &gtk4::Grid, row: i32, label: &str, widget: &impl gtk4::prelude::IsA<gtk4::Widget>) {
    let lbl = gtk4::Label::builder()
        .label(label)
        .xalign(0.0)
        .hexpand(false)
        .width_chars(24)
        .build();
    grid.attach(&lbl, 0, row, 1, 1);
    grid.attach(widget, 1, row, 1, 1);
}
