//! Note editor, presented as an AdwDialog (was a bare GtkPopover with no
//! scrim, no title, anchored wherever the triggering button happened to be -
//! flagged in design review as the weakest surface in the app). AdwDialog
//! gives us the scrim, the title, and correct modal anchoring for free.

use bread_theme::adw;
use breadpad_shared::{
    parser::parse_rule_based,
    scheduler::Scheduler,
    store::Store,
    types::{Note, NoteType, RecurrenceRule},
};
use chrono::{Local, TimeZone, Utc};
use gtk4::{glib, prelude::*};
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Same wording used by `main::show_add_note_window`'s New Note dialog - the
/// two surfaces used to teach the user two different input languages for
/// the same fields.
pub const TIME_PLACEHOLDER: &str = "tomorrow 9am  /  at 7pm  /  2026-08-01 09:00";
pub const RRULE_PLACEHOLDER: &str = "RRULE:FREQ=WEEKLY;BYDAY=MO";

pub fn open_editor(
    parent: &gtk4::Widget,
    note: &Note,
    store: Arc<Store>,
    morning: String,
    on_save: Rc<dyn Fn(Note)>,
    on_delete: Rc<dyn Fn()>,
    on_error: Rc<dyn Fn(String)>,
) -> libadwaita::Dialog {
    let dialog = libadwaita::Dialog::builder()
        .title("Edit Note")
        .content_width(480)
        .build();

    let header = libadwaita::HeaderBar::new();
    let toolbar_view = libadwaita::ToolbarView::new();
    toolbar_view.add_top_bar(&header);

    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(16)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    let group = adw::preferences_group("Details", None);

    let body_row = libadwaita::EntryRow::builder().title("Body").build();
    body_row.set_text(&note.body);
    group.add(&body_row);

    let type_row = adw::action_row("Type", None);
    let type_pill_box = gtk4::Box::builder().orientation(gtk4::Orientation::Horizontal).spacing(4).valign(gtk4::Align::Center).build();
    let selected_type: Rc<RefCell<String>> = Rc::new(RefCell::new(note.note_type.as_str().to_string()));
    let type_pills: Vec<(gtk4::Button, &'static str)> = NoteType::all_builtin().iter().map(|&name| (bread_theme::gtk::chip(name), name)).collect();
    for (btn, name) in &type_pills {
        bread_theme::gtk::set_chip_active(btn, *name == selected_type.borrow().as_str());
        let sel = selected_type.clone();
        let name = *name;
        let all_btns: Vec<gtk4::Button> = type_pills.iter().map(|(b, _)| b.clone()).collect();
        btn.connect_clicked(move |clicked| {
            *sel.borrow_mut() = name.to_string();
            for b in &all_btns { bread_theme::gtk::set_chip_active(b, false); }
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
    // the group description instead of a placeholder that would otherwise
    // vanish behind the title when empty.
    group.add(&time_row);

    let rrule_row = libadwaita::EntryRow::builder().title("Recurrence").build();
    rrule_row.set_text(note.rrule.as_ref().map(|r| r.as_str()).unwrap_or(""));
    group.add(&rrule_row);

    content.append(&group);

    let hint = gtk4::Label::builder()
        .label(format!("Time: {TIME_PLACEHOLDER}\nRecurrence: {RRULE_PLACEHOLDER}"))
        .css_classes(["dim-label"])
        .xalign(0.0)
        .wrap(true)
        .build();
    content.append(&hint);

    let btn_row = gtk4::Box::builder().orientation(gtk4::Orientation::Horizontal).spacing(8).build();
    let delete_btn = gtk4::Button::builder().label("Delete").css_classes(["destructive-action"]).build();
    let save_btn = gtk4::Button::builder().label("Save").css_classes(["confirm-button"]).hexpand(true).build();
    btn_row.append(&delete_btn);
    btn_row.append(&save_btn);
    content.append(&btn_row);

    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .build();
    scroll.set_child(Some(&content));
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
        let selected_type = selected_type.clone();

        save_btn.connect_clicked(move |_| {
            let mut updated = note_clone.clone();
            updated.body = body_row.text().to_string();
            updated.note_type = NoteType::from_str(&selected_type.borrow());
            let time_str = time_row.text().to_string();
            updated.time = if time_str.trim().is_empty() { None } else { parse_time_field(&time_str, &morning) };
            let rrule_text = rrule_row.text().to_string();
            updated.rrule = if rrule_text.trim().is_empty() { None } else { Some(RecurrenceRule::new(rrule_text)) };

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

    dialog.present(Some(parent));
    dialog
}

fn spawn_bg<F, T, C>(work: F, then: C)
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
    C: FnOnce(T) + 'static,
{
    let (tx, rx) = futures_channel::oneshot::channel::<T>();
    std::thread::spawn(move || { let _ = tx.send(work()); });
    glib::MainContext::default().spawn_local(async move {
        if let Ok(result) = rx.await {
            then(result);
        }
    });
}

fn parse_time_field(s: &str, morning: &str) -> Option<chrono::DateTime<Utc>> {
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M") {
        if let chrono::LocalResult::Single(local) = Local.from_local_datetime(&naive) {
            return Some(local.with_timezone(&Utc));
        }
    }
    parse_rule_based(s, morning).time
}
