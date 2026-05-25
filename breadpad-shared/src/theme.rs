use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Palette {
    pub background: String,
    pub foreground: String,
    pub color0: String,
    pub color1: String,
    pub color2: String,
    pub color3: String,
    pub color4: String,
    pub color5: String,
    pub color6: String,
    pub color7: String,
}

// Catppuccin Mocha fallback
impl Default for Palette {
    fn default() -> Self {
        Palette {
            background: "#1e1e2e".into(),
            foreground: "#cdd6f4".into(),
            color0: "#45475a".into(),
            color1: "#f38ba8".into(),
            color2: "#a6e3a1".into(),
            color3: "#f9e2af".into(),
            color4: "#89b4fa".into(),
            color5: "#f5c2e7".into(),
            color6: "#94e2d5".into(),
            color7: "#bac2de".into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct WalColors {
    #[serde(default)]
    colors: HashMap<String, String>,
    special: Option<WalSpecial>,
}

#[derive(Debug, Deserialize)]
struct WalSpecial {
    background: Option<String>,
    foreground: Option<String>,
}

pub(crate) fn palette_from_wal_json(json: &str) -> Option<Palette> {
    let wal: WalColors = serde_json::from_str(json).ok()?;
    Some(Palette {
        background: wal.special.as_ref().and_then(|s| s.background.clone()).unwrap_or_else(|| "#1e1e2e".into()),
        foreground: wal.special.as_ref().and_then(|s| s.foreground.clone()).unwrap_or_else(|| "#cdd6f4".into()),
        color0: wal.colors.get("color0").cloned().unwrap_or_else(|| "#45475a".into()),
        color1: wal.colors.get("color1").cloned().unwrap_or_else(|| "#f38ba8".into()),
        color2: wal.colors.get("color2").cloned().unwrap_or_else(|| "#a6e3a1".into()),
        color3: wal.colors.get("color3").cloned().unwrap_or_else(|| "#f9e2af".into()),
        color4: wal.colors.get("color4").cloned().unwrap_or_else(|| "#89b4fa".into()),
        color5: wal.colors.get("color5").cloned().unwrap_or_else(|| "#f5c2e7".into()),
        color6: wal.colors.get("color6").cloned().unwrap_or_else(|| "#94e2d5".into()),
        color7: wal.colors.get("color7").cloned().unwrap_or_else(|| "#bac2de".into()),
    })
}

pub fn load_palette() -> Palette {
    let wal_path = wal_colors_path();
    if !wal_path.exists() {
        return Palette::default();
    }
    match std::fs::read_to_string(&wal_path)
        .ok()
        .and_then(|s| palette_from_wal_json(&s))
    {
        Some(wal) => wal,
        None => Palette::default(),
    }
}


pub fn build_css(palette: &Palette, user_css: Option<&str>) -> String {
    let mut css = format!(
        r#"
@define-color bg {bg};
@define-color fg {fg};
@define-color red {c1};
@define-color green {c2};
@define-color yellow {c3};
@define-color blue {c4};
@define-color pink {c5};
@define-color teal {c6};
@define-color overlay {c0};

window {{
    background-color: @bg;
    color: @fg;
    border-radius: 12px;
}}

.popup-entry {{
    background: @bg;
    color: @fg;
    border: 2px solid @blue;
    border-radius: 8px;
    padding: 12px 16px;
    font-size: 16px;
    caret-color: @fg;
}}

.popup-entry:focus {{
    outline: none;
    border-color: @teal;
}}

.type-chip {{
    background: @overlay;
    color: @fg;
    border-radius: 999px;
    padding: 2px 10px;
    font-size: 12px;
    margin: 2px;
}}

.type-chip.active {{
    background: @blue;
    color: @bg;
}}

.confirm-button {{
    background: @green;
    color: @bg;
    border: none;
    border-radius: 8px;
    padding: 8px 16px;
    font-weight: bold;
}}

.note-card {{
    background: shade(@bg, 1.1);
    border-radius: 8px;
    padding: 12px;
    margin: 4px 8px;
    border-left: 3px solid @blue;
}}

.note-card:hover {{
    background: shade(@bg, 1.2);
}}

.search-entry {{
    background: shade(@bg, 1.1);
    color: @fg;
    border: 1px solid @overlay;
    border-radius: 8px;
    padding: 8px 12px;
    margin: 8px;
}}
"#,
        bg = palette.background,
        fg = palette.foreground,
        c0 = palette.color0,
        c1 = palette.color1,
        c2 = palette.color2,
        c3 = palette.color3,
        c4 = palette.color4,
        c5 = palette.color5,
        c6 = palette.color6,
    );

    css.push_str(r#"
.dim-label {
    color: alpha(@fg, 0.5);
    font-size: 12px;
}

.sidebar {
    background: shade(@bg, 0.93);
}

.sidebar-row {
    padding: 6px 12px;
    font-size: 14px;
}

.sidebar-row:hover:not(:selected) {
    background: shade(@bg, 1.08);
}

.sidebar-row:selected {
    background: @blue;
    color: @bg;
}

.sidebar-section-label {
    color: alpha(@fg, 0.4);
    font-size: 10px;
    font-weight: bold;
    padding: 10px 14px 2px 14px;
    letter-spacing: 1px;
}

.action-btn {
    background: transparent;
    border: none;
    border-radius: 6px;
    padding: 2px 7px;
    min-width: 28px;
    min-height: 28px;
    font-size: 14px;
}

.action-btn:hover {
    background: shade(@bg, 1.3);
}

.done-btn { color: @green; }
.done-btn:hover { background: alpha(@green, 0.15); }

.edit-btn { color: @blue; }
.edit-btn:hover { background: alpha(@blue, 0.15); }

.danger-btn { color: @red; }
.danger-btn:hover { background: alpha(@red, 0.15); }

.note-card-todo      { border-left-color: @green;  }
.note-card-reminder  { border-left-color: @yellow; }
.note-card-idea      { border-left-color: @pink;   }
.note-card-question  { border-left-color: @teal;   }
.note-card-note      { border-left-color: @blue;   }

entry {
    background: shade(@bg, 1.1);
    color: @fg;
    border: 1px solid @overlay;
    border-radius: 6px;
    caret-color: @fg;
    padding: 5px 10px;
}

entry:focus {
    border-color: @blue;
    outline: none;
}
"#);

    if let Some(extra) = user_css {
        css.push('\n');
        css.push_str(extra);
    }

    css
}

fn wal_colors_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("~/.cache"))
        .join("wal")
        .join("colors.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKYO_NIGHT_WAL: &str = r##"{
        "special": {
            "background": "#1a1b26",
            "foreground": "#c0caf5"
        },
        "colors": {
            "color0": "#15161e",
            "color1": "#f7768e",
            "color2": "#9ece6a",
            "color3": "#e0af68",
            "color4": "#7aa2f7",
            "color5": "#bb9af7",
            "color6": "#7dcfff",
            "color7": "#a9b1d6"
        }
    }"##;

    // ---- Default palette (Catppuccin Mocha) ----

    #[test]
    fn default_background_is_catppuccin_mocha() {
        assert_eq!(Palette::default().background, "#1e1e2e");
    }

    #[test]
    fn default_foreground_is_catppuccin_mocha() {
        assert_eq!(Palette::default().foreground, "#cdd6f4");
    }

    #[test]
    fn default_red_is_catppuccin_mocha() {
        assert_eq!(Palette::default().color1, "#f38ba8");
    }

    #[test]
    fn default_blue_is_catppuccin_mocha() {
        assert_eq!(Palette::default().color4, "#89b4fa");
    }

    #[test]
    fn default_teal_is_catppuccin_mocha() {
        assert_eq!(Palette::default().color6, "#94e2d5");
    }

    // ---- palette_from_wal_json ----

    #[test]
    fn wal_json_parses_special_background() {
        let p = palette_from_wal_json(TOKYO_NIGHT_WAL).unwrap();
        assert_eq!(p.background, "#1a1b26");
    }

    #[test]
    fn wal_json_parses_special_foreground() {
        let p = palette_from_wal_json(TOKYO_NIGHT_WAL).unwrap();
        assert_eq!(p.foreground, "#c0caf5");
    }

    #[test]
    fn wal_json_parses_numbered_colors() {
        let p = palette_from_wal_json(TOKYO_NIGHT_WAL).unwrap();
        assert_eq!(p.color0, "#15161e");
        assert_eq!(p.color1, "#f7768e");
        assert_eq!(p.color4, "#7aa2f7");
        assert_eq!(p.color7, "#a9b1d6");
    }

    #[test]
    fn wal_json_missing_special_falls_back_to_defaults() {
        let json = r##"{"colors":{"color0":"#000000"}}"##;
        let p = palette_from_wal_json(json).unwrap();
        assert_eq!(p.background, "#1e1e2e");
        assert_eq!(p.foreground, "#cdd6f4");
    }

    #[test]
    fn wal_json_missing_color_falls_back_to_default() {
        let json = r##"{"special":{"background":"#ff0000","foreground":"#ffffff"},"colors":{}}"##;
        let p = palette_from_wal_json(json).unwrap();
        assert_eq!(p.color4, "#89b4fa"); // default blue
    }

    #[test]
    fn invalid_wal_json_returns_none() {
        assert!(palette_from_wal_json("not json").is_none());
        assert!(palette_from_wal_json("").is_none());
        assert!(palette_from_wal_json("{}").is_some()); // empty but valid → all defaults
    }

    // ---- build_css ----

    #[test]
    fn css_defines_bg_color() {
        let css = build_css(&Palette::default(), None);
        assert!(css.contains("@define-color bg #1e1e2e"), "css missing bg: {}", &css[..300]);
    }

    #[test]
    fn css_defines_fg_color() {
        let css = build_css(&Palette::default(), None);
        assert!(css.contains("@define-color fg #cdd6f4"));
    }

    #[test]
    fn css_defines_all_named_colors() {
        let css = build_css(&Palette::default(), None);
        for name in &["red", "green", "yellow", "blue", "pink", "teal", "overlay"] {
            assert!(css.contains(&format!("@define-color {} ", name)), "missing @define-color {}", name);
        }
    }

    #[test]
    fn css_contains_window_rule() {
        let css = build_css(&Palette::default(), None);
        assert!(css.contains("window {"));
        assert!(css.contains("background-color: @bg"));
    }

    #[test]
    fn css_contains_popup_entry_class() {
        let css = build_css(&Palette::default(), None);
        assert!(css.contains(".popup-entry {"), "css: {}", &css[300..600]);
    }

    #[test]
    fn css_contains_note_card_class() {
        let css = build_css(&Palette::default(), None);
        assert!(css.contains(".note-card {"));
    }

    #[test]
    fn css_contains_type_chip_class() {
        let css = build_css(&Palette::default(), None);
        assert!(css.contains(".type-chip {"));
    }

    #[test]
    fn css_contains_sidebar_row_class() {
        let css = build_css(&Palette::default(), None);
        assert!(css.contains(".sidebar-row {"));
    }

    #[test]
    fn css_appends_user_css() {
        let user = ".my-override { color: hotpink; }";
        let css = build_css(&Palette::default(), Some(user));
        assert!(css.contains(".my-override { color: hotpink; }"));
    }

    #[test]
    fn css_without_user_css_omits_user_rules() {
        let css = build_css(&Palette::default(), None);
        assert!(!css.contains(".my-override"));
    }

    #[test]
    fn css_reflects_custom_palette_colors() {
        let mut p = Palette::default();
        p.background = "#deadbe".into();
        p.color4 = "#cafe00".into();
        let css = build_css(&p, None);
        assert!(css.contains("@define-color bg #deadbe"), "css: {}", &css[..300]);
        assert!(css.contains("@define-color blue #cafe00"), "css: {}", &css[..300]);
    }

    #[test]
    fn css_from_wal_palette_uses_wal_colors() {
        let p = palette_from_wal_json(TOKYO_NIGHT_WAL).unwrap();
        let css = build_css(&p, None);
        assert!(css.contains("@define-color bg #1a1b26"), "css: {}", &css[..300]);
        assert!(css.contains("@define-color fg #c0caf5"));
    }

    #[test]
    fn load_palette_returns_valid_palette() {
        // No wal file in CI/test env; should return non-empty strings starting with #
        let palette = load_palette();
        assert!(!palette.background.is_empty());
        assert!(palette.background.starts_with('#'), "bg: {}", palette.background);
        assert!(!palette.foreground.is_empty());
        assert!(palette.color4.starts_with('#'));
    }
}
