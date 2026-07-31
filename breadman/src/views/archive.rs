use super::row::{build_empty_state, RowSpec};
use breadpad_shared::types::Note;
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

    let mut archived: Vec<&Note> = notes.iter().filter(|n| n.done).collect();
    // Sort by completion time, not creation time - the previous sort used
    // `created` while the row displayed `completed` ("done {date}"), which
    // is why the last row could appear out of order against the visible
    // dates.
    archived.sort_by_key(|n| std::cmp::Reverse(n.completed.unwrap_or(n.created)));

    if archived.is_empty() {
        list.append(&build_empty_state("folder-symbolic", "Nothing archived yet.", None));
    } else {
        for note in archived {
            let completed_str = note
                .completed
                .map(|t| {
                    let local: chrono::DateTime<chrono::Local> = t.into();
                    format!("done {}", local.format("%b %d"))
                })
                .unwrap_or_else(|| "done".into());
            let spec = RowSpec { date_label: completed_str, note, show_type_badge: true, show_done: false };
            list.append(&super::row::build(spec, state.clone()));
        }
    }

    scroll.set_child(Some(&list));
    scroll
}
