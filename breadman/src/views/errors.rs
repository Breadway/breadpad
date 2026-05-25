use chrono::DateTime;
use gtk4::prelude::*;

pub fn build(entries: &[(DateTime<chrono::Local>, String)]) -> gtk4::ScrolledWindow {
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let list = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(4)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();

    if entries.is_empty() {
        list.append(
            &gtk4::Label::builder()
                .label("No errors or warnings this session.")
                .margin_top(32)
                .build(),
        );
    } else {
        for (ts, msg) in entries.iter().rev() {
            let row = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(8)
                .css_classes(["note-card"])
                .margin_top(2)
                .margin_bottom(2)
                .build();

            let time_label = gtk4::Label::builder()
                .label(&ts.format("%H:%M:%S").to_string())
                .width_chars(10)
                .xalign(0.0)
                .css_classes(["dim-label"])
                .build();

            let msg_label = gtk4::Label::builder()
                .label(msg)
                .hexpand(true)
                .xalign(0.0)
                .wrap(true)
                .selectable(true)
                .build();

            row.append(&time_label);
            row.append(&msg_label);
            list.append(&row);
        }
    }

    scroll.set_child(Some(&list));
    scroll
}
