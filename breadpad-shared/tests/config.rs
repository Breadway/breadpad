use breadpad_shared::config::{Config, ModelConfig, RemindersConfig, Settings};
use tempfile::TempDir;

// ---- Default values ----

#[test]
fn default_settings() {
    let s = Settings::default();
    assert_eq!(s.default_type, "note");
    assert!(s.workspace_tag);
    assert_eq!(s.archive_after_days, 30);
}

#[test]
fn default_snooze_options_contains_all_three() {
    let s = Settings::default();
    assert!(s.snooze_options.iter().any(|x| x == "15m"));
    assert!(s.snooze_options.iter().any(|x| x == "1h"));
    assert!(s.snooze_options.iter().any(|x| x == "tomorrow_morning"));
}

#[test]
fn default_model_config() {
    let m = ModelConfig::default();
    assert_eq!(m.execution_provider, "auto");
    assert!(m.path.contains("classifier.onnx"));
    assert!(m.tokenizer.contains("tokenizer.json"));
}

#[test]
fn default_reminders_config() {
    let r = RemindersConfig::default();
    assert_eq!(r.default_morning, "08:00");
    assert_eq!(r.missed_grace_minutes, 60);
}

#[test]
fn default_config_composes_defaults() {
    let cfg = Config::default();
    assert_eq!(cfg.settings.default_type, "note");
    assert_eq!(cfg.model.execution_provider, "auto");
    assert_eq!(cfg.reminders.default_morning, "08:00");
}

// ---- TOML deserialization ----

#[test]
fn full_config_from_toml() {
    let toml = r#"
[settings]
default_type = "todo"
workspace_tag = false
snooze_options = ["15m", "2h"]
archive_after_days = 7

[model]
path = "/tmp/classifier.onnx"
tokenizer = "/tmp/tokenizer.json"
execution_provider = "cpu"

[reminders]
default_morning = "07:30"
missed_grace_minutes = 30
"#;
    let cfg: Config = toml::from_str(toml).unwrap();
    assert_eq!(cfg.settings.default_type, "todo");
    assert!(!cfg.settings.workspace_tag);
    assert_eq!(cfg.settings.snooze_options, vec!["15m", "2h"]);
    assert_eq!(cfg.settings.archive_after_days, 7);
    assert_eq!(cfg.model.execution_provider, "cpu");
    assert_eq!(cfg.model.path, "/tmp/classifier.onnx");
    assert_eq!(cfg.reminders.default_morning, "07:30");
    assert_eq!(cfg.reminders.missed_grace_minutes, 30);
}

#[test]
fn empty_toml_uses_all_defaults() {
    let cfg: Config = toml::from_str("").unwrap();
    assert_eq!(cfg.settings.default_type, "note");
    assert!(cfg.settings.workspace_tag);
    assert_eq!(cfg.model.execution_provider, "auto");
    assert_eq!(cfg.reminders.default_morning, "08:00");
}

#[test]
fn partial_toml_only_settings_section() {
    let toml = r#"
[settings]
default_type = "reminder"
"#;
    let cfg: Config = toml::from_str(toml).unwrap();
    assert_eq!(cfg.settings.default_type, "reminder");
    // Other sections should still have defaults
    assert_eq!(cfg.model.execution_provider, "auto");
    assert_eq!(cfg.reminders.default_morning, "08:00");
}

#[test]
fn partial_toml_only_model_section() {
    let toml = r#"
[model]
execution_provider = "npu"
"#;
    let cfg: Config = toml::from_str(toml).unwrap();
    assert_eq!(cfg.model.execution_provider, "npu");
    assert_eq!(cfg.settings.default_type, "note");
}

#[test]
fn execution_provider_variants_accepted() {
    for ep in &["auto", "npu", "vulkan", "cpu"] {
        let toml = format!("[model]\nexecution_provider = \"{}\"", ep);
        let cfg: Config = toml::from_str(&toml).unwrap();
        assert_eq!(cfg.model.execution_provider, *ep);
    }
}

// ---- TOML serialization round-trip ----

#[test]
fn default_config_serializes_to_valid_toml() {
    let cfg = Config::default();
    let serialized = toml::to_string_pretty(&cfg).unwrap();
    let reparsed: Config = toml::from_str(&serialized).unwrap();
    assert_eq!(reparsed.settings.default_type, cfg.settings.default_type);
    assert_eq!(reparsed.settings.workspace_tag, cfg.settings.workspace_tag);
    assert_eq!(reparsed.model.execution_provider, cfg.model.execution_provider);
    assert_eq!(reparsed.reminders.default_morning, cfg.reminders.default_morning);
}

#[test]
fn custom_config_round_trips() {
    let mut cfg = Config::default();
    cfg.settings.default_type = "idea".into();
    cfg.settings.archive_after_days = 14;
    cfg.model.execution_provider = "vulkan".into();
    cfg.reminders.default_morning = "06:45".into();
    cfg.reminders.missed_grace_minutes = 120;

    let toml = toml::to_string_pretty(&cfg).unwrap();
    let rt: Config = toml::from_str(&toml).unwrap();
    assert_eq!(rt.settings.default_type, "idea");
    assert_eq!(rt.settings.archive_after_days, 14);
    assert_eq!(rt.model.execution_provider, "vulkan");
    assert_eq!(rt.reminders.default_morning, "06:45");
    assert_eq!(rt.reminders.missed_grace_minutes, 120);
}

// ---- Config::save + Config::load ----

#[test]
fn save_and_load_round_trip() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("breadpad.toml");

    let mut cfg = Config::default();
    cfg.settings.default_type = "question".into();
    cfg.model.execution_provider = "cpu".into();
    cfg.reminders.missed_grace_minutes = 45;

    // Manually save to a known path (Config::save uses the fixed XDG path,
    // so we use toml serialization + write here to test the round-trip logic)
    let toml = toml::to_string_pretty(&cfg).unwrap();
    std::fs::write(&config_path, &toml).unwrap();

    let loaded: Config = toml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(loaded.settings.default_type, "question");
    assert_eq!(loaded.model.execution_provider, "cpu");
    assert_eq!(loaded.reminders.missed_grace_minutes, 45);
}

// ---- The example from the README ----

#[test]
fn readme_example_toml_parses() {
    let toml = r#"
[settings]
default_type = "note"
workspace_tag = true
snooze_options = ["15m", "1h", "tomorrow_morning"]
archive_after_days = 30

[model]
path = "~/.local/share/breadpad/model/classifier.onnx"
tokenizer = "~/.local/share/breadpad/model/tokenizer.json"
execution_provider = "auto"

[reminders]
default_morning = "08:00"
missed_grace_minutes = 60
"#;
    let cfg: Config = toml::from_str(toml).unwrap();
    assert_eq!(cfg.settings.default_type, "note");
    assert!(cfg.settings.workspace_tag);
    assert_eq!(cfg.model.execution_provider, "auto");
    assert_eq!(cfg.reminders.default_morning, "08:00");
    assert_eq!(cfg.reminders.missed_grace_minutes, 60);
}
