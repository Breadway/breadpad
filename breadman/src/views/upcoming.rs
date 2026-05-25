use breadpad_shared::types::{Note, NoteType};
use gtk4::prelude::*;

pub fn build(notes: &[Note]) -> gtk4::ScrolledWindow {
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
        .build();

    let mut upcoming: Vec<&Note> = notes
        .iter()
        .filter(|n| {
            !n.done
                && matches!(n.note_type, NoteType::Reminder | NoteType::Todo)
                && n.effective_time().is_some()
        })
        .collect();
    upcoming.sort_by_key(|n| n.effective_time().unwrap());

    if upcoming.is_empty() {
        let label = gtk4::Label::builder()
            .label("No upcoming reminders or todos.")
            .margin_top(32)
            .build();
        list.append(&label);
    } else {
        for note in upcoming {
            let card = build_upcoming_card(note);
            list.append(&card);
        }
    }

    scroll.set_child(Some(&list));
    scroll
}

fn build_upcoming_card(note: &Note) -> gtk4::Box {
    let row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .margin_start(8)
        .margin_end(8)
        .margin_top(4)
        .margin_bottom(4)
        .css_classes(["note-card"])
        .build();

    let time_str = note
        .effective_time()
        .map(|t| {
            let local: chrono::DateTime<chrono::Local> = t.into();
            local.format("%a %b %d, %H:%M").to_string()
        })
        .unwrap_or_default();

    let time_label = gtk4::Label::builder()
        .label(&time_str)
        .width_chars(18)
        .xalign(0.0)
        .build();

    let body_label = gtk4::Label::builder()
        .label(&note.body)
        .hexpand(true)
        .xalign(0.0)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();

    let type_label = gtk4::Label::builder()
        .label(note.note_type.as_str())
        .css_classes(["type-chip"])
        .build();

    row.append(&time_label);
    row.append(&body_label);
    row.append(&type_label);
    row
}
