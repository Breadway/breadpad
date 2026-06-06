use breadpad_shared::config::{expand_path, CalendarConfig, Config, ModelConfig, RemindersConfig, Settings};
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
    assert!(m.path.contains("classifier.onnx"));
    assert!(m.tokenizer.contains("tokenizer.json"));
    assert_eq!(m.ort_dylib_path, "");
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
ort_dylib_path = "/tmp/libonnxruntime.so"

[reminders]
default_morning = "07:30"
missed_grace_minutes = 30
"#;
    let cfg: Config = toml::from_str(toml).unwrap();
    assert_eq!(cfg.settings.default_type, "todo");
    assert!(!cfg.settings.workspace_tag);
    assert_eq!(cfg.settings.snooze_options, vec!["15m", "2h"]);
    assert_eq!(cfg.settings.archive_after_days, 7);
    assert_eq!(cfg.model.path, "/tmp/classifier.onnx");
    assert_eq!(cfg.model.ort_dylib_path, "/tmp/libonnxruntime.so");
    assert_eq!(cfg.reminders.default_morning, "07:30");
    assert_eq!(cfg.reminders.missed_grace_minutes, 30);
}

#[test]
fn empty_toml_uses_all_defaults() {
    let cfg: Config = toml::from_str("").unwrap();
    assert_eq!(cfg.settings.default_type, "note");
    assert!(cfg.settings.workspace_tag);
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
    assert_eq!(cfg.reminders.default_morning, "08:00");
}

// ---- TOML serialization round-trip ----

#[test]
fn default_config_serializes_to_valid_toml() {
    let cfg = Config::default();
    let serialized = toml::to_string_pretty(&cfg).unwrap();
    let reparsed: Config = toml::from_str(&serialized).unwrap();
    assert_eq!(reparsed.settings.default_type, cfg.settings.default_type);
    assert_eq!(reparsed.settings.workspace_tag, cfg.settings.workspace_tag);
    assert_eq!(reparsed.reminders.default_morning, cfg.reminders.default_morning);
}

#[test]
fn custom_config_round_trips() {
    let mut cfg = Config::default();
    cfg.settings.default_type = "idea".into();
    cfg.settings.archive_after_days = 14;
    cfg.reminders.default_morning = "06:45".into();
    cfg.reminders.missed_grace_minutes = 120;

    let toml = toml::to_string_pretty(&cfg).unwrap();
    let rt: Config = toml::from_str(&toml).unwrap();
    assert_eq!(rt.settings.default_type, "idea");
    assert_eq!(rt.settings.archive_after_days, 14);
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
    cfg.reminders.missed_grace_minutes = 45;

    let toml = toml::to_string_pretty(&cfg).unwrap();
    std::fs::write(&config_path, &toml).unwrap();

    let loaded: Config = toml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(loaded.settings.default_type, "question");
    assert_eq!(loaded.reminders.missed_grace_minutes, 45);
}

// ---- The example from the README ----

#[test]
fn example_toml_parses() {
    let toml = r#"
[settings]
default_type = "note"
workspace_tag = true
snooze_options = ["15m", "1h", "tomorrow_morning"]
archive_after_days = 30

[model]
path = "~/.local/share/breadpad/model/classifier.onnx"
tokenizer = "~/.local/share/breadpad/model/tokenizer.json"
ort_dylib_path = ""

[reminders]
default_morning = "08:00"
missed_grace_minutes = 60
"#;
    let cfg: Config = toml::from_str(toml).unwrap();
    assert_eq!(cfg.settings.default_type, "note");
    assert!(cfg.settings.workspace_tag);
    assert_eq!(cfg.reminders.default_morning, "08:00");
    assert_eq!(cfg.reminders.missed_grace_minutes, 60);
}

// ---- CalendarConfig ----

#[test]
fn default_calendar_config_is_disabled() {
    let c = CalendarConfig::default();
    assert!(!c.enabled);
    assert!(c.url.is_empty());
    assert!(c.username.is_empty());
    assert!(c.password.is_empty());
}

#[test]
fn calendar_config_from_toml() {
    let toml = r#"
[calendar]
enabled = true
url = "https://cloud.example.com/remote.php/dav/calendars/user/personal/"
username = "user"
password = "secret"
"#;
    let cfg: Config = toml::from_str(toml).unwrap();
    assert!(cfg.calendar.enabled);
    assert!(cfg.calendar.url.contains("dav/calendars"));
    assert_eq!(cfg.calendar.username, "user");
    assert_eq!(cfg.calendar.password, "secret");
}

#[test]
fn calendar_config_round_trips() {
    let mut cfg = Config::default();
    cfg.calendar.enabled = true;
    cfg.calendar.url = "https://example.com/cal".into();
    cfg.calendar.username = "alice".into();
    cfg.calendar.password = "hunter2".into();

    let toml = toml::to_string_pretty(&cfg).unwrap();
    let rt: Config = toml::from_str(&toml).unwrap();
    assert!(rt.calendar.enabled);
    assert_eq!(rt.calendar.url, "https://example.com/cal");
    assert_eq!(rt.calendar.username, "alice");
    assert_eq!(rt.calendar.password, "hunter2");
}

#[test]
fn default_config_calendar_disabled() {
    let cfg = Config::default();
    assert!(!cfg.calendar.enabled);
}

// ---- OllamaConfig ----

#[test]
fn default_ollama_config_enabled() {
    let m = ModelConfig::default();
    assert!(m.ollama.enabled);
    assert_eq!(m.ollama.endpoint, "http://localhost:11434");
    assert!(!m.ollama.model.is_empty());
    assert!(m.ollama.confidence_threshold > 0.0 && m.ollama.confidence_threshold <= 1.0);
}

#[test]
fn ollama_config_from_toml() {
    let toml = r#"
[model.ollama]
enabled = false
endpoint = "http://localhost:9999"
model = "llama3"
confidence_threshold = 0.8
"#;
    let cfg: Config = toml::from_str(toml).unwrap();
    assert!(!cfg.model.ollama.enabled);
    assert_eq!(cfg.model.ollama.endpoint, "http://localhost:9999");
    assert_eq!(cfg.model.ollama.model, "llama3");
    assert!((cfg.model.ollama.confidence_threshold - 0.8).abs() < 1e-5);
}

// ---- expand_path ----

#[test]
fn expand_path_tilde_prefix_replaced_with_home() {
    let home = dirs::home_dir().unwrap();
    let expanded = expand_path("~/some/path");
    assert!(expanded.starts_with(&home));
    assert!(expanded.ends_with("some/path"));
}

#[test]
fn expand_path_bare_tilde_is_home() {
    let home = dirs::home_dir().unwrap();
    assert_eq!(expand_path("~"), home);
}

#[test]
fn expand_path_absolute_path_unchanged() {
    let p = expand_path("/usr/local/bin/breadpad");
    assert_eq!(p.to_str().unwrap(), "/usr/local/bin/breadpad");
}

#[test]
fn expand_path_relative_path_unchanged() {
    let p = expand_path("relative/path");
    assert_eq!(p.to_str().unwrap(), "relative/path");
}

// ---- ModelConfig::resolved_ort_dylib_path ----

#[test]
fn resolved_ort_dylib_empty_returns_none() {
    let m = ModelConfig::default();
    assert!(m.resolved_ort_dylib_path().is_none());
}

#[test]
fn resolved_ort_dylib_whitespace_only_returns_none() {
    let mut m = ModelConfig::default();
    m.ort_dylib_path = "   ".into();
    assert!(m.resolved_ort_dylib_path().is_none());
}

#[test]
fn resolved_ort_dylib_set_returns_some() {
    let mut m = ModelConfig::default();
    m.ort_dylib_path = "/usr/lib/libonnxruntime.so".into();
    assert_eq!(
        m.resolved_ort_dylib_path().unwrap().to_str().unwrap(),
        "/usr/lib/libonnxruntime.so"
    );
}

// ---- ModelConfig::resolved_paths ----

#[test]
fn resolved_paths_expands_tildes() {
    let m = ModelConfig::default();
    let (model, tokenizer) = m.resolved_paths();
    let home = dirs::home_dir().unwrap();
    assert!(model.starts_with(&home), "model path should be under home: {:?}", model);
    assert!(tokenizer.starts_with(&home), "tokenizer path should be under home: {:?}", tokenizer);
}
