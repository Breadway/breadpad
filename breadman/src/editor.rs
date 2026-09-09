//! Note editor, presented as an AdwDialog (was a bare GtkPopover with no
//! scrim, no title, anchored wherever the triggering button happened to be -
//! flagged in design review as the weakest surface in the app). AdwDialog
//! gives us the scrim, the title, and correct modal anchoring for free.
//!
//! Both entry points ([`open_editor`] for an existing note, [`open_new_note`]
//! for a fresh one) build the *same* field surface via [`build_fields`], so
//! the "New Note" and "Edit Note" dialogs no longer teach two different
//! input languages for the same fields (PAD-3).

use breadpad_shared::{
    parser::parse_rule_based,
    scheduler::Scheduler,
    store::Store,
    types::{Note, NoteType, RecurrenceRule},
};
use chrono::{Local, TimeZone, Utc};
use gtk4::prelude::*;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::spawn_bg;

/// Shared wording for the free-text time / recurrence fields, so every
/// surface that accepts them documents them identically.
pub const TIME_PLACEHOLDER: &str = "tomorrow 9am  /  at 7pm  /  2026-08-01 09:00";
pub const RRULE_PLACEHOLDER: &str = "RRULE:FREQ=WEEKLY;BYDAY=MO";

/// The editable fields of a note, wired to a [`libadwaita::PreferencesGroup`]
/// of `EntryRow`s plus the shared chip type-picker. Returned by
/// [`build_fields`] and consumed by both dialog entry points.
#[derive(Clone)]
struct Fields {
    body: libadwaita::EntryRow,
    time: libadwaita::EntryRow,
    rrule: libadwaita::EntryRow,
    selected_type: Rc<RefCell<String>>,
}

/// Build the shared field surface, pre-filled from `note` (pass a fresh
/// `Note::new("", preselect, None)` for the "new note" case). Returns the
/// scrollable content widget to drop into the dialog's `ToolbarView`, plus
/// handles to read the fields back on save.
fn build_fields(note: &Note) -> (gtk4::ScrolledWindow, gtk4::Box, Fields) {
    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(16)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    let group = libadwaita::PreferencesGroup::builder()
        .title("Details")
        .build();

    let body_row = libadwaita::EntryRow::builder().title("Body").build();
    body_row.set_text(&note.body);
    group.add(&body_row);

    let type_row = libadwaita::ActionRow::builder().title("Type").build();
    let type_pill_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(4)
        .valign(gtk4::Align::Center)
        .build();
    let selected_type: Rc<RefCell<String>> =
        Rc::new(RefCell::new(note.note_type.as_str().to_string()));
    let type_pills: Vec<(gtk4::Button, &'static str)> = NoteType::all_builtin()
        .iter()
        .map(|&name| (bread_theme::gtk::chip(name), name))
        .collect();
    for (btn, name) in &type_pills {
        bread_theme::gtk::set_chip_active(btn, *name == selected_type.borrow().as_str());
        let sel = selected_type.clone();
        let name = *name;
        let all_btns: Vec<gtk4::Button> = type_pills.iter().map(|(b, _)| b.clone()).collect();
        btn.connect_clicked(move |clicked| {
            *sel.borrow_mut() = name.to_string();
            for b in &all_btns {
                bread_theme::gtk::set_chip_active(b, false);
            }
            bread_theme::gtk::set_chip_active(clicked, true);
        });
        type_pill_box.append(btn);
    }
    type_row.add_suffix(&type_pill_box);
    group.add(&type_row);

    let time_text = note
        .time
        .map(|t| {
            let local: chrono::DateTime<Local> = t.into();
            local.format("%Y-%m-%d %H:%M").to_string()
        })
        .unwrap_or_default();
    let time_row = libadwaita::EntryRow::builder().title("Time").build();
    time_row.set_text(&time_text);
    // EntryRow has no placeholder-text property of its own (unlike GtkEntry) -
    // the title already communicates the field, so the example format goes in
    // the hint label below instead of a placeholder that would otherwise
    // vanish behind the title when empty.
    group.add(&time_row);

    let rrule_row = libadwaita::EntryRow::builder().title("Recurrence").build();
    rrule_row.set_text(note.rrule.as_ref().map(|r| r.as_str()).unwrap_or(""));
    group.add(&rrule_row);

    content.append(&group);

    let hint = gtk4::Label::builder()
        .label(format!(
            "Time: {TIME_PLACEHOLDER}\nRecurrence: {RRULE_PLACEHOLDER}"
        ))
        .css_classes(["dim-label"])
        .xalign(0.0)
        .wrap(true)
        .build();
    content.append(&hint);

    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .min_content_height(400)
        .build();
    scroll.set_child(Some(&content));

    (
        scroll,
        content,
        Fields {
            body: body_row,
            time: time_row,
            rrule: rrule_row,
            selected_type,
        },
    )
}

fn shell(title: &str) -> (libadwaita::Dialog, libadwaita::ToolbarView) {
    let dialog = libadwaita::Dialog::builder()
        .title(title)
        .content_width(480)
        .content_height(520)
        .build();
    let toolbar_view = libadwaita::ToolbarView::new();
    toolbar_view.add_top_bar(&libadwaita::HeaderBar::new());
    (dialog, toolbar_view)
}

/// Editor for an existing note. Builds the dialog but does not present it -
/// callers that need to hook its `map` signal (screenshot mode) must
/// connect before presenting or they miss the signal entirely; interactive
/// callers present immediately with `dialog.present(Some(parent))`.
pub fn open_editor(
    note: &Note,
    store: Arc<Store>,
    morning: String,
    on_save: Rc<dyn Fn(Note)>,
    on_delete: Rc<dyn Fn()>,
    on_error: Rc<dyn Fn(String)>,
) -> libadwaita::Dialog {
    let (dialog, toolbar_view) = shell("Edit Note");
    let (scroll, content, fields) = build_fields(note);

    let btn_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .build();
    let delete_btn = gtk4::Button::builder()
        .label("Delete")
        .css_classes(["destructive-action"])
        .build();
    let save_btn = gtk4::Button::builder()
        .label("Save")
        .css_classes(["confirm-button"])
        .hexpand(true)
        .build();
    btn_row.append(&delete_btn);
    btn_row.append(&save_btn);
    content.append(&btn_row);

    toolbar_view.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar_view));

    // Delete: two-click confirm
    let confirming = Rc::new(RefCell::new(false));
    {
        let confirming = confirming.clone();
        let delete_btn_label = delete_btn.clone();
        let note_id = note.id.clone();
        let store_del = store.clone();
        let dialog_del = dialog.clone();
        let on_delete = Rc::clone(&on_delete);
        let on_error = Rc::clone(&on_error);

        delete_btn.connect_clicked(move |_| {
            if *confirming.borrow() {
                let store = store_del.clone();
                let id = note_id.clone();
                let on_delete = Rc::clone(&on_delete);
                let on_error = Rc::clone(&on_error);
                let dialog = dialog_del.clone();
                spawn_bg(
                    move || -> anyhow::Result<()> {
                        store.delete_note(&id)?;
                        if let Err(e) = Scheduler::cancel(&id) {
                            tracing::warn!("failed to cancel timer for {}: {}", id, e);
                        }
                        Ok(())
                    },
                    move |result| {
                        match result {
                            Ok(()) => on_delete(),
                            Err(e) => on_error(format!("delete failed: {}", e)),
                        }
                        dialog.close();
                    },
                );
            } else {
                *confirming.borrow_mut() = true;
                delete_btn_label.set_label("Sure?");
            }
        });
    }

    // Save
    {
        let note_clone = note.clone();
        let dialog_save = dialog.clone();
        let on_error = Rc::clone(&on_error);
        let fields = fields.clone();

        save_btn.connect_clicked(move |_| {
            let mut updated = note_clone.clone();
            updated.body = fields.body.text().to_string();
            updated.note_type = NoteType::from_str(&fields.selected_type.borrow());
            let time_str = fields.time.text().to_string();
            updated.time = if time_str.trim().is_empty() {
                None
            } else {
                parse_time_field(&time_str, &morning)
            };
            let rrule_text = fields.rrule.text().to_string();
            updated.rrule = if rrule_text.trim().is_empty() {
                None
            } else {
                Some(RecurrenceRule::new(rrule_text))
            };

            dialog_save.close();

            let store_bg = store.clone();
            let on_save = Rc::clone(&on_save);
            let on_error = Rc::clone(&on_error);
            spawn_bg(
                move || -> anyhow::Result<Note> {
                    store_bg.update_note(&updated)?;
                    if let Err(e) = Scheduler::cancel(&updated.id) {
                        tracing::warn!("cancel before reschedule: {}", e);
                    }
                    if updated.time.is_some() || updated.rrule.is_some() {
                        Scheduler::schedule(&updated)?;
                    }
                    Ok(updated)
                },
                move |result| match result {
                    Ok(note) => on_save(note),
                    Err(e) => on_error(format!("update failed: {}", e)),
                },
            );
        });
    }

    dialog
}

/// Editor for a brand-new note. Same field surface as [`open_editor`], but
/// with a single "Add Note" action and create-time semantics: the body is
/// run through the rule-based parser (so "todo: call the vet tomorrow 9am"
/// still sets the type, time, and cleaned body), and an explicit type pick
/// always wins over the parser.
pub fn open_new_note(
    preselect: NoteType,
    store: Store,
    morning: String,
    default_type: String,
    on_done: Rc<dyn Fn(Vec<Note>)>,
    on_error: Rc<dyn Fn(String)>,
) -> libadwaita::Dialog {
    let (dialog, toolbar_view) = shell("New Note");
    let seed = Note::new(String::new(), preselect, None);
    let (scroll, content, fields) = build_fields(&seed);

    let status_label = gtk4::Label::builder()
        .label("")
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    content.append(&status_label);

    let add_btn = gtk4::Button::builder()
        .label("Add Note")
        .css_classes(["confirm-button"])
        .hexpand(true)
        .build();
    content.append(&add_btn);

    toolbar_view.set_content(Some(&scroll));
    dialog.set_child(Some(&toolbar_view));

    let do_add: Rc<dyn Fn()> = Rc::new({
        let dialog = dialog.clone();
        let fields_body = fields.body.clone();
        let fields_time = fields.time.clone();
        let fields_rrule = fields.rrule.clone();
        let selected_type = fields.selected_type.clone();
        let status_label = status_label.clone();
        let on_done = Rc::clone(&on_done);
        let on_error = Rc::clone(&on_error);

        move || {
            let body_text = fields_body.text().to_string();
            if body_text.trim().is_empty() {
                status_label.set_label("Body is required.");
                return;
            }

            let parsed = parse_rule_based(&body_text, &morning);
            let user_type = NoteType::from_str(&selected_type.borrow());
            let default_nt = NoteType::from_str(&default_type);

            let mut note = Note::new(parsed.body.clone(), user_type.clone(), None);
            if user_type == default_nt {
                note.note_type = parsed.note_type;
            }
            note.time = parsed.time;
            note.rrule = parsed.rrule;

            let time_str = fields_time.text().to_string();
            if !time_str.trim().is_empty() {
                let tp = parse_rule_based(&time_str, &morning);
                if tp.time.is_some() {
                    note.time = tp.time;
                }
                if tp.rrule.is_some() {
                    note.rrule = tp.rrule;
                }
            }

            let rrule_str = fields_rrule.text().to_string();
            if !rrule_str.trim().is_empty() {
                note.rrule = Some(RecurrenceRule::new(rrule_str));
            }

            let store = store.clone();
            dialog.close();

            let on_done = Rc::clone(&on_done);
            let on_error = Rc::clone(&on_error);
            spawn_bg(
                move || -> anyhow::Result<Vec<Note>> {
                    store.save_note(&note)?;
                    if note.time.is_some() || note.rrule.is_some() {
                        if let Err(e) = Scheduler::cancel(&note.id) {
                            tracing::warn!("cancel before schedule: {}", e);
                        }
                        Scheduler::schedule(&note)?;
                    }
                    store.load_all()
                },
                move |result| match result {
                    Ok(fresh) => on_done(fresh),
                    Err(e) => on_error(format!("save failed: {}", e)),
                },
            );
        }
    });

    {
        let do_add = Rc::clone(&do_add);
        add_btn.connect_clicked(move |_| do_add());
    }
    {
        let do_add = Rc::clone(&do_add);
        let time = fields.time.clone();
        let rrule = fields.rrule.clone();
        fields.body.connect_entry_activated(move |_| {
            if time.text().is_empty() && rrule.text().is_empty() {
                do_add();
            }
        });
    }

    dialog
}

fn parse_time_field(s: &str, morning: &str) -> Option<chrono::DateTime<Utc>> {
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M") {
        if let chrono::LocalResult::Single(local) = Local.from_local_datetime(&naive) {
            return Some(local.with_timezone(&Utc));
        }
    }
    parse_rule_based(s, morning).time
}
