//! Local stand-ins for `bread_theme::gtk::{chip, set_chip_active}` and
//! `bread_theme::adw::init`, which are not on bread-theme v0.7.1.

use gtk4::prelude::*;

pub fn chip(label: &str) -> gtk4::Button {
    gtk4::Button::builder().label(label).css_classes(["chip"]).build()
}

pub fn set_chip_active(chip: &impl IsA<gtk4::Widget>, active: bool) {
    if active {
        chip.add_css_class("active");
    } else {
        chip.remove_css_class("active");
    }
}

/// Initializes libadwaita and forces dark mode (bread-theme's palette is a
/// fixed dark base regardless of the system GTK preference).
pub fn init_adw() {
    libadwaita::init().expect("failed to initialize libadwaita");
    libadwaita::StyleManager::default().set_color_scheme(libadwaita::ColorScheme::ForceDark);
}
