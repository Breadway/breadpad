use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn default_type_str() -> String { "note".into() }
fn default_workspace_tag() -> bool { true }
fn default_snooze_options() -> Vec<String> {
    vec!["15m".into(), "1h".into(), "tomorrow_morning".into()]
}
fn default_archive_after_days() -> i64 { 30 }
fn default_model_path() -> String { "~/.local/share/breadpad/model/classifier.onnx".into() }
fn default_tokenizer_path() -> String { "~/.local/share/breadpad/model/tokenizer.json".into() }
fn default_ort_dylib_path() -> String { "".into() }
fn default_morning_time() -> String { "08:00".into() }
fn default_missed_grace_minutes() -> i64 { 60 }
fn default_ollama_endpoint() -> String { "http://localhost:11434".into() }
fn default_ollama_model() -> String { "fastflowlm".into() }
fn default_ollama_confidence_threshold() -> f32 { 0.6 }
fn default_ollama_enabled() -> bool { true }
fn default_calendar_enabled() -> bool { false }

pub fn expand_path(path: &str) -> PathBuf {
    if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(path)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_type_str")]
    pub default_type: String,
    #[serde(default = "default_workspace_tag")]
    pub workspace_tag: bool,
    #[serde(default = "default_snooze_options")]
    pub snooze_options: Vec<String>,
    #[serde(default = "default_archive_after_days")]
    pub archive_after_days: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            default_type: default_type_str(),
            workspace_tag: default_workspace_tag(),
            snooze_options: default_snooze_options(),
            archive_after_days: default_archive_after_days(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    #[serde(default = "default_ollama_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_ollama_model")]
    pub model: String,
    #[serde(default = "default_ollama_confidence_threshold")]
    pub confidence_threshold: f32,
    #[serde(default = "default_ollama_enabled")]
    pub enabled: bool,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        OllamaConfig {
            endpoint: default_ollama_endpoint(),
            model: default_ollama_model(),
            confidence_threshold: default_ollama_confidence_threshold(),
            enabled: default_ollama_enabled(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    #[serde(default = "default_model_path")]
    pub path: String,
    #[serde(default = "default_tokenizer_path")]
    pub tokenizer: String,
    /// Path to `libonnxruntime.so`. Auto-discovered when empty.
    #[serde(default = "default_ort_dylib_path")]
    pub ort_dylib_path: String,
    #[serde(default)]
    pub ollama: OllamaConfig,
}

impl Default for ModelConfig {
    fn default() -> Self {
        ModelConfig {
            path: default_model_path(),
            tokenizer: default_tokenizer_path(),
            ort_dylib_path: default_ort_dylib_path(),
            ollama: OllamaConfig::default(),
        }
    }
}

impl ModelConfig {
    pub fn resolved_paths(&self) -> (PathBuf, PathBuf) {
        (expand_path(&self.path), expand_path(&self.tokenizer))
    }

    pub fn resolved_ort_dylib_path(&self) -> Option<PathBuf> {
        let raw = self.ort_dylib_path.trim();
        if raw.is_empty() {
            return None;
        }
        Some(expand_path(raw))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemindersConfig {
    #[serde(default = "default_morning_time")]
    pub default_morning: String,
    #[serde(default = "default_missed_grace_minutes")]
    pub missed_grace_minutes: i64,
}

impl Default for RemindersConfig {
    fn default() -> Self {
        RemindersConfig {
            default_morning: default_morning_time(),
            missed_grace_minutes: default_missed_grace_minutes(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConfig {
    #[serde(default = "default_calendar_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub username: String,
    /// WARNING: stored as plaintext in breadpad.toml. Restrict the file's permissions
    /// (`chmod 600 ~/.config/breadpad/breadpad.toml`) and keep it out of version control.
    /// A future release may support reading the password from the OS secret service instead.
    #[serde(default)]
    pub password: String,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        CalendarConfig {
            enabled: false,
            url: String::new(),
            username: String::new(),
            password: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub model: ModelConfig,
    #[serde(default)]
    pub reminders: RemindersConfig,
    #[serde(default)]
    pub calendar: CalendarConfig,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            let cfg = Config::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let text = fs::read_to_string(&path)?;
        let cfg: Config = toml::from_str(&text)?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        fs::write(&path, text)?;
        Ok(())
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("breadpad")
        .join("breadpad.toml")
}

pub fn style_css_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("breadpad")
        .join("style.css")
}
