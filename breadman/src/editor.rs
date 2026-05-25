use breadpad_shared::{
    parser::parse_rule_based,
    store::Store,
    types::{Note, NoteType, RecurrenceRule},
};
use chrono::{Local, TimeZone, Utc};
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

pub fn build_editor_popover(
    note: &Note,
    store: Arc<Store>,
    morning: String,
    on_save: impl Fn(Note) + 'static,
    on_delete: impl Fn() + 'static,
) -> gtk4::Popover {
    let popover = gtk4::Popover::new();
    popover.set_has_arrow(false);

    let vbox = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .width_request(420)
        .build();

    vbox.append(&gtk4::Label::builder().label("Body").xalign(0.0).build());
    let body_entry = gtk4::Entry::builder()
        .text(&note.body)
        .hexpand(true)
        .build();
    vbox.append(&body_entry);

    vbox.append(&gtk4::Label::builder().label("Type").xalign(0.0).build());
    let type_combo = gtk4::DropDown::from_strings(NoteType::all_builtin());
    let current_idx = NoteType::all_builtin()
        .iter()
        .position(|&s| s == note.note_type.as_str())
        .unwrap_or(3) as u32;
    type_combo.set_selected(current_idx);
    vbox.append(&type_combo);

    vbox.append(&gtk4::Label::builder().label("Time").xalign(0.0).build());
    let time_text = note
        .time
        .map(|t| {
            let local: chrono::DateTime<Local> = t.into();
            local.format("%Y-%m-%d %H:%M").to_string()
        })
        .unwrap_or_default();
    let time_entry = gtk4::Entry::builder()
        .text(&time_text)
        .placeholder_text("YYYY-MM-DD HH:MM  or  tomorrow 9am  (blank = no time)")
        .hexpand(true)
        .build();
    vbox.append(&time_entry);

    vbox.append(&gtk4::Label::builder().label("Recurrence").xalign(0.0).build());
    let rrule_entry = gtk4::Entry::builder()
        .text(note.rrule.as_ref().map(|r| r.as_str()).unwrap_or(""))
        .placeholder_text("RRULE:FREQ=WEEKLY;BYDAY=MO  (blank = none)")
        .build();
    vbox.append(&rrule_entry);

    // Button row: [Delete] [Save]
    let btn_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .build();

    let delete_btn = gtk4::Button::builder()
        .label("🗑  Delete")
        .css_classes(["danger-btn"])
        .build();
    let save_btn = gtk4::Button::builder()
        .label("Save")
        .css_classes(["confirm-button"])
        .hexpand(true)
        .build();
    btn_row.append(&delete_btn);
    btn_row.append(&save_btn);
    vbox.append(&btn_row);

    // Delete: two-click confirm using a single handler and shared state
    let confirming = Rc::new(RefCell::new(false));
    {
        let confirming = confirming.clone();
        let delete_btn_label = delete_btn.clone();
        let note_id = note.id.clone();
        let store_del = store.clone();
        let popover_del = popover.clone();

        delete_btn.connect_clicked(move |_| {
            let currently = *confirming.borrow();
            if currently {
                if let Err(e) = store_del.delete_note(&note_id) {
                    tracing::error!("failed to delete note: {}", e);
                } else {
                    on_delete();
                }
                popover_del.popdown();
            } else {
                *confirming.borrow_mut() = true;
                delete_btn_label.set_label("Sure?");
            }
        });
    }

    // Save
    let note_clone = note.clone();
    let popover_save = popover.clone();

    save_btn.connect_clicked(move |_| {
        let mut updated = note_clone.clone();
        updated.body = body_entry.text().to_string();

        updated.note_type = NoteType::from_str(
            NoteType::all_builtin()
                .get(type_combo.selected() as usize)
                .copied()
                .unwrap_or("note"),
        );

        let time_str = time_entry.text().to_string();
        updated.time = if time_str.trim().is_empty() {
            None
        } else {
            parse_time_field(&time_str, &morning)
        };

        let rrule_text = rrule_entry.text().to_string();
        updated.rrule = if rrule_text.trim().is_empty() {
            None
        } else {
            Some(RecurrenceRule::new(rrule_text))
        };

        if let Err(e) = store.update_note(&updated) {
            tracing::error!("failed to update note: {}", e);
        } else {
            on_save(updated);
        }
        popover_save.popdown();
    });

    popover.set_child(Some(&vbox));
    popover
}

fn parse_time_field(s: &str, morning: &str) -> Option<chrono::DateTime<Utc>> {
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M") {
        if let chrono::LocalResult::Single(local) = Local.from_local_datetime(&naive) {
            return Some(local.with_timezone(&Utc));
        }
    }
    parse_rule_based(s, morning).time
}
