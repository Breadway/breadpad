use breadpad_shared::types::Note;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

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
    archived.sort_by(|a, b| b.created.cmp(&a.created));

    if archived.is_empty() {
        list.append(
            &gtk4::Label::builder()
                .label("Archive is empty.")
                .margin_top(32)
                .build(),
        );
    } else {
        for note in archived {
            list.append(&build_archive_card(note, state.clone()));
        }
    }

    scroll.set_child(Some(&list));
    scroll
}

fn build_archive_card(note: &Note, state: crate::AppState) -> gtk4::Box {
    let row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .margin_start(8)
        .margin_end(8)
        .margin_top(2)
        .margin_bottom(2)
        .css_classes(["note-card"])
        .build();

    let completed_str = note
        .completed
        .map(|t| {
            let local: chrono::DateTime<chrono::Local> = t.into();
            format!("done {}", local.format("%b %d"))
        })
        .unwrap_or_else(|| "done".into());

    let done_label = gtk4::Label::builder()
        .label(&completed_str)
        .width_chars(12)
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

    // 🗑 Delete — two-click confirm
    let delete_btn = gtk4::Button::builder()
        .label("🗑")
        .css_classes(["action-btn", "danger-btn"])
        .tooltip_text("Delete permanently")
        .build();
    {
        let confirming = Rc::new(RefCell::new(false));
        let note_id = note.id.clone();
        let row_c = row.clone();
        let btn_c = delete_btn.clone();

        delete_btn.connect_clicked(move |_| {
            if *confirming.borrow() {
                let store = state.write_store();
                if let Err(e) = store.delete_note(&note_id) {
                    state.log_error(format!("delete failed: {}", e));
                }
                row_c.set_visible(false);
                state.reload_notes();
            } else {
                *confirming.borrow_mut() = true;
                btn_c.set_label("Sure?");
            }
        });
    }

    row.append(&done_label);
    row.append(&body_label);
    row.append(&type_label);
    row.append(&delete_btn);
    row
}
