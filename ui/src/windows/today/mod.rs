//! The Today window: what is on today's plate and how much of it is done.
//!
//! ← and → show earlier days, read-only and rebuilt from timestamps. There
//! are no future days, because notes have no dates.

mod list;

use std::collections::HashSet;

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::{CommandError, Glance, Note, Os, Project, Status};
use wasm_bindgen::JsCast;

use crate::components::{Calendar, DayMarks, IconButton, IconName, ProjectPicker};
use crate::ipc::{self, api, events};
use crate::logic::dates::Day;
use crate::logic::project_picker::{PickerMode, PickerRow};
use crate::logic::sections;
use crate::logic::shortcut::format_accelerator;
use crate::logic::today::{
    clamp_to_past, key_action, moved_selection, neighbour, selection_after_load, visible_ids,
    KeyAction, KeyContext, KeyPress,
};
use crate::state::{use_appearance, use_i18n, use_settings};
use crate::time::{self, DateStyle, Width};

use list::{Hints, Notes, Nothing, Progress};

/// Everything the window's actions read and write. `Copy`, so each handler
/// takes it by value.
#[derive(Clone, Copy)]
pub(super) struct Today {
    pub(super) glance: RwSignal<Option<Glance>>,
    pub(super) error: RwSignal<Option<CommandError>>,
    /// The id of the selected row.
    pub(super) selected: RwSignal<Option<String>>,
    /// The day being looked at. `None` follows today, across midnight too.
    pub(super) view_day: RwSignal<Option<Day>>,
    /// Today's date as of the last load, so a window left open overnight
    /// redraws for the new day instead of trusting an old clock reading.
    pub(super) today: RwSignal<Day>,
    pub(super) calendar_open: RwSignal<bool>,
    pub(super) marks: RwSignal<DayMarks>,
    pub(super) collapsed_sections: RwSignal<HashSet<String>>,
    pub(super) picker_open: RwSignal<bool>,
    pub(super) picker_projects: RwSignal<Vec<Project>>,
    pub(super) list: NodeRef<html::Div>,
    /// Counts loads, so a slow answer to an old one cannot replace a newer.
    pub(super) request: StoredValue<u32>,
}

impl Today {
    fn new() -> Self {
        Today {
            glance: RwSignal::new(None),
            error: RwSignal::new(None),
            selected: RwSignal::new(None),
            view_day: RwSignal::new(None),
            today: RwSignal::new(time::today()),
            calendar_open: RwSignal::new(false),
            marks: RwSignal::new(DayMarks::new()),
            collapsed_sections: RwSignal::new(HashSet::new()),
            picker_open: RwSignal::new(false),
            picker_projects: RwSignal::new(Vec::new()),
            list: NodeRef::new(),
            request: StoredValue::new(0),
        }
    }

    pub(super) fn viewing_today(self) -> bool {
        self.view_day.with(Option::is_none)
    }

    pub(super) fn shown_day(self) -> Day {
        self.view_day.get().unwrap_or_else(|| self.today.get())
    }

    fn fail(self, error: CommandError) {
        self.error.set(Some(error));
    }

    pub(super) fn is_section_collapsed(self, project: &str) -> bool {
        self.collapsed_sections.with(|set| set.contains(project))
    }

    pub(super) fn set_section_collapsed(self, project: &str, collapse: bool) {
        if collapse {
            let current = self.selected.get_untracked();
            if current.is_some() {
                let next = self.glance.with_untracked(|glance| {
                    let g = glance.as_ref()?;
                    let mut combined = sections::group(&g.carried);
                    combined.extend(sections::group(&g.today));
                    self.collapsed_sections.with_untracked(|collapsed| {
                        sections::selection_after_collapse(
                            &combined,
                            collapsed,
                            project,
                            current.as_deref(),
                        )
                    })
                });
                if next != current {
                    self.selected.set(next);
                }
            }
            self.collapsed_sections.update(|set| {
                set.insert(project.to_owned());
            });
        } else {
            self.collapsed_sections.update(|set| {
                set.remove(project);
            });
        }
    }

    pub(super) fn toggle_section(self, project: &str) {
        let collapsed = self
            .collapsed_sections
            .with_untracked(|set| set.contains(project));
        self.set_section_collapsed(project, !collapsed);
    }

    pub(super) fn project_color(self, name: &str) -> Option<String> {
        self.picker_projects
            .with(|list| list.iter().find(|p| p.name == name)?.color.clone())
    }

    fn open_picker(self) {
        self.picker_open.set(true);
        spawn_local(async move {
            match api::projects_list(true).await {
                Ok(projects) => self.picker_projects.set(projects),
                Err(error) => self.fail(error),
            }
        });
    }

    fn set_note_project(self, note_id: String, project: Option<String>) {
        self.picker_open.set(false);
        self.focus_list();
        spawn_local(async move {
            if let Err(error) = api::note_set_project(&note_id, project.as_deref()).await {
                self.fail(error);
            }
        });
    }

    /// The ids of every visible row, carried notes first, skipping collapsed
    /// project sections.
    fn with_ids<T>(self, read: impl FnOnce(&[&str]) -> T) -> T {
        self.glance.with_untracked(|glance| {
            let ids = match glance {
                Some(g) => self
                    .collapsed_sections
                    .with_untracked(|collapsed| visible_ids(&g.carried, &g.today, collapsed)),
                None => Vec::new(),
            };
            let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
            read(&refs)
        })
    }

    fn selected_note(self) -> Option<Note> {
        let id = self.selected.get_untracked()?;
        self.glance.with_untracked(|glance| {
            let glance = glance.as_ref()?;
            glance
                .carried
                .iter()
                .chain(&glance.today)
                .find(|note| note.id == id)
                .cloned()
        })
    }

    fn focus_list(self) {
        if let Some(list) = self.list.get_untracked() {
            let _ = list.focus();
        }
    }

    /// Fetches the viewed day again. The bounds are recomputed each time, so
    /// following today is never stale across midnight. `reset` moves the
    /// selection back to the first row.
    async fn load(self, reset: bool) {
        let request = self.request.get_value().wrapping_add(1);
        self.request.set_value(request);
        let today = time::today();
        let day = self.view_day.get_untracked().unwrap_or(today);
        let (from, to) = time::bounds(day);
        let answer = api::glance_get(from, to).await;
        if self.request.get_value() != request {
            return;
        }
        if let Ok(projects) = api::projects_list(true).await {
            if self.request.get_value() == request {
                self.picker_projects.set(projects);
            }
        }
        match answer {
            Ok(glance) => {
                self.today.set(today);
                self.glance.set(Some(glance));
                self.error.set(None);
                let current = self.selected.get_untracked();
                let next =
                    self.with_ids(|ids| selection_after_load(ids, current.as_deref(), reset));
                self.selected.set(next);
            }
            Err(error) => self.fail(error),
        }
    }

    /// Shows a day, or today for `None`. Days after today are refused.
    fn show_day(self, day: Option<Day>) {
        let next = clamp_to_past(day, time::today());
        if next == self.view_day.get_untracked() {
            return;
        }
        self.view_day.set(next);
        spawn_local(self.load(true));
    }

    fn step_day(self, delta: i64) {
        let shown = self.view_day.get_untracked().unwrap_or_else(time::today);
        self.show_day(Some(shown.plus_days(delta)));
    }

    fn move_selection(self, delta: isize) {
        let current = self.selected.get_untracked();
        let next = self.with_ids(|ids| moved_selection(ids, current.as_deref(), delta));
        if next.is_some() {
            self.selected.set(next);
        }
    }

    fn close_calendar(self) {
        self.calendar_open.set(false);
        self.focus_list();
    }

    fn load_marks(self, edges: Vec<i64>) {
        if edges.len() < 2 {
            return;
        }
        spawn_local(async move {
            let marks = match api::notes_day_marks(&edges).await {
                Ok(counts) => edges
                    .iter()
                    .zip(counts)
                    .filter(|(_, mark)| mark.total > 0)
                    .map(|(start, mark)| (time::day_of(*start), mark))
                    .collect(),
                // The calendar still works without its dots.
                Err(_) => DayMarks::new(),
            };
            self.marks.set(marks);
        });
    }

    /// Marks the note done, or not done. The list refreshes when the shell
    /// announces the change.
    pub(super) fn toggle(self, note: &Note) {
        let id = note.id.clone();
        let status = if note.status == Status::Completed {
            Status::Inbox
        } else {
            Status::Completed
        };
        spawn_local(async move {
            if let Err(error) = api::note_set_status(&id, status).await {
                self.fail(error);
            }
        });
    }

    /// Parks the note in Later and keeps the selection on a neighbour.
    fn send_to_later(self, note: &Note) {
        let id = note.id.clone();
        let next = self.with_ids(|ids| neighbour(ids, &id));
        spawn_local(async move {
            match api::note_set_later(&id, true).await {
                Ok(()) => {
                    self.selected.set(next);
                    self.load(false).await;
                }
                Err(error) => self.fail(error),
            }
        });
    }

    /// Opens the inbox, filtered to the day the note was captured.
    pub(super) fn show_captured_day(self, note: &Note) {
        let day = time::day_of(note.created_at).key();
        spawn_local(async move {
            if let Err(error) = api::inbox_open_date(&day).await {
                self.fail(error);
            }
        });
    }

    pub(super) fn open_details(self, note: &Note) {
        let id = note.id.clone();
        self.selected.set(Some(id.clone()));
        spawn_local(async move {
            if let Err(error) = api::details_open(&id, posthaste_model::DetailsFrom::Today).await {
                self.fail(error);
            }
        });
    }

    fn on_key(self, event: KeyboardEvent) {
        if self.picker_open.get_untracked() {
            return;
        }
        let key = event.key();
        let on_button = event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|element| element.closest("button").ok().flatten().is_some());
        let selected = self.selected_note();
        let press = KeyPress {
            key: &key,
            modified: event.meta_key() || event.ctrl_key() || event.alt_key(),
            composing: event.is_composing(),
        };
        let context = KeyContext {
            calendar_open: self.calendar_open.get_untracked(),
            viewing_today: self.view_day.with_untracked(Option::is_none),
            has_selection: selected.is_some(),
            on_button,
        };
        let Some(action) = key_action(press, context) else {
            return;
        };
        event.prevent_default();
        match (action, selected) {
            (KeyAction::CloseCalendar, _) => self.close_calendar(),
            (KeyAction::OpenCalendar, _) => self.calendar_open.set(true),
            (KeyAction::StepDay(delta), _) => self.step_day(delta),
            (KeyAction::ShowToday, _) => self.show_day(None),
            (KeyAction::Dismiss, _) => spawn_local(async {
                // Nothing to show if hiding fails; the window is still here.
                let _ = api::today_dismiss().await;
            }),
            (KeyAction::Move(delta), _) => self.move_selection(delta),
            (KeyAction::Toggle, Some(note)) => self.toggle(&note),
            (KeyAction::SendToLater, Some(note)) => self.send_to_later(&note),
            (KeyAction::SetProject, Some(_)) => self.open_picker(),
            (KeyAction::OpenDetails, Some(note)) => self.open_details(&note),
            // `key_action` only asks for these with a note selected.
            (
                KeyAction::Toggle
                | KeyAction::SendToLater
                | KeyAction::SetProject
                | KeyAction::OpenDetails,
                None,
            ) => {}
        }
    }
}

/// What the middle of the window shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Body {
    /// The first answer has not arrived.
    Loading,
    Failed,
    Empty,
    Notes,
}

#[component]
pub fn TodayWindow(os: Os) -> impl IntoView {
    let settings = use_settings();
    use_appearance(settings);
    let i18n = use_i18n(settings);
    let today = Today::new();
    let locale = Memo::new(move |_| time::locale_for(i18n.translator().language().code));

    spawn_local(today.load(false));
    Effect::new(move |_| {
        // Once the list exists, keys should work without a click.
        if today.list.get().is_some() {
            today.focus_list();
        }
    });
    // Shown again by the shortcut: today's bounds may have moved, and the
    // user's eyes start at the top.
    events::on::<events::TodayShown>(move |()| {
        today.view_day.set(None);
        today.calendar_open.set(false);
        today.picker_open.set(false);
        spawn_local(today.load(true));
        today.focus_list();
    });
    // Back from Note details: same day, same selection, eyes on the list.
    events::on::<events::TodayReturned>(move |()| {
        spawn_local(today.load(false));
        today.focus_list();
    });
    // Notes changed elsewhere (capture, inbox): refresh, keep the selection.
    events::on::<events::NotesChanged>(move |()| spawn_local(today.load(false)));
    // The OS can give the window focus after the page asked for it: put
    // focus back on the list so keys work.
    let refocus = window_event_listener(leptos::ev::focus, move |_| {
        let on_page = document()
            .active_element()
            .is_none_or(|active| active.tag_name().eq_ignore_ascii_case("body"));
        if on_page {
            today.focus_list();
        }
    });
    // On the window, not the panel: focus can fall to the page when the
    // focused button or list goes away, and Esc must still close.
    let keys = window_event_listener(leptos::ev::keydown, move |event| today.on_key(event));
    on_cleanup(move || {
        refocus.remove();
        keys.remove();
    });

    let date_label = move || {
        let shown = today.shown_day();
        // A past day shares the row with "Back to today", so it is shorter.
        let width = if today.viewing_today() {
            Width::Long
        } else {
            Width::Short
        };
        let style = DateStyle {
            weekday: Some(width),
            month: Some(width),
            day: true,
            year: shown.year != today.today.get().year,
        };
        time::format_day(shown, &locale.get(), style)
    };
    let capture_keys = Memo::new(move |_| {
        settings.shortcuts.with(|report| match report {
            Some(report) if report.capture.registered && !report.capture.accelerator.is_empty() => {
                format_accelerator(&report.capture.accelerator, os)
            }
            _ => Vec::new(),
        })
    });
    let body = Memo::new(move |_| {
        if today.error.with(Option::is_some) {
            Body::Failed
        } else {
            today.glance.with(|glance| match glance {
                None => Body::Loading,
                Some(glance) if glance.total == 0 => Body::Empty,
                Some(_) => Body::Notes,
            })
        }
    });
    let label = move |key| Signal::derive(move || i18n.translator().text(key).to_owned());

    view! {
        <div class="today">
            <section class="today__panel" aria-label=i18n.text(key::TODAY_TITLE)>
                <header class="today__header" data-tauri-drag-region>
                    <div class="today__day">
                        <IconButton
                            icon=IconName::Prev
                            label=label(key::TODAY_PREVIOUS_DAY)
                            on_click=move |()| today.step_day(-1)
                        />
                        <h1 class="today__date">
                            <button
                                type="button"
                                class="today__date-button"
                                title=i18n.text(key::TODAY_CHOOSE_DAY_TIP)
                                aria-haspopup="dialog"
                                aria-expanded=move || today.calendar_open.get().to_string()
                                // The calendar closes on a press outside it;
                                // this press is about to close it itself.
                                on:mousedown=move |event| {
                                    if today.calendar_open.get_untracked() {
                                        event.stop_propagation();
                                    }
                                }
                                on:click=move |_| {
                                    if today.calendar_open.get_untracked() {
                                        today.close_calendar();
                                    } else {
                                        today.calendar_open.set(true);
                                    }
                                }
                            >
                                {date_label}
                            </button>
                        </h1>
                        <IconButton
                            icon=IconName::Next
                            label=label(key::TODAY_NEXT_DAY)
                            disabled=Signal::derive(move || today.viewing_today())
                            on_click=move |()| today.step_day(1)
                        />
                        <Show when=move || !today.viewing_today()>
                            <button
                                type="button"
                                class="today__back"
                                on:click=move |_| today.show_day(None)
                            >
                                {i18n.text(key::TODAY_BACK_TO_TODAY)}
                            </button>
                        </Show>
                    </div>
                    <Show when=move || today.calendar_open.get()>
                        <div class="today__calendar">
                            <Calendar
                                i18n=i18n
                                value=Some(today.shown_day())
                                marks=today.marks
                                max=time::today()
                                on_month_change=move |edges| today.load_marks(edges)
                                on_pick=move |day| {
                                    today.close_calendar();
                                    today.show_day(Some(day));
                                }
                                on_close=move |()| today.close_calendar()
                            />
                        </div>
                    </Show>
                    <Progress today=today i18n=i18n />
                </header>

                {move || match body.get() {
                    Body::Loading => ().into_any(),
                    Body::Failed => view! {
                        <p class="today__error" role="alert">
                            {move || today.error.with(|e| e.as_ref().map(|e| ipc::message(e, i18n.translator())))}
                        </p>
                    }
                    .into_any(),
                    Body::Empty => view! { <Nothing today=today i18n=i18n capture_keys=capture_keys /> }.into_any(),
                    Body::Notes => view! { <Notes today=today i18n=i18n locale=locale /> }.into_any(),
                }}

                <Show when=move || today.picker_open.get()>
                    <div class="today__picker">
                        <ProjectPicker
                            i18n=i18n
                            mode=PickerMode::SetNote
                            projects=Signal::derive(move || today.picker_projects.get())
                            on_pick=move |row| {
                                if let Some(note) = today.selected_note() {
                                    let project = match row {
                                        PickerRow::NoProject => None,
                                        PickerRow::Project { name, .. }
                                        | PickerRow::Create(name)
                                        | PickerRow::Unarchive(name) => Some(name),
                                    };
                                    today.set_note_project(note.id, project);
                                } else {
                                    today.picker_open.set(false);
                                    today.focus_list();
                                }
                            }
                            on_close=move |()| {
                                today.picker_open.set(false);
                                today.focus_list();
                            }
                        />
                    </div>
                </Show>

                <Hints today=today i18n=i18n />
            </section>
        </div>
    }
}
