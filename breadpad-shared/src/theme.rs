pub use bread_theme::{load_palette, Palette};

/// Apply breadpad/breadman's stylesheet and keep it live across palette changes.
/// [`build_css`] bundles the shared component sheet with the app's own rules from
/// the current pywal palette; `bread_theme::gtk::apply_app_css` re-runs this
/// whenever `bread-theme reload` rewrites the shared theme file, so the UI
/// recolours in place (and re-reads the user's `style.css` override too).
pub fn apply_live() {
    bread_theme::gtk::apply_app_css(|| {
        let palette = load_palette();
        let user_css = std::fs::read_to_string(crate::config::style_css_path()).ok();
        build_css(&palette, user_css.as_deref())
    });
}

/// Generate the full breadpad/breadman CSS string. The base — `@define-color`
/// palette, fonts, and generic widget styling — comes from the shared
/// `bread_theme::stylesheet`, so breadpad and breadman look identical to the
/// rest of the ecosystem. Only breadpad-specific component rules are appended.
pub fn build_css(palette: &Palette, user_css: Option<&str>) -> String {
    // Shared ecosystem base (define-colors incl. accent, font, buttons, entries,
    // switches, lists, cards, scrollbars). `overlay` here is color7 — consistent
    // with every other bread app (breadpad previously mapped it to color0).
    let mut css = bread_theme::stylesheet(palette);

    css.push_str(
        r#"
/* breadpad/breadman-specific components */
window { border-radius: 8px; }

.popup-entry {
    background: @bg;
    color: @fg;
    border: 2px solid @blue;
    border-radius: 6px;
    padding: 12px 16px;
    font-size: 14px;
    caret-color: @fg;
}

.popup-entry:focus {
    outline: none;
    border-color: @teal;
}

/* Shared "selected/active" language: a ghost/ outline default state that
   stays quiet, and a solid, high-contrast accent fill for whatever is
   currently selected — used identically by .type-chip.active and
   .sidebar-row:selected so the two windows read as one system. */
.type-chip {
    background: transparent;
    color: alpha(@fg, 0.6);
    border: 1px solid alpha(@fg, 0.18);
    border-radius: 999px;
    padding: 4px 12px;
    font-size: 12px;
    margin: 4px;
}

.type-chip.active {
    background: @blue;
    color: @on-accent;
    border-color: @blue;
    font-weight: 600;
}

/* Per-type tint so info badges (note cards, workspace/recur tags) stay
   scannable at a glance without the old full-color emoji. */
.note-card-todo      .type-chip { color: @green;  border-color: alpha(@green, 0.4); }
.note-card-reminder  .type-chip { color: @yellow; border-color: alpha(@yellow, 0.4); }
.note-card-idea      .type-chip { color: @pink;   border-color: alpha(@pink, 0.4); }
.note-card-question  .type-chip { color: @teal;   border-color: alpha(@teal, 0.4); }
.note-card-note      .type-chip { color: @blue;   border-color: alpha(@blue, 0.4); }

.confirm-button {
    background: @blue;
    color: @on-accent;
    border: none;
    border-radius: 8px;
    padding: 10px 22px;
    min-height: 20px;
    font-size: 15px;
    font-weight: bold;
}

.confirm-button:hover { background: shade(@blue, 1.1); }

/* Separates the primary action from the chip row it sits beside so it
   doesn't read as just another pill. */
.confirm-wrap {
    border-left: 1px solid alpha(@fg, 0.12);
    padding-left: 12px;
    margin-left: 4px;
}

.prefix-hint {
    color: alpha(@fg, 0.4);
    font-size: 10px;
    letter-spacing: 0.3px;
}

.note-card {
    background: shade(@bg, 1.12);
    border: 1px solid alpha(@fg, 0.07);
    border-radius: 10px;
    padding: 12px;
    margin: 6px 0;
    border-left: 3px solid @blue;
}

.note-card:hover {
    background: shade(@bg, 1.22);
    border-color: alpha(@fg, 0.12);
}

.note-title {
    font-weight: 600;
}

.note-card .action-btn {
    opacity: 0.45;
}

.note-card:hover .action-btn {
    opacity: 1;
}

.search-entry {
    background: shade(@bg, 1.1);
    color: @fg;
    border: 1px solid @overlay;
    border-radius: 6px;
    padding: 5px 10px;
    font-size: 13px;
}

.search-entry:focus {
    border-color: @blue;
    outline: none;
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
    color: @on-accent;
    font-weight: 600;
}

.sidebar-row-minor {
    opacity: 0.5;
    font-size: 12px;
}

.sidebar-count {
    color: alpha(@fg, 0.45);
    font-size: 11px;
}

.icon-todo     { color: @green;  }
.icon-reminder { color: @yellow; }
.icon-idea     { color: @pink;   }
.icon-note     { color: @blue;   }
.icon-question { color: @teal;   }

.sidebar-row:selected .sidebar-count {
    color: alpha(@on-accent, 0.75);
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
    padding: 3px 8px;
    min-width: 32px;
    min-height: 32px;
    font-size: 16px;
}

.action-btn:hover {
    background: shade(@bg, 1.3);
    opacity: 1;
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
        // #1e1e2e (Catppuccin Mocha) was the default before bread-theme
        // v0.2.10, which deliberately fixed background/surface/overlay to
        // BOS's own dark theme (#0c0c0c etc.) instead of tracking pywal —
        // see bread-theme's `palette.rs` `FIXED_BACKGROUND` doc comment.
        // This assertion was never updated for that bump, so it started
        // failing the moment breadpad's Cargo.toml pin moved to v0.2.10.
        let css = build_css(&Palette::default(), None);
        assert!(css.contains("@define-color bg #0c0c0c"), "css missing bg: {}", &css[..300]);
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
