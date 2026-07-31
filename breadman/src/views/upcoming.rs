use super::row::{build_empty_state, RowSpec};
use breadpad_shared::types::{Note, NoteType};
use gtk4::prelude::*;

pub fn build(notes: &[Note], state: crate::AppState) -> gtk4::ScrolledWindow {
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
        list.append(&build_empty_state("x-office-calendar-symbolic", "No upcoming reminders or todos.", None));
    } else {
        for note in upcoming {
            let time_str = note
                .effective_time()
                .map(|t| {
                    let local: chrono::DateTime<chrono::Local> = t.into();
                    local.format("%a %b %d, %H:%M").to_string()
                })
                .unwrap_or_default();
            let spec = RowSpec { date_label: time_str, note, show_type_badge: true, show_done: true };
            list.append(&super::row::build(spec, state.clone()));
        }
    }

    scroll.set_child(Some(&list));
    scroll
}
