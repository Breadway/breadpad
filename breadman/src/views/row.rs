//! Shared single-line note row, used by every list view (All/Upcoming/
//! per-type/Archive) instead of each view hand-rolling its own card. Design
//! review found the two-line card (title/badge top, huge dead gap, actions
//! bottom-right) used by the active views wasted enormous horizontal space
//! compared to Archive's tighter aligned-column layout - this ports that
//! layout everywhere and unifies the row template (including the edit
//! affordance, previously pencil-in-active / click-row-in-archive).

use breadpad_shared::types::{Note, NoteType};
use gtk4::prelude::*;
use libadwaita::prelude::*;
use std::rc::Rc;

pub struct RowSpec<'a> {
    pub date_label: String,
    pub note: &'a Note,
    pub show_type_badge: bool,
    pub show_done: bool,
}

/// Type-tinted badge class matching the `note-card-{type}` accent-bar colors
/// already established in breadpad-shared's theme (todo=green,
/// reminder=yellow, idea=pink, question=teal, note=blue).
fn type_chip_class(note_type: &NoteType) -> &'static str {
    match note_type {
        NoteType::Todo => "type-chip-todo",
        NoteType::Reminder => "type-chip-reminder",
        NoteType::Idea => "type-chip-idea",
        NoteType::Note => "type-chip-note",
        NoteType::Question => "type-chip-question",
        NoteType::Tag(_) => "type-chip",
    }
}

pub fn build(spec: RowSpec, state: crate::AppState) -> gtk4::Box {
    let note = spec.note;
    let row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .margin_start(8)
        .margin_end(8)
        .margin_top(2)
        .margin_bottom(2)
        .css_classes(["note-card"])
        .build();
    row.add_css_class(&format!("note-card-{}", note.note_type.as_str()));

    let date_label = gtk4::Label::builder()
        .label(&spec.date_label)
        .width_chars(16)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    row.append(&date_label);

    let body_label = gtk4::Label::builder()
        .label(&note.body)
        .hexpand(true)
        .xalign(0.0)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();
    row.append(&body_label);

    if let Some(ws) = &note.workspace {
        row.append(
            &gtk4::Label::builder()
                .label(format!("ws:{}", ws))
                .css_classes(["type-chip"])
                .build(),
        );
    }
    if note.rrule.is_some() {
        row.append(&gtk4::Label::builder().label("\u{21bb}").css_classes(["dim-label"]).build());
    }

    if spec.show_type_badge {
        row.append(
            &gtk4::Label::builder()
                .label(note.note_type.as_str())
                .css_classes(["type-chip", type_chip_class(&note.note_type)])
                .build(),
        );
    }

    if spec.show_done {
        let done_btn = gtk4::Button::builder()
            .icon_name("object-select-symbolic")
            .css_classes(["action-btn", "done-btn"])
            .tooltip_text("Mark done")
            .build();
        {
            let note_id = note.id.clone();
            let row_c = row.clone();
            let state_c = state.clone();
            done_btn.connect_clicked(move |_| {
                row_c.set_visible(false); // optimistic hide
                let store = state_c.write_store();
                let id = note_id.clone();
                let state = state_c.clone();
                crate::spawn_bg(
                    move || -> anyhow::Result<Vec<Note>> {
                        if let Some(mut n) = store.get_by_id(&id)? {
                            n.mark_done();
                            store.update_note(&n)?;
                        }
                        store.load_all()
                    },
                    move |result| match result {
                        Ok(fresh) => {
                            *state.notes.borrow_mut() = fresh;
                            crate::rebuild_stack(&state);
                            let active = state.active_view.borrow().clone();
                            state.stack.set_visible_child_name(&active);
                        }
                        Err(e) => state.log_error(format!("mark done failed: {}", e)),
                    },
                );
            });
        }
        row.append(&done_btn);
    }

    let edit_btn = gtk4::Button::builder()
        .icon_name("document-edit-symbolic")
        .css_classes(["action-btn", "edit-btn"])
        .tooltip_text("Edit")
        .build();
    {
        let note_c = note.clone();
        let state_c = state.clone();
        let body_label_c = body_label.clone();
        let row_c = row.clone();

        edit_btn.connect_clicked(move |btn| {
            let morning = state_c.cfg.borrow().reminders.default_morning.clone();
            let store = std::sync::Arc::new(state_c.write_store());

            let state_save = state_c.clone();
            let body_label_save = body_label_c.clone();
            let state_del = state_c.clone();
            let row_del = row_c.clone();
            let state_err = state_c.clone();

            let dialog = crate::editor::open_editor(
                &note_c,
                store,
                morning,
                std::rc::Rc::new(move |updated: Note| {
                    body_label_save.set_label(&updated.body);
                    state_save.reload_notes();
                    crate::rebuild_stack(&state_save);
                    let active = state_save.active_view.borrow().clone();
                    state_save.stack.set_visible_child_name(&active);
                }),
                std::rc::Rc::new(move || {
                    row_del.set_visible(false);
                    state_del.reload_notes();
                    crate::rebuild_stack(&state_del);
                    let active = state_del.active_view.borrow().clone();
                    state_del.stack.set_visible_child_name(&active);
                }),
                std::rc::Rc::new(move |e: String| {
                    state_err.log_error(e);
                }),
            );
            dialog.present(Some(btn.upcast_ref::<gtk4::Widget>()));
        });
    }
    row.append(&edit_btn);

    let delete_btn = gtk4::Button::builder()
        .icon_name("user-trash-symbolic")
        .css_classes(["action-btn", "danger-btn"])
        .tooltip_text("Delete")
        .build();
    {
        use std::cell::RefCell;
        use std::rc::Rc;
        let confirming = Rc::new(RefCell::new(false));
        let note_id = note.id.clone();
        let row_c = row.clone();
        let state_c = state.clone();
        let btn_c = delete_btn.clone();

        delete_btn.connect_clicked(move |_| {
            if *confirming.borrow() {
                row_c.set_visible(false); // optimistic hide
                let store = state_c.write_store();
                let id = note_id.clone();
                let state = state_c.clone();
                crate::spawn_bg(
                    move || -> anyhow::Result<Vec<Note>> {
                        store.delete_note(&id)?;
                        if let Err(e) = breadpad_shared::scheduler::Scheduler::cancel(&id) {
                            tracing::warn!("failed to cancel timer for {}: {}", id, e);
                        }
                        store.load_all()
                    },
                    move |result| match result {
                        Ok(fresh) => {
                            *state.notes.borrow_mut() = fresh;
                            crate::rebuild_stack(&state);
                            let active = state.active_view.borrow().clone();
                            state.stack.set_visible_child_name(&active);
                        }
                        Err(e) => state.log_error(format!("delete failed: {}", e)),
                    },
                );
            } else {
                *confirming.borrow_mut() = true;
                btn_c.set_icon_name("edit-delete-symbolic");
                btn_c.set_tooltip_text(Some("Click again to delete permanently"));
            }
        });
    }
    row.append(&delete_btn);

    row
}

/// Centered "nothing here" state with type-specific copy and (optionally) a
/// direct affordance to act on - the empty states were all top-anchored
/// generic text with no icon or action.
pub fn build_empty_state(icon_name: &str, text: &str, action: Option<(String, Rc<dyn Fn()>)>) -> gtk4::Widget {
    let outer = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(12)
        .valign(gtk4::Align::Center)
        .halign(gtk4::Align::Center)
        .vexpand(true)
        .build();

    let icon = gtk4::Image::builder()
        .icon_name(icon_name)
        .pixel_size(32)
        .css_classes(["dim-label"])
        .build();
    outer.append(&icon);

    let label = gtk4::Label::builder()
        .label(text)
        .css_classes(["dim-label"])
        .justify(gtk4::Justification::Center)
        .build();
    outer.append(&label);

    if let Some((label_text, on_click)) = action {
        let btn = gtk4::Button::builder()
            .label(&label_text)
            .css_classes(["confirm-button"])
            .halign(gtk4::Align::Center)
            .build();
        btn.connect_clicked(move |_| on_click());
        outer.append(&btn);
    }

    outer.upcast()
}

/// Convenience wrapper for the note-list views: a "+ New {type}" button that
/// opens the New Note window preselected to `note_type`.
pub fn new_note_action(note_type: NoteType, window: gtk4::ApplicationWindow, state: crate::AppState) -> (String, Rc<dyn Fn()>) {
    let label = format!("+  New {}", note_type.as_str());
    let action: Rc<dyn Fn()> = Rc::new(move || {
        crate::show_add_note_window(&window, state.clone(), note_type.clone(), |_| {});
    });
    (label, action)
}
