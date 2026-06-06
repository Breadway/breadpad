pub use bread_theme::{load_palette, Palette};

/// Generate the full breadpad CSS string. The base colour variables come from
/// `bread-theme`; the widget rules below are breadpad-specific.
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

* {{
    font-family: 'Varela Round', sans-serif;
}}

window {{
    background-color: @bg;
    color: @fg;
    border-radius: 8px;
}}

.popup-entry {{
    background: @bg;
    color: @fg;
    border: 2px solid @blue;
    border-radius: 6px;
    padding: 12px 16px;
    font-size: 14px;
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
    padding: 4px 12px;
    font-size: 12px;
    margin: 4px;
}}

.type-chip.active {{
    background: @blue;
    color: @bg;
}}

.confirm-button {{
    background: @blue;
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
    margin: 8px;
    border-left: 3px solid @blue;
}}

.note-card:hover {{
    background: shade(@bg, 1.2);
}}

.search-entry {{
    background: shade(@bg, 1.1);
    color: @fg;
    border: 1px solid @overlay;
    border-radius: 6px;
    padding: 8px 12px;
}}

.search-entry:focus {{
    border-color: @blue;
    outline: none;
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
    padding: 8px 12px;
    font-size: 14px;
    transition: background 100ms ease;
}

.sidebar-row:hover:not(:selected) {
    background: shade(@bg, 1.1);
}

.sidebar-row:selected {
    background: @blue;
    color: @bg;
    font-weight: 500;
}

.sidebar-section-label {
    color: alpha(@fg, 0.5);
    font-size: 11px;
    font-weight: 600;
    padding: 12px 12px 8px 12px;
    letter-spacing: 0.5px;
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

.reminder-window {
    background: @bg;
    border: 1px solid @overlay;
    border-radius: 8px;
}

.reminder-emoji { font-size: 20px; }

.reminder-title {
    font-size: 12px;
    font-weight: bold;
    color: alpha(@fg, 0.6);
    letter-spacing: 0.5px;
}

.reminder-time {
    font-size: 12px;
    color: alpha(@fg, 0.5);
}

.reminder-body {
    font-size: 18px;
    font-weight: bold;
    color: @fg;
}

.reminder-dismiss {
    background: transparent;
    border: 1px solid @overlay;
    border-radius: 8px;
    padding: 8px 16px;
    color: alpha(@fg, 0.6);
}

.reminder-dismiss:hover { background: shade(@bg, 1.1); }

.reminder-snooze {
    background: transparent;
    border: 1px solid @overlay;
    border-radius: 8px;
    padding: 8px 16px;
    color: @fg;
}

.reminder-snooze:hover { background: shade(@bg, 1.1); }

.snooze-option {
    background: transparent;
    border: none;
    border-radius: 6px;
    padding: 8px 12px;
    color: @fg;
}

.snooze-option:hover { background: shade(@bg, 1.2); }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_defines_bg_color() {
        let css = build_css(&Palette::default(), None);
        assert!(css.contains("@define-color bg #1e1e2e"), "css missing bg: {}", &css[..300]);
    }

    #[test]
    fn css_defines_all_named_colors() {
        let css = build_css(&Palette::default(), None);
        for name in &["red", "green", "yellow", "blue", "pink", "teal", "overlay"] {
            assert!(css.contains(&format!("@define-color {name} ")), "missing @define-color {name}");
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
}
