//! The main window: the sidebar, a section of notes or the settings, and
//! the search that looks through every note.

mod category_picker;
mod date_filter;
mod help;
mod notes;
mod settings;
mod trash;

use std::time::Duration;

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Key};
use posthaste_model::{AppInfo, CommandError, Counts, DetailsFrom, Note, Os, Status};
use wasm_bindgen::JsCast;

use crate::browser::{self, bind_text};
use crate::components::{
    Button, Icon, IconButton, IconName, Kbd, SceneHost, Segmented, Toaster, Toasts,
};
use crate::ipc::{api, events};
use crate::logic::dates::Day;
use crate::logic::inbox::{
    build_query, key_action, Filters, KeyAction, KeyContext, KeyPress, StatusFilter, View,
};
use crate::logic::selection::{moved_from_none, neighbour};
use crate::logic::shortcut::{format_accelerator, has_mod, mod_cap};
use crate::state::{
    use_appearance, use_i18n, use_settings, use_updates, I18n, Message, SettingsHandle,
};
use crate::time;
use posthaste_model::Window as SceneWindow;

/// How many notes a section shows before "Show older notes".
const PAGE: usize = 200;
/// A short pause while typing keeps a fast typist from queueing queries.
const SEARCH_PAUSE: Duration = Duration::from_millis(90);

fn view_title(view: View) -> Key {
    match view {
        View::Inbox => key::INBOX_NAV_INBOX,
        View::Today => key::INBOX_NAV_TODAY,
        View::Later => key::INBOX_NAV_LATER,
        View::Archive => key::INBOX_NAV_ARCHIVE,
        View::Trash => key::INBOX_NAV_TRASH,
        View::Settings => key::INBOX_NAV_SETTINGS,
    }
}

fn view_icon(view: View) -> IconName {
    match view {
        View::Inbox => IconName::Inbox,
        View::Today => IconName::Sun,
        View::Later => IconName::Later,
        View::Archive => IconName::Archive,
        View::Trash => IconName::Trash,
        View::Settings => IconName::Settings,
    }
}

/// Everything the window's actions read and write. `Copy`, so each handler
/// and each part of the window takes it by value.
#[derive(Clone, Copy)]
struct Main {
    os: Os,
    i18n: I18n,
    settings: SettingsHandle,
    toaster: Toaster,
    /// The locale dates and times are written in.
    locale: Memo<String>,
    view: RwSignal<View>,
    filters: RwSignal<Filters>,
    filters_open: RwSignal<bool>,
    /// The category picker, opened with `#`.
    picker_open: RwSignal<bool>,
    /// What the last query answered: up to one more than `limit`, which is
    /// how the list knows there are older notes.
    notes: RwSignal<Vec<Note>>,
    /// The first answer for this section has arrived.
    loaded: RwSignal<bool>,
    limit: RwSignal<usize>,
    counts: RwSignal<Option<Counts>>,
    selected: RwSignal<Option<String>>,
    editing: RwSignal<Option<String>>,
    help_open: RwSignal<bool>,
    /// Bumped to run the list query again: data changed, or a new day began.
    revision: RwSignal<u32>,
    /// Counts queries, so a slow answer to an old one cannot replace a newer.
    request: StoredValue<u32>,
    search: NodeRef<html::Input>,
}

impl Main {
    fn new(os: Os, i18n: I18n, settings: SettingsHandle) -> Self {
        Main {
            os,
            i18n,
            settings,
            toaster: Toaster::new(),
            locale: Memo::new(move |_| time::locale_for(i18n.translator().language().code)),
            view: RwSignal::new(View::Inbox),
            filters: RwSignal::new(Filters::default()),
            filters_open: RwSignal::new(false),
            picker_open: RwSignal::new(false),
            notes: RwSignal::new(Vec::new()),
            loaded: RwSignal::new(false),
            limit: RwSignal::new(PAGE),
            counts: RwSignal::new(None),
            selected: RwSignal::new(None),
            editing: RwSignal::new(None),
            help_open: RwSignal::new(false),
            revision: RwSignal::new(0),
            request: StoredValue::new(0),
            search: NodeRef::new(),
        }
    }

    fn fail(self, error: CommandError) {
        self.toaster.fail(error);
    }

    fn searching(self) -> bool {
        self.filters.with(Filters::is_searching)
    }

    /// Trash lists its own notes; it only shares the search box.
    fn shows_trash(self) -> bool {
        self.view.get() == View::Trash && !self.searching()
    }

    fn refresh(self) {
        self.revision
            .update(|revision| *revision = revision.wrapping_add(1));
    }

    fn go(self, next: View) {
        let switching = self.view.get_untracked() != next;
        self.view.set(next);
        // The category sits on top of the note views and follows them; a
        // view that cannot show it (Trash, Settings) takes it off too, so
        // nothing filters a list the user cannot see the reason for.
        let keep_category = !matches!(next, View::Trash | View::Settings);
        self.filters.update(|filters| {
            let category = keep_category.then(|| filters.category.clone()).flatten();
            *filters = Filters {
                category,
                ..Filters::default()
            };
        });
        self.picker_open.set(false);
        self.filters_open.set(false);
        self.limit.set(PAGE);
        self.editing.set(None);
        self.selected.set(None);
        if switching {
            self.loaded.set(false);
        }
    }

    /// Ends the search, keeping the category filter.
    fn clear_search(self) {
        self.filters.update(|filters| {
            filters.text = String::new();
            filters.status = StatusFilter::default();
            filters.date = None;
        });
    }

    /// Filters the list by one category, from the picker or a row.
    fn set_category(self, name: String) {
        self.limit.set(PAGE);
        self.picker_open.set(false);
        self.selected.set(None);
        self.filters.update(|filters| filters.category = Some(name));
    }

    /// Takes the category filter off.
    fn clear_category(self) {
        self.limit.set(PAGE);
        self.selected.set(None);
        self.filters.update(|filters| filters.category = None);
    }

    fn set_search_text(self, text: String) {
        self.limit.set(PAGE);
        self.filters.update(|filters| filters.text = text);
    }

    fn set_date(self, date: Option<Day>) {
        self.limit.set(PAGE);
        self.filters.update(|filters| filters.date = date);
    }

    /// Search works on notes, so it leaves Settings first.
    fn focus_search(self) {
        if self.view.get_untracked() == View::Settings {
            self.go(View::Inbox);
        }
        request_animation_frame(move || {
            if let Some(search) = self.search.get_untracked() {
                let _ = search.focus();
            }
        });
    }

    fn blur_search(self) {
        if let Some(search) = self.search.get_untracked() {
            let _ = search.blur();
        }
    }

    fn new_note(self) {
        spawn_local(async move {
            if let Err(error) = api::capture_open().await {
                self.fail(error);
            }
        });
    }

    fn toggle_sidebar(self) {
        spawn_local(async move {
            if let Err(error) = self
                .settings
                .update(|s| s.sidebar_collapsed = !s.sidebar_collapsed)
                .await
            {
                self.fail(error);
            }
        });
    }

    /// Fetches the notes of the section, or of the search. Skipped where
    /// another part of the window lists its own.
    fn load(self) {
        let view = self.view.get();
        let filters = self.filters.get();
        let limit = self.limit.get();
        self.revision.with(|_| ());
        if view == View::Settings || (view == View::Trash && !filters.is_searching()) {
            return;
        }
        let request = self.request.get_value().wrapping_add(1);
        self.request.set_value(request);
        let pause = if filters.text.is_empty() {
            Duration::ZERO
        } else {
            SEARCH_PAUSE
        };
        let run = move || {
            if self.request.get_value() != request {
                return;
            }
            let mut query = build_query(
                view,
                &filters,
                time::bounds(time::today()),
                filters.date.map(time::bounds),
            );
            query.limit = Some(limit as u32 + 1);
            spawn_local(async move {
                let answer = api::notes_list(&query).await;
                if self.request.get_value() != request {
                    return;
                }
                match answer {
                    Ok(notes) => {
                        self.notes.set(notes);
                        self.loaded.set(true);
                    }
                    Err(error) => self.fail(error),
                }
            });
        };
        if pause.is_zero() {
            run();
        } else {
            set_timeout(run, pause);
        }
    }

    fn load_counts(self) {
        self.revision.with(|_| ());
        let (from, to) = time::bounds(time::today());
        spawn_local(async move {
            // The sidebar still works without its numbers.
            if let Ok(counts) = api::notes_counts(from, to).await {
                self.counts.set(Some(counts));
            }
        });
    }

    /// The ids of the notes on screen, in order.
    fn with_visible_ids<T>(self, read: impl FnOnce(&[&str]) -> T) -> T {
        let limit = self.limit.get_untracked();
        self.notes.with_untracked(|notes| {
            let ids: Vec<&str> = notes.iter().take(limit).map(|n| n.id.as_str()).collect();
            read(&ids)
        })
    }

    fn selected_note(self) -> Option<Note> {
        let id = self.selected.get_untracked()?;
        let limit = self.limit.get_untracked();
        self.notes
            .with_untracked(|notes| notes.iter().take(limit).find(|n| n.id == id).cloned())
    }

    // ---- what can be done to a note --------------------------------------

    /// Moves a note to `status` and offers to put it back.
    fn set_status(self, note: &Note, status: Status, done: Key) {
        let (id, before) = (note.id.clone(), note.status);
        spawn_local(async move {
            match api::note_set_status(&id, status).await {
                Ok(()) => self.toaster.undoable(Message::Plain(done), move || {
                    let id = id.clone();
                    spawn_local(async move {
                        if let Err(error) = api::note_set_status(&id, before).await {
                            self.fail(error);
                        }
                    });
                }),
                Err(error) => self.fail(error),
            }
        });
    }

    fn toggle_complete(self, note: &Note) {
        if note.status == Status::Completed {
            self.set_status(note, Status::Inbox, key::INBOX_TOAST_NOT_COMPLETED);
        } else {
            self.set_status(note, Status::Completed, key::INBOX_TOAST_COMPLETED);
        }
    }

    fn archive(self, note: &Note) {
        self.set_status(note, Status::Archived, key::INBOX_TOAST_ARCHIVED);
    }

    fn restore(self, note: &Note) {
        self.set_status(note, Status::Inbox, key::INBOX_TOAST_TO_INBOX);
    }

    /// Parks the note in Later, or brings it back to the inbox.
    fn toggle_later(self, note: &Note) {
        let id = note.id.clone();
        let later = note.later_at.is_none();
        let done = if later {
            key::INBOX_TOAST_TO_LATER
        } else {
            key::INBOX_TOAST_TO_INBOX
        };
        spawn_local(async move {
            match api::note_set_later(&id, later).await {
                Ok(()) => self.toaster.undoable(Message::Plain(done), move || {
                    let id = id.clone();
                    spawn_local(async move {
                        if let Err(error) = api::note_set_later(&id, !later).await {
                            self.fail(error);
                        }
                    });
                }),
                Err(error) => self.fail(error),
            }
        });
    }

    fn remove(self, note: &Note) {
        let id = note.id.clone();
        spawn_local(async move {
            match api::note_delete(&id).await {
                Ok(()) => {
                    self.toaster
                        .undoable(Message::Plain(key::INBOX_TOAST_TRASHED), move || {
                            let id = id.clone();
                            spawn_local(async move {
                                if let Err(error) = api::note_undelete(&id).await {
                                    self.fail(error);
                                }
                            });
                        })
                }
                Err(error) => self.fail(error),
            }
        });
    }

    fn copy(self, note: &Note) {
        let text = note.content.clone();
        spawn_local(async move {
            if browser::copy_text(&text).await {
                self.toaster.info(Message::Plain(key::INBOX_TOAST_COPIED));
            } else {
                self.toaster
                    .error(Message::Plain(key::INBOX_TOAST_COPY_FAILED));
            }
        });
    }

    fn start_edit(self, id: &str) {
        self.selected.set(Some(id.to_owned()));
        self.editing.set(Some(id.to_owned()));
    }

    fn open_details(self, note: &Note) {
        let id = note.id.clone();
        self.selected.set(Some(id.clone()));
        spawn_local(async move {
            if let Err(error) = api::details_open(&id, DetailsFrom::Inbox).await {
                self.fail(error);
            }
        });
    }

    /// Stores an edit. `false` when it could not be stored; the editor then
    /// stays open with the text in it.
    async fn save_edit(self, id: &str, content: &str, category: Option<&str>) -> bool {
        match api::note_update(id, content, category).await {
            Ok(()) => {
                self.editing.set(None);
                true
            }
            Err(error) => {
                self.fail(error);
                false
            }
        }
    }

    fn on_key(self, event: KeyboardEvent) {
        if settings::is_recording_shortcut() {
            return;
        }
        let key = event.key();
        let in_search = self.search.get_untracked().is_some_and(|search| {
            event
                .target()
                .is_some_and(|target| target.loose_eq(search.unchecked_ref()))
        });
        let selected = self.selected_note();
        let press = KeyPress {
            key: &key,
            primary: has_mod(event.meta_key(), event.ctrl_key(), self.os),
            alt: event.alt_key(),
            composing: event.is_composing(),
        };
        let context = KeyContext {
            view: self.view.get_untracked(),
            searching: self.filters.with_untracked(Filters::is_searching),
            dialog_open: browser::dialog_is_open(),
            typing: browser::is_typing(&event),
            in_search,
            on_control: browser::is_within(&event, "button, a, [role=radio]"),
            has_rows: self.with_visible_ids(|ids| !ids.is_empty()),
            has_category: self.filters.with_untracked(|f| f.category.is_some()),
            selected: selected.as_ref().map(|note| note.status),
        };
        let Some(keyed) = key_action(press, context) else {
            return;
        };
        event.prevent_default();
        // Worked out before the note leaves the list.
        let next = selected
            .as_ref()
            .filter(|_| keyed.advance)
            .map(|note| self.with_visible_ids(|ids| neighbour(ids, &note.id)));
        match (keyed.action, selected) {
            (KeyAction::Go(view), _) => self.go(view),
            (KeyAction::FocusSearch, _) => self.focus_search(),
            (KeyAction::NewNote, _) => self.new_note(),
            (KeyAction::ToggleSidebar, _) => self.toggle_sidebar(),
            (KeyAction::Help, _) => self.help_open.set(true),
            (KeyAction::LeaveSearch, _) => {
                self.clear_search();
                self.blur_search();
            }
            (KeyAction::SearchToList, _) => {
                self.blur_search();
                let first = self.with_visible_ids(|ids| ids.first().map(|id| (*id).to_owned()));
                self.selected.set(first);
            }
            (KeyAction::Move(delta), _) => {
                let current = self.selected.get_untracked();
                let moved =
                    self.with_visible_ids(|ids| moved_from_none(ids, current.as_deref(), delta));
                if moved.is_some() {
                    self.selected.set(moved);
                }
            }
            (KeyAction::ClearSearch, _) => self.clear_search(),
            (KeyAction::ClearCategory, _) => self.clear_category(),
            (KeyAction::PickCategory, _) => self.picker_open.set(true),
            (KeyAction::ClearSelection, _) => self.selected.set(None),
            (KeyAction::Edit, Some(note)) => self.start_edit(&note.id),
            (KeyAction::ToggleComplete, Some(note)) => self.toggle_complete(&note),
            (KeyAction::Archive, Some(note)) => self.archive(&note),
            (KeyAction::Restore, Some(note)) => self.restore(&note),
            (KeyAction::ToggleLater, Some(note)) => self.toggle_later(&note),
            (KeyAction::Copy, Some(note)) => self.copy(&note),
            (KeyAction::OpenDetails, Some(note)) => self.open_details(&note),
            (KeyAction::Delete, Some(note)) => self.remove(&note),
            // `key_action` only asks for these with a note selected.
            (
                KeyAction::Edit
                | KeyAction::ToggleComplete
                | KeyAction::Archive
                | KeyAction::Restore
                | KeyAction::ToggleLater
                | KeyAction::Copy
                | KeyAction::OpenDetails
                | KeyAction::Delete,
                None,
            ) => {}
        }
        if let Some(next) = next {
            self.selected.set(next);
        }
    }
}

/// The three distinct content panes mounted inside the main scroll area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    LoadingSettings,
    Settings,
    Trash,
    Notes,
}

#[component]
pub fn MainWindow(info: AppInfo) -> impl IntoView {
    let os = info.os;
    let settings = use_settings();
    let appearance = use_appearance(settings);
    let i18n = use_i18n(settings);
    let updates = use_updates();
    let main = Main::new(os, i18n, settings);

    Effect::new(move |_| main.load());
    Effect::new(move |_| main.load_counts());
    events::on::<events::NotesChanged>(move |()| main.refresh());
    events::on::<events::Navigate>(move |target| {
        if let Some(view) = View::parse(&target) {
            main.go(view);
        }
    });
    // From the "where it came from" tag in Today: the notes captured that day.
    events::on::<events::InboxShowDate>(move |day| {
        main.go(View::Inbox);
        main.filters
            .update(|filters| filters.date = Day::parse(&day));
        main.filters_open.set(true);
    });
    // "Today" depends on the clock: refresh when the window comes back.
    let refocus = window_event_listener(leptos::ev::focus, move |_| main.refresh());
    let keys = window_event_listener(leptos::ev::keydown, move |event| main.on_key(event));
    // Closing the window only hides it, so a category filter would outlive
    // the look at the list it belonged to: take it off when the window goes.
    let hidden = window_event_listener(leptos::ev::visibilitychange, move |_| {
        if document().visibility_state() == web_sys::VisibilityState::Hidden {
            main.picker_open.set(false);
            main.filters.update(|filters| filters.category = None);
        }
    });
    on_cleanup(move || {
        refocus.remove();
        keys.remove();
        hidden.remove();
    });

    let capture_keys = Memo::new(move |_| {
        settings.shortcuts.with(|report| match report {
            Some(report) if report.capture.registered && !report.capture.accelerator.is_empty() => {
                format_accelerator(&report.capture.accelerator, os)
            }
            _ => Vec::new(),
        })
    });
    let in_settings = move || main.view.get() == View::Settings;
    let pane = Memo::new(move |_| {
        if in_settings() {
            if settings.settings.with(Option::is_some) {
                Pane::Settings
            } else {
                Pane::LoadingSettings
            }
        } else if main.shows_trash() {
            Pane::Trash
        } else {
            Pane::Notes
        }
    });
    let title = move || {
        if main.searching() {
            key::INBOX_SEARCH_TITLE
        } else {
            view_title(main.view.get())
        }
    };
    let count = move |view: View| {
        main.counts.with(|counts| {
            let counts = counts.as_ref()?;
            let count = match view {
                View::Inbox => counts.inbox,
                View::Today => counts.today,
                View::Later => counts.later,
                View::Trash => counts.trash,
                View::Archive | View::Settings => return None,
            };
            (count > 0).then_some(count)
        })
    };
    let sidebar_collapsed = Memo::new(move |_| {
        settings
            .settings
            .with(|s| s.as_ref().is_some_and(|s| s.sidebar_collapsed))
    });
    let sidebar_shortcut = format!("{}+B", mod_cap(os));
    let sidebar_toggle_label = Signal::derive(move || {
        let key = if sidebar_collapsed.get() {
            key::INBOX_SIDEBAR_EXPAND
        } else {
            key::INBOX_SIDEBAR_COLLAPSE
        };
        i18n.translator()
            .format(key, &[("keys", &sidebar_shortcut)])
    });
    let nav_item = move |view: View| {
        // A search is its own place: no section is the current one then.
        let current = move || {
            let here = main.view.get() == view && (view == View::Settings || !main.searching());
            here.then_some("page")
        };
        let label = i18n.label(view_title(view));
        view! {
            <button
                type="button"
                class="nav__item"
                data-collapsed=move || sidebar_collapsed.get().then_some("")
                aria-current=current
                aria-label=move || sidebar_collapsed.get().then(|| label.get())
                title=move || sidebar_collapsed.get().then(|| label.get())
                on:click=move |_| main.go(view)
            >
                <Icon name=view_icon(view) />
                <span class="nav__label">{i18n.text(view_title(view))}</span>
                {move || count(view).map(|count| view! { <span class="nav__count">{count}</span> })}
            </button>
        }
    };
    let shortcut_broken = move || {
        settings.shortcuts.with(|report| {
            report
                .as_ref()
                .and_then(|r| (!r.capture.registered).then(|| r.capture.error.clone()))
        })
    };
    let install = move |()| {
        spawn_local(async move {
            if let Err(error) = updates.install().await {
                main.fail(error);
            }
        });
    };
    let class = match os {
        Os::Macos => "app os-macos",
        Os::Windows => "app os-windows",
        Os::Linux => "app os-linux",
    };
    let info = StoredValue::new(info);

    view! {
        <div class=class data-collapsed=move || sidebar_collapsed.get().then_some("")>
            <aside class="sidebar" data-collapsed=move || sidebar_collapsed.get().then_some("")>
                <div class="sidebar__header">
                    <div class="sidebar__brand">"Posthaste"</div>
                    <IconButton
                        class="sidebar__toggle"
                        icon=IconName::Sidebar
                        label=sidebar_toggle_label
                        expanded=Signal::derive(move || !sidebar_collapsed.get())
                        on_click=move |()| main.toggle_sidebar()
                    />
                </div>
                <nav class="nav" aria-label=i18n.text(key::INBOX_SECTIONS)>
                    {View::LISTS.into_iter().map(nav_item).collect_view()}
                </nav>
                <div class="sidebar__spacer"></div>
                <button
                    type="button"
                    class="btn sidebar__new"
                    data-collapsed=move || sidebar_collapsed.get().then_some("")
                    aria-label=move || sidebar_collapsed.get().then(|| i18n.translator().text(key::INBOX_NEW_NOTE).to_owned())
                    title=move || sidebar_collapsed.get().then(|| i18n.translator().text(key::INBOX_NEW_NOTE).to_owned())
                    on:click=move |_| main.new_note()
                >
                    <span class="sidebar__new__icon">
                        <Icon name=IconName::Plus />
                    </span>
                    <span class="sidebar__new__label">
                        {i18n.text(key::INBOX_NEW_NOTE)}
                    </span>
                    <Show when=move || !sidebar_collapsed.get() && capture_keys.with(|keys| !keys.is_empty())>
                        <Kbd keys=capture_keys />
                    </Show>
                </button>
                {nav_item(View::Settings)}
            </aside>

            <SceneHost window=SceneWindow::Main>
                <main class="content">
                <header class="content__header" data-tauri-drag-region=(os == Os::Macos).then_some("")>
                    <h1 class="content__title">{move || i18n.translator().text(title())}</h1>
                    <Show when=move || !in_settings()>
                        <div class="search" role="search">
                            <span class="search__icon">
                                <Icon name=IconName::Search size=14 />
                            </span>
                            <input
                                node_ref=main.search
                                class="field"
                                type="text"
                                aria-label=i18n.text(key::INBOX_SEARCH_LABEL)
                                placeholder=i18n.text(key::INBOX_SEARCH_LABEL)
                                use:bind_text=Signal::derive(move || main.filters.with(|filters| filters.text.clone()))
                                on:input=move |event| main.set_search_text(event_target_value(&event))
                                spellcheck="false"
                            />
                            <Show when=move || main.searching()>
                                <IconButton
                                    class="search__clear"
                                    icon=IconName::Close
                                    label=i18n.label(key::INBOX_SEARCH_CLEAR)
                                    on_click=move |()| main.clear_search()
                                />
                            </Show>
                        </div>
                        <IconButton
                            icon=IconName::Filter
                            label=i18n.label(key::INBOX_FILTER_TOGGLE)
                            expanded=Signal::derive(move || main.searching() || main.filters_open.get())
                            on_click=move |()| main.filters_open.update(|open| *open = !*open)
                        />
                    </Show>
                </header>

                {move || (!in_settings()).then(shortcut_broken).flatten().map(|reason| view! {
                    <div class="banner" role="alert">
                        <div class="banner__text">
                            <span class="banner__title">{i18n.text(key::INBOX_BANNER_SHORTCUT_TITLE)} " "</span>
                            {reason}
                        </div>
                        <Button on_click=move |()| main.go(View::Settings)>
                            {i18n.text(key::INBOX_BANNER_SHORTCUT_ACTION)}
                        </Button>
                    </div>
                })}

                {move || (!in_settings()).then(|| updates.found.get()).flatten().map(|update| view! {
                    <div class="banner" role="status">
                        <div class="banner__text">
                            <span class="banner__title">
                                {move || i18n.translator().format(
                                    key::INBOX_BANNER_UPDATE_TITLE,
                                    &[("version", &update.version)],
                                )}
                                " "
                            </span>
                            {i18n.text(key::INBOX_BANNER_UPDATE_TEXT)}
                        </div>
                        <Button busy=updates.installing on_click=install>
                            {i18n.text(key::INBOX_BANNER_UPDATE_ACTION)}
                        </Button>
                    </div>
                })}

                <Show when=move || !in_settings() && (main.searching() || main.filters_open.get())>
                    <FilterBar main=main />
                </Show>

                <div class="content__scroll">
                    <div class="content__column">
                        {move || {
                            let named = main.filters.with(|f| f.category.clone());
                            (pane.get() == Pane::Notes && named.is_some()).then(|| {
                                let t = i18n.translator();
                                let name = t
                                    .format(key::INBOX_FILTER_CATEGORY_LINE, &[("name", &named.unwrap_or_default())]);
                                view! {
                                    <div class="catfilter">
                                        <span class="catfilter__name">{name}</span>
                                        <IconButton
                                            icon=IconName::Close
                                            label=i18n.label(key::INBOX_FILTER_CATEGORY_CLEAR)
                                            on_click=move |()| main.clear_category()
                                        />
                                    </div>
                                }
                                .into_any()
                            })
                        }}
                        <div class="cat-anchor">
                            <Show when=move || main.picker_open.get()>
                                <category_picker::CategoryPicker main=main />
                            </Show>
                        </div>
                        {move || match pane.get() {
                            Pane::LoadingSettings => ().into_any(),
                            Pane::Settings => view! {
                                <settings::SettingsView
                                    info=info.get_value()
                                    i18n=i18n
                                    settings=settings
                                    appearance=appearance
                                    updates=updates
                                    toaster=main.toaster
                                />
                            }
                            .into_any(),
                            Pane::Trash => view! {
                                <trash::TrashView i18n=i18n locale=main.locale toaster=main.toaster />
                            }
                            .into_any(),
                            Pane::Notes => view! {
                                <notes::NoteList main=main capture_keys=capture_keys />
                            }
                            .into_any(),
                        }}
                    </div>
                </div>
                </main>
            </SceneHost>

            <Toasts toaster=main.toaster i18n=i18n />
            <help::ShortcutHelp
                i18n=i18n
                open=main.help_open
                mod_key=mod_cap(os)
            />
        </div>
    }
}

/// The controls that narrow a search: status and the day of capture.
#[component]
fn FilterBar(main: Main) -> impl IntoView {
    let i18n = main.i18n;
    let found = move || {
        (main.searching() && main.loaded.get()).then(|| {
            let limit = main.limit.get();
            let (shown, more) = main
                .notes
                .with(|notes| (notes.len().min(limit) as u64, notes.len() > limit));
            let t = i18n.translator();
            match (shown, more) {
                (0, _) => t.text(key::INBOX_FILTER_NO_MATCHES).to_owned(),
                (shown, false) => t.plural(key::INBOX_FILTER_COUNT, shown, &[]),
                (shown, true) => t.plural(key::INBOX_FILTER_COUNT_MORE, shown, &[]),
            }
        })
    };
    view! {
        <div class="filters">
            <Segmented
                i18n=i18n
                label=key::INBOX_FILTER_STATUS
                value=Signal::derive(move || main.filters.with(|filters| filters.status))
                // The status narrows a search; on its own it searches nothing.
                disabled=Signal::derive(move || !main.searching())
                options=vec![
                    (StatusFilter::All, key::INBOX_FILTER_ALL),
                    (StatusFilter::Open, key::INBOX_FILTER_OPEN),
                    (StatusFilter::Completed, key::INBOX_FILTER_COMPLETED),
                    (StatusFilter::Archived, key::INBOX_FILTER_ARCHIVED),
                ]
                on_change=move |status| main.filters.update(|filters| filters.status = status)
            />
            <date_filter::DateFilter
                i18n=i18n
                locale=main.locale
                value=Signal::derive(move || main.filters.with(|filters| filters.date))
                on_change=move |day| main.set_date(day)
            />
            {move || found().map(|text| view! {
                <span class="filters__count" aria-live="polite">{text}</span>
            })}
        </div>
    }
}
