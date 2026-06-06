use anyhow::Result;
use breadpad_shared::{
    config::Config,
    parser::parse_rule_based,
    scheduler::Scheduler,
    store::Store,
    theme::{build_css, load_palette},
    types::{Note, NoteType, RecurrenceRule},
};
use chrono::Local;
use gtk4::{glib, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

mod editor;
mod views;

// ── Args ─────────────────────────────────────────────────────────────────────

mod args {
    #[derive(Debug)]
    pub struct Args {
        pub view: Option<String>,
        pub done_id: Option<String>,
        pub upcoming_plain: bool,
    }

    pub fn parse() -> Args {
        let mut args = Args {
            view: None,
            done_id: None,
            upcoming_plain: false,
        };
        let raw: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < raw.len() {
            match raw[i].as_str() {
                "--view" | "-v" => {
                    i += 1;
                    args.view = raw.get(i).cloned();
                }
                "done" => {
                    i += 1;
                    args.done_id = raw.get(i).cloned();
                }
                "upcoming" => {
                    if raw.get(i + 1).map(|s| s.as_str()) == Some("--plain") {
                        args.upcoming_plain = true;
                        i += 1;
                    }
                    args.view = Some("upcoming".into());
                }
                _ => {}
            }
            i += 1;
        }
        args
    }
}

// ── AppState ──────────────────────────────────────────────────────────────────

/// Shared UI state, cheap to clone (all fields are Rc/Arc).
#[derive(Clone)]
struct AppState {
    store: Arc<Store>,
    notes: Rc<RefCell<Vec<Note>>>,
    cfg: Rc<RefCell<Config>>,
    errors: Rc<RefCell<Vec<(chrono::DateTime<Local>, String)>>>,
    active_view: Rc<RefCell<String>>,
    stack: gtk4::Stack,
}

impl AppState {
    fn new(store: Arc<Store>, notes: Vec<Note>, cfg: Config, stack: gtk4::Stack) -> Self {
        AppState {
            store,
            notes: Rc::new(RefCell::new(notes)),
            cfg: Rc::new(RefCell::new(cfg)),
            errors: Rc::new(RefCell::new(Vec::new())),
            active_view: Rc::new(RefCell::new("all".to_string())),
            stack,
        }
    }

    fn log_error(&self, msg: impl Into<String>) {
        self.errors.borrow_mut().push((Local::now(), msg.into()));
    }

    fn reload_notes(&self) {
        match self.store.load_all() {
            Ok(fresh) => *self.notes.borrow_mut() = fresh,
            Err(e) => self.log_error(format!("failed to reload notes: {}", e)),
        }
    }

    /// Returns a Store clone with CalDAV wired in if enabled in config.
    fn write_store(&self) -> Store {
        let base = self.store.as_ref().clone();
        let cfg = self.cfg.borrow();
        if cfg.calendar.enabled {
            base.with_calendar(cfg.calendar.clone())
        } else {
            base
        }
    }
}

// ── Background I/O helper ─────────────────────────────────────────────────────

/// Run `work` on a background thread, then call `then` on the GTK main thread.
///
/// `work` must be `Send + 'static` (moves into the thread).
/// `then` only needs `'static` — it can capture GTK widgets and `Rc<RefCell<...>>`.
///
/// Uses `glib::MainContext::spawn_local` (called from the main thread) with a
/// `futures_channel::oneshot` to bridge the blocking result back to the async future.
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

// ── Refresh ───────────────────────────────────────────────────────────────────

fn refresh(state: &AppState) {
    state.reload_notes();
    rebuild_stack(state);
    let active = state.active_view.borrow().clone();
    state.stack.set_visible_child_name(&active);
}

/// Replace only the "all" stack page with a new list built from `notes`.
/// All other pages are left untouched, preserving scroll position etc.
fn rebuild_all_view(notes: &[Note], state: &AppState) {
    if let Some(child) = state.stack.child_by_name("all") {
        state.stack.remove(&child);
    }
    let scroll = build_note_list(notes, state.clone());
    state.stack.add_named(&scroll, Some("all"));
}

fn rebuild_stack(state: &AppState) {
    while let Some(child) = state.stack.first_child() {
        state.stack.remove(&child);
    }

    let notes: Vec<Note> = state.notes.borrow().clone();
    let cfg: Config = state.cfg.borrow().clone();
    let errors: Vec<_> = state.errors.borrow().clone();

    // All
    let all_scroll = build_note_list(&notes, state.clone());
    state.stack.add_named(&all_scroll, Some("all"));

    // Upcoming
    let upcoming = views::upcoming::build(&notes);
    state.stack.add_named(&upcoming, Some("upcoming"));

    // Per-type
    for type_name in NoteType::all_builtin() {
        let nt = NoteType::from_str(type_name);
        let filtered: Vec<Note> = notes
            .iter()
            .filter(|n| n.note_type == nt && !n.done)
            .cloned()
            .collect();
        let scroll = build_note_list(&filtered, state.clone());
        state.stack.add_named(&scroll, Some(type_name));
    }

    // Archive
    let archive = views::archive::build(&notes, state.clone());
    state.stack.add_named(&archive, Some("archive"));

    // Settings
    let state_s = state.clone();
    let settings = views::settings::build(&cfg, move |new_cfg| {
        *state_s.cfg.borrow_mut() = new_cfg;
    });
    state.stack.add_named(&settings, Some("settings"));

    // Errors
    let errors_view = views::errors::build(&errors);
    state.stack.add_named(&errors_view, Some("errors"));
}

// ── main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("breadman=info".parse().unwrap()),
        )
        .init();

    let args = args::parse();
    let cfg = Config::load()?;

    if let Some(id) = &args.done_id {
        return cmd_done(id);
    }
    if args.upcoming_plain {
        return cmd_upcoming_plain();
    }

    run_app(args.view, cfg)
}

fn cmd_done(id: &str) -> Result<()> {
    let store = Store::new()?;
    let note = match store.get_by_id(id)? {
        Some(n) => n,
        None => anyhow::bail!("note {} not found", id),
    };
    let mut updated = note;
    updated.mark_done();
    store.update_note(&updated)?;
    println!("marked {} as done", id);
    Ok(())
}

fn cmd_upcoming_plain() -> Result<()> {
    let store = Store::new()?;
    let mut notes: Vec<Note> = store
        .load_all()?
        .into_iter()
        .filter(|n| {
            !n.done
                && matches!(n.note_type, NoteType::Reminder | NoteType::Todo)
                && n.effective_time().is_some()
        })
        .collect();
    notes.sort_by_key(|n| n.effective_time().expect("filtered by is_some above"));
    for note in &notes {
        let t = note.effective_time().expect("filtered by is_some above");
        let local: chrono::DateTime<Local> = t.into();
        println!("[{}] {} — {}", note.id, local.format("%a %b %d %H:%M"), note.body);
    }
    Ok(())
}

fn run_app(initial_view: Option<String>, cfg: Config) -> Result<()> {
    let app = gtk4::Application::builder()
        .application_id("com.breadway.breadman")
        .build();

    let cfg = Arc::new(cfg);
    let initial_view = Arc::new(initial_view);

    app.connect_activate(move |app| {
        let cfg = cfg.as_ref().clone();
        let initial_view = initial_view.as_deref().map(|s| s.to_string());
        if let Err(e) = build_app_window(app, cfg, initial_view) {
            tracing::error!("failed to build window: {}", e);
        }
    });

    let code = app.run_with_args::<String>(&[]);
    if code != glib::ExitCode::SUCCESS {
        anyhow::bail!("GTK application exited with error");
    }
    Ok(())
}

// ── Window ────────────────────────────────────────────────────────────────────

fn build_app_window(
    app: &gtk4::Application,
    cfg: Config,
    initial_view: Option<String>,
) -> Result<()> {
    apply_css(&cfg);

    let store = Arc::new(Store::new()?);
    let notes = store.load_all()?;

    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("breadman")
        .default_width(960)
        .default_height(640)
        .build();

    let hbox = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .build();

    // ── Sidebar ───────────────────────────────────────────────────
    let sidebar_vbox = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .width_request(190)
        .build();

    let new_note_btn = gtk4::Button::builder()
        .label("✚  New Note")
        .css_classes(["confirm-button"])
        .margin_start(12)
        .margin_end(12)
        .margin_top(16)
        .margin_bottom(12)
        .build();
    sidebar_vbox.append(&new_note_btn);

    let sidebar_list = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::Single)
        .css_classes(["sidebar"])
        .build();

    let make_section = |title: &str| {
        let row = gtk4::ListBoxRow::builder()
            .selectable(false)
            .activatable(false)
            .build();
        row.set_child(Some(
            &gtk4::Label::builder()
                .label(title)
                .xalign(0.0)
                .css_classes(["sidebar-section-label"])
                .build(),
        ));
        row
    };
    let make_item = |id: &str, icon: &str, label: &str| {
        let row = gtk4::ListBoxRow::builder()
            .css_classes(["sidebar-row"])
            .build();
        row.set_widget_name(id);
        let hbox = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(10)
            .build();
        hbox.append(
            &gtk4::Label::builder()
                .label(icon)
                .width_chars(2)
                .xalign(0.5)
                .build(),
        );
        hbox.append(
            &gtk4::Label::builder()
                .label(label)
                .xalign(0.0)
                .hexpand(true)
                .build(),
        );
        row.set_child(Some(&hbox));
        row
    };

    sidebar_list.append(&make_section("VIEWS"));
    sidebar_list.append(&make_item("all", "📋", "All"));
    sidebar_list.append(&make_item("upcoming", "📅", "Upcoming"));
    sidebar_list.append(&make_section("TYPES"));
    sidebar_list.append(&make_item("todo", "✅", "Todo"));
    sidebar_list.append(&make_item("reminder", "🔔", "Reminder"));
    sidebar_list.append(&make_item("idea", "💡", "Idea"));
    sidebar_list.append(&make_item("note", "📝", "Note"));
    sidebar_list.append(&make_item("question", "❓", "Question"));
    sidebar_list.append(&make_section("MORE"));
    sidebar_list.append(&make_item("archive", "📦", "Archive"));
    sidebar_list.append(&make_item("settings", "⚙", "Settings"));
    sidebar_list.append(&make_item("errors", "⚠", "Errors"));
    sidebar_vbox.append(&sidebar_list);

    // ── Content area ──────────────────────────────────────────────
    let content_vbox = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .hexpand(true)
        .build();

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Search notes…")
        .css_classes(["search-entry"])
        .margin_start(12)
        .margin_end(12)
        .margin_top(12)
        .margin_bottom(8)
        .build();

    let stack = gtk4::Stack::builder().hexpand(true).vexpand(true).build();

    content_vbox.append(&search_entry);
    content_vbox.append(&stack);

    hbox.append(&sidebar_vbox);
    hbox.append(&gtk4::Separator::builder()
        .orientation(gtk4::Orientation::Vertical)
        .build());
    hbox.append(&content_vbox);
    window.set_child(Some(&hbox));

    // ── AppState ──────────────────────────────────────────────────
    let state = AppState::new(store, notes, cfg, stack.clone());

    // Initial build
    rebuild_stack(&state);

    // ── Sidebar selection ─────────────────────────────────────────
    {
        let state_c = state.clone();
        sidebar_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let view = row.widget_name().to_string();
                if view.is_empty() { return; }
                *state_c.active_view.borrow_mut() = view.clone();
                refresh(&state_c);
            }
        });
    }

    // ── Search ────────────────────────────────────────────────────
    {
        let state_c = state.clone();
        search_entry.connect_search_changed(move |entry| {
            let query = entry.text().to_string();
            let all_notes = state_c.notes.borrow().clone();
            let filtered: Vec<Note> = if query.trim().is_empty() {
                all_notes
            } else {
                let q = query.to_lowercase();
                all_notes
                    .into_iter()
                    .filter(|n| n.body.to_lowercase().contains(&q))
                    .collect()
            };
            // Only replace the "all" page — other views keep their scroll position.
            rebuild_all_view(&filtered, &state_c);
            state_c.stack.set_visible_child_name("all");
        });
    }

    // ── New Note button ───────────────────────────────────────────
    {
        let state_c = state.clone();
        let window_c = window.clone();
        new_note_btn.connect_clicked(move |_| {
            show_add_note_window(&window_c, state_c.clone());
        });
    }

    // ── Select initial view ───────────────────────────────────────
    let initial = initial_view.as_deref().unwrap_or("all");
    *state.active_view.borrow_mut() = initial.to_string();
    for row in sidebar_list
        .observe_children()
        .snapshot()
        .iter()
        .filter_map(|o| o.clone().downcast::<gtk4::ListBoxRow>().ok())
    {
        if row.widget_name() == initial {
            sidebar_list.select_row(Some(&row));
            break;
        }
    }
    stack.set_visible_child_name(initial);

    window.present();
    Ok(())
}

// ── Note list & cards ─────────────────────────────────────────────────────────

fn build_note_list(notes: &[Note], state: AppState) -> gtk4::ScrolledWindow {
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let list = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let mut sorted: Vec<Note> = notes.iter().filter(|n| !n.done).cloned().collect();
    sorted.sort_by(|a, b| b.created.cmp(&a.created));

    if sorted.is_empty() {
        list.append(
            &gtk4::Label::builder()
                .label("No notes here yet.")
                .margin_top(32)
                .build(),
        );
    } else {
        for note in &sorted {
            list.append(&build_note_card(note, state.clone()));
        }
    }

    scroll.set_child(Some(&list));
    scroll
}

fn build_note_card(note: &Note, state: AppState) -> gtk4::Box {
    let card = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(8)
        .margin_start(0)
        .margin_end(0)
        .margin_top(0)
        .margin_bottom(0)
        .css_classes(["note-card"])
        .build();
    card.add_css_class(&format!("note-card-{}", note.note_type.as_str()));

    // Top row: body + type chip
    let top_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .build();

    let body_label = gtk4::Label::builder()
        .label(&note.body)
        .hexpand(true)
        .xalign(0.0)
        .wrap(true)
        .build();

    let type_chip = gtk4::Label::builder()
        .label(note.note_type.as_str())
        .css_classes(["type-chip"])
        .build();

    top_row.append(&body_label);
    top_row.append(&type_chip);

    // Bottom row: metadata + action buttons
    let bottom_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .build();

    let created_str = {
        let local: chrono::DateTime<Local> = note.created.into();
        local.format("%b %d %H:%M").to_string()
    };
    let meta_label = gtk4::Label::builder()
        .label(&created_str)
        .css_classes(["dim-label"])
        .xalign(0.0)
        .build();

    // Date first, then chips
    bottom_row.append(&meta_label);
    if let Some(ws) = &note.workspace {
        bottom_row.append(
            &gtk4::Label::builder()
                .label(&format!("ws:{}", ws))
                .css_classes(["type-chip"])
                .build(),
        );
    }
    if let Some(t) = note.time {
        let local: chrono::DateTime<Local> = t.into();
        bottom_row.append(
            &gtk4::Label::builder()
                .label(&local.format("⏰ %b %d %H:%M").to_string())
                .css_classes(["dim-label"])
                .build(),
        );
    }
    if note.rrule.is_some() {
        bottom_row.append(
            &gtk4::Label::builder()
                .label("↻")
                .css_classes(["type-chip"])
                .build(),
        );
    }

    bottom_row.append(&gtk4::Box::builder().hexpand(true).build());

    // ✓ Done button
    let done_btn = gtk4::Button::builder()
        .label("✓")
        .css_classes(["action-btn", "done-btn"])
        .tooltip_text("Mark done")
        .build();
    {
        let note_id = note.id.clone();
        let card_c = card.clone();
        let state_c = state.clone();
        done_btn.connect_clicked(move |_| {
            card_c.set_visible(false); // optimistic hide
            let store = state_c.write_store();
            let id = note_id.clone();
            let state = state_c.clone();
            spawn_bg(
                move || -> anyhow::Result<Vec<Note>> {
                    if let Some(mut n) = store.get_by_id(&id)? {
                        n.mark_done();
                        store.update_note(&n)?;
                    }
                    store.load_all()
                },
                move |result| {
                    match result {
                        Ok(fresh) => {
                            *state.notes.borrow_mut() = fresh;
                            rebuild_stack(&state);
                            let active = state.active_view.borrow().clone();
                            state.stack.set_visible_child_name(&active);
                        }
                        Err(e) => state.log_error(format!("mark done failed: {}", e)),
                    }
                },
            );
        });
    }
    bottom_row.append(&done_btn);

    // ✎ Edit button
    let edit_btn = gtk4::Button::builder()
        .label("✎")
        .css_classes(["action-btn", "edit-btn"])
        .tooltip_text("Edit")
        .build();
    {
        let note_c = note.clone();
        let state_c = state.clone();
        let body_label_c = body_label.clone();
        let card_c = card.clone();

        edit_btn.connect_clicked(move |btn| {
            let morning = state_c.cfg.borrow().reminders.default_morning.clone();
            let store = Arc::new(state_c.write_store());

            let state_save = state_c.clone();
            let body_label_save = body_label_c.clone();
            let state_del = state_c.clone();
            let card_del = card_c.clone();
            let state_err = state_c.clone();

            let popover = editor::build_editor_popover(
                &note_c,
                store,
                morning,
                Rc::new(move |updated: Note| {
                    body_label_save.set_label(&updated.body);
                    state_save.reload_notes();
                    rebuild_stack(&state_save);
                    let active = state_save.active_view.borrow().clone();
                    state_save.stack.set_visible_child_name(&active);
                }),
                Rc::new(move || {
                    card_del.set_visible(false);
                    state_del.reload_notes();
                    rebuild_stack(&state_del);
                    let active = state_del.active_view.borrow().clone();
                    state_del.stack.set_visible_child_name(&active);
                }),
                Rc::new(move |e: String| {
                    state_err.log_error(e);
                }),
            );
            popover.set_parent(btn);
            popover.popup();
        });
    }
    bottom_row.append(&edit_btn);

    // 🗑 Delete button — two-click confirm: first click → "Sure?", second → delete
    let delete_btn = gtk4::Button::builder()
        .label("🗑")
        .css_classes(["action-btn", "danger-btn"])
        .tooltip_text("Delete")
        .build();
    {
        use std::cell::RefCell;
        use std::rc::Rc;
        let confirming = Rc::new(RefCell::new(false));
        let note_id = note.id.clone();
        let card_c = card.clone();
        let state_c = state.clone();
        let btn_c = delete_btn.clone();

        delete_btn.connect_clicked(move |_| {
            if *confirming.borrow() {
                card_c.set_visible(false); // optimistic hide
                let store = state_c.write_store();
                let id = note_id.clone();
                let state = state_c.clone();
                spawn_bg(
                    move || -> anyhow::Result<Vec<Note>> {
                        store.delete_note(&id)?;
                        if let Err(e) = Scheduler::cancel(&id) {
                            tracing::warn!("failed to cancel timer for {}: {}", id, e);
                        }
                        store.load_all()
                    },
                    move |result| {
                        match result {
                            Ok(fresh) => {
                                *state.notes.borrow_mut() = fresh;
                                rebuild_stack(&state);
                                let active = state.active_view.borrow().clone();
                                state.stack.set_visible_child_name(&active);
                            }
                            Err(e) => state.log_error(format!("delete failed: {}", e)),
                        }
                    },
                );
            } else {
                *confirming.borrow_mut() = true;
                btn_c.set_label("Sure?");
            }
        });
    }
    bottom_row.append(&delete_btn);

    card.append(&top_row);
    card.append(&bottom_row);
    card
}

// ── Add note window ───────────────────────────────────────────────────────────

fn show_add_note_window(parent: &gtk4::ApplicationWindow, state: AppState) {
    let win = gtk4::Window::builder()
        .title("New Note")
        .transient_for(parent)
        .modal(true)
        .default_width(500)
        .build();

    let vbox = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(10)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    vbox.append(&gtk4::Label::builder().label("Body").xalign(0.0).build());
    let body_entry = gtk4::Entry::builder()
        .placeholder_text("What's on your mind?")
        .hexpand(true)
        .build();
    vbox.append(&body_entry);

    // Type chips
    let chip_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(4)
        .build();
    let selected_type: Rc<RefCell<NoteType>> = Rc::new(RefCell::new(NoteType::Note));
    let chips: Vec<(gtk4::Button, NoteType)> = NoteType::all_builtin()
        .iter()
        .map(|&name| {
            let btn = gtk4::Button::builder()
                .label(name)
                .css_classes(["type-chip"])
                .build();
            (btn, NoteType::from_str(name))
        })
        .collect();
    for (btn, nt) in &chips {
        let sel = selected_type.clone();
        let nt_c = nt.clone();
        let all_btns: Vec<gtk4::Button> = chips.iter().map(|(b, _)| b.clone()).collect();
        btn.connect_clicked(move |clicked| {
            *sel.borrow_mut() = nt_c.clone();
            for b in &all_btns { b.remove_css_class("active"); }
            clicked.add_css_class("active");
        });
        chip_box.append(btn);
    }
    if let Some((btn, _)) = chips.iter().find(|(_, nt)| *nt == NoteType::Note) {
        btn.add_css_class("active");
    }
    vbox.append(&chip_box);

    vbox.append(&gtk4::Label::builder().label("Time (optional)").xalign(0.0).build());
    let time_entry = gtk4::Entry::builder()
        .placeholder_text("tomorrow 9am  /  at 7pm  /  in 30 minutes")
        .hexpand(true)
        .build();
    vbox.append(&time_entry);

    vbox.append(&gtk4::Label::builder().label("Recurrence (optional)").xalign(0.0).build());
    let rrule_entry = gtk4::Entry::builder()
        .placeholder_text("RRULE:FREQ=WEEKLY;BYDAY=MO")
        .hexpand(true)
        .build();
    vbox.append(&rrule_entry);

    let status_label = gtk4::Label::builder()
        .label("")
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    vbox.append(&status_label);

    let btn_row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .build();
    let cancel_btn = gtk4::Button::builder().label("Cancel").build();
    let add_btn = gtk4::Button::builder()
        .label("Add Note")
        .css_classes(["confirm-button"])
        .build();
    btn_row.append(&gtk4::Box::builder().hexpand(true).build());
    btn_row.append(&cancel_btn);
    btn_row.append(&add_btn);
    vbox.append(&btn_row);

    win.set_child(Some(&vbox));

    // Cancel
    {
        let win_c = win.clone();
        cancel_btn.connect_clicked(move |_| win_c.close());
    }

    // Shared add-note logic — called by both the button and the Enter key.
    let do_add: Rc<dyn Fn()> = Rc::new({
        let win = win.clone();
        let state = state.clone();
        let body_entry = body_entry.clone();
        let time_entry = time_entry.clone();
        let rrule_entry = rrule_entry.clone();
        let selected_type = selected_type.clone();
        let status_label = status_label.clone();

        move || {
            let body_text = body_entry.text().to_string();
            if body_text.trim().is_empty() {
                status_label.set_label("Body is required.");
                return;
            }

            let morning = state.cfg.borrow().reminders.default_morning.clone();
            let parsed = parse_rule_based(&body_text, &morning);
            let user_type = selected_type.borrow().clone();
            let default_type = NoteType::from_str(&state.cfg.borrow().settings.default_type);

            let mut note = Note::new(parsed.body.clone(), user_type.clone(), None);
            if user_type == default_type {
                note.note_type = parsed.note_type;
            }
            note.time = parsed.time;
            note.rrule = parsed.rrule;

            let time_str = time_entry.text().to_string();
            if !time_str.trim().is_empty() {
                let tp = parse_rule_based(&time_str, &morning);
                if tp.time.is_some() { note.time = tp.time; }
                if tp.rrule.is_some() { note.rrule = tp.rrule; }
            }

            let rrule_str = rrule_entry.text().to_string();
            if !rrule_str.trim().is_empty() {
                note.rrule = Some(RecurrenceRule::new(rrule_str));
            }

            let store = state.write_store();
            win.close();

            let state_bg = state.clone();
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
                move |result| {
                    match result {
                        Ok(fresh) => {
                            *state_bg.notes.borrow_mut() = fresh;
                            rebuild_stack(&state_bg);
                            let active = state_bg.active_view.borrow().clone();
                            state_bg.stack.set_visible_child_name(&active);
                        }
                        Err(e) => state_bg.log_error(format!("save failed: {}", e)),
                    }
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
        let time_entry = time_entry.clone();
        let rrule_entry = rrule_entry.clone();
        body_entry.connect_activate(move |_| {
            if time_entry.text().is_empty() && rrule_entry.text().is_empty() {
                do_add();
            }
        });
    }

    win.present();
    body_entry.grab_focus();
}

// ── CSS ───────────────────────────────────────────────────────────────────────

fn apply_css(_cfg: &Config) {
    let palette = load_palette();
    let user_css = std::fs::read_to_string(breadpad_shared::config::style_css_path()).ok();
    let css = build_css(&palette, user_css.as_deref());

    let provider = gtk4::CssProvider::new();
    provider.load_from_string(&css);
    let Some(display) = gtk4::gdk::Display::default() else {
        tracing::warn!("no default display; skipping CSS provider");
        return;
    };
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
