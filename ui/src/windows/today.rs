//! The Today window: what is on today's plate and how much of it is done.
//!
//! ← and → show earlier days, read-only and rebuilt from timestamps. There
//! are no future days, because notes have no dates.

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Translator};
use posthaste_model::{CommandError, Glance, Note, Os, Status};
use wasm_bindgen::JsCast;

use super::note_badges::NoteBadges;
use crate::components::{caps, Calendar, DayMarks, EmptyState, Icon, IconButton, IconName, Kbd};
use crate::ipc::{self, api, events};
use crate::logic::dates::Day;
use crate::logic::shortcut::format_accelerator;
use crate::logic::today::{
    clamp_to_past, is_done, key_action, moved_selection, neighbour, origin, selection_after_load,
    KeyAction, KeyContext, KeyPress, Origin,
};
use crate::state::{use_appearance, use_i18n, use_settings, I18n};
use crate::time::{self, DateStyle, Width};

/// Everything the window's actions read and write. `Copy`, so each handler
/// takes it by value.
#[derive(Clone, Copy)]
struct Today {
    glance: RwSignal<Option<Glance>>,
    error: RwSignal<Option<CommandError>>,
    /// The id of the selected row.
    selected: RwSignal<Option<String>>,
    /// The day being looked at. `None` follows today, across midnight too.
    view_day: RwSignal<Option<Day>>,
    /// Today's date as of the last load, so a window left open overnight
    /// redraws for the new day instead of trusting an old clock reading.
    today: RwSignal<Day>,
    calendar_open: RwSignal<bool>,
    marks: RwSignal<DayMarks>,
    list: NodeRef<html::Div>,
    /// Counts loads, so a slow answer to an old one cannot replace a newer.
    request: StoredValue<u32>,
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
            list: NodeRef::new(),
            request: StoredValue::new(0),
        }
    }

    fn viewing_today(self) -> bool {
        self.view_day.with(Option::is_none)
    }

    fn shown_day(self) -> Day {
        self.view_day.get().unwrap_or_else(|| self.today.get())
    }

    fn fail(self, error: CommandError) {
        self.error.set(Some(error));
    }

    /// The ids of every row, carried notes first, as listed on screen.
    fn with_ids<T>(self, read: impl FnOnce(&[&str]) -> T) -> T {
        self.glance.with_untracked(|glance| {
            let ids: Vec<&str> = glance
                .iter()
                .flat_map(|g| g.carried.iter().chain(&g.today))
                .map(|note| note.id.as_str())
                .collect();
            read(&ids)
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
    fn toggle(self, note: &Note) {
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
    fn show_captured_day(self, note: &Note) {
        let day = time::day_of(note.created_at).key();
        spawn_local(async move {
            if let Err(error) = api::inbox_open_date(&day).await {
                self.fail(error);
            }
        });
    }

    fn open_details(self, note: &Note) {
        let id = note.id.clone();
        self.selected.set(Some(id.clone()));
        spawn_local(async move {
            if let Err(error) = api::details_open(&id, api::DetailsFrom::Today).await {
                self.fail(error);
            }
        });
    }

    fn on_key(self, event: KeyboardEvent) {
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
            (KeyAction::OpenDetails, Some(note)) => self.open_details(&note),
            // `key_action` only asks for these with a note selected.
            (KeyAction::Toggle | KeyAction::SendToLater | KeyAction::OpenDetails, None) => {}
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

                <Hints today=today i18n=i18n />
            </section>
        </div>
    }
}

/// "3 of 5 done" and the bar under it.
#[component]
fn Progress(today: Today, i18n: I18n) -> impl IntoView {
    let counts = Memo::new(move |_| today.glance.with(|g| g.as_ref().map(|g| (g.done, g.total))));
    move || {
        counts.get().map(|(done, total)| {
            let all_done = total > 0 && done == total;
            let text = move || {
                let t = i18n.translator();
                if all_done {
                    t.text(key::TODAY_ALL_DONE).to_owned()
                } else {
                    let (done, total) = (done.to_string(), total.to_string());
                    t.format(key::TODAY_COUNT, &[("done", &done), ("total", &total)])
                }
            };
            view! {
                <p class="today__count" data-all-done=all_done.then_some("true")>{text}</p>
                <progress
                    class="today__bar"
                    value=done
                    max=total
                    aria-label=i18n.text(key::TODAY_PROGRESS_LABEL)
                ></progress>
            }
        })
    }
}

/// The empty day: says so, and on today says how to capture a note.
#[component]
fn Nothing(today: Today, i18n: I18n, capture_keys: Memo<Vec<String>>) -> impl IntoView {
    let title = Signal::derive(move || {
        let key = if today.viewing_today() {
            key::TODAY_EMPTY_TODAY_TITLE
        } else {
            key::TODAY_EMPTY_PAST_TITLE
        };
        i18n.translator().text(key).to_owned()
    });
    view! {
        <EmptyState title=title>
            {move || {
                let t = i18n.translator();
                if !today.viewing_today() {
                    t.text(key::TODAY_EMPTY_PAST_TEXT).into_any()
                } else if capture_keys.with(Vec::is_empty) {
                    t.text(key::TODAY_EMPTY_CAPTURE_PLAIN).into_any()
                } else {
                    let (before, after) = t.around(key::TODAY_EMPTY_CAPTURE_WITH_SHORTCUT, "keys");
                    view! { {before} <Kbd keys=capture_keys /> {after} }.into_any()
                }
            }}
        </EmptyState>
    }
}

/// The two groups of rows, and the line that says more exist.
#[component]
fn Notes(today: Today, i18n: I18n, locale: Memo<String>) -> impl IntoView {
    let carried = move || {
        today
            .glance
            .with(|g| g.as_ref().map(|g| g.carried.clone()).unwrap_or_default())
    };
    let captured = move || {
        today
            .glance
            .with(|g| g.as_ref().map(|g| g.today.clone()).unwrap_or_default())
    };
    let more = Memo::new(move |_| {
        today.glance.with(|g| {
            g.as_ref()
                .map_or(0, |g| g.total - (g.carried.len() + g.today.len()) as i64)
        })
    });
    let has_more = move || more.get() > 0;
    let own_group = move || {
        if today.viewing_today() {
            key::TODAY_GROUP_TODAY
        } else {
            key::TODAY_GROUP_THAT_DAY
        }
    };

    view! {
        <div node_ref=today.list class="today__list" tabindex="-1">
            <Group today=today i18n=i18n locale=locale notes=Signal::derive(carried)
                label=Signal::derive(move || i18n.translator().text(key::TODAY_GROUP_CARRIED).to_owned()) />
            <Group today=today i18n=i18n locale=locale notes=Signal::derive(captured)
                label=Signal::derive(move || i18n.translator().text(own_group()).to_owned()) />
        </div>
        <Show when=has_more>
            <p class="today__more">
                {move || {
                    let key = if today.viewing_today() { key::TODAY_MORE_IN_INBOX } else { key::TODAY_MORE };
                    i18n.translator().format(key, &[("count", &more.get().to_string())])
                }}
            </p>
        </Show>
    }
}

/// Everything about a note that changes how its row is drawn. A row is
/// redrawn only when this changes, so focus and scroll survive a refresh.
fn row_key(note: &Note) -> (String, i64, Option<i64>, bool, u32) {
    (
        note.id.clone(),
        note.updated_at,
        note.completed_at,
        note.has_details,
        note.attachment_count,
    )
}

#[component]
fn Group(
    today: Today,
    i18n: I18n,
    locale: Memo<String>,
    notes: Signal<Vec<Note>>,
    label: Signal<String>,
) -> impl IntoView {
    view! {
        <Show when=move || notes.with(|notes| !notes.is_empty())>
            <section class="today__group" aria-label=move || label.get()>
                <h2 class="today__group-label">{move || label.get()}</h2>
                <ul class="today__rows">
                    <For each=move || notes.get() key=row_key let:note>
                        <Row today=today i18n=i18n locale=locale note=note />
                    </For>
                </ul>
            </section>
        </Show>
    }
}

/// The words for where a carried note came from: what is shown beside it,
/// and the day alone for the button's accessible name.
fn origin_words(origin: Origin, today: Day, locale: &str, t: Translator) -> (String, String) {
    match origin {
        Origin::Yesterday => {
            let text = t.text(key::TODAY_ORIGIN_YESTERDAY).to_owned();
            (text.clone(), text)
        }
        Origin::Earlier { day, days_ago } => {
            let style = DateStyle {
                weekday: Some(Width::Short),
                month: Some(Width::Short),
                day: true,
                year: day.year != today.year,
            };
            let date = time::format_day(day, locale, style).replace(',', "");
            let shown = match days_ago {
                Some(count) => t.format(
                    key::TODAY_ORIGIN_DAYS_AGO,
                    &[("date", &date), ("count", &count.to_string())],
                ),
                None => date.clone(),
            };
            (shown, date)
        }
    }
}

#[component]
fn Row(today: Today, i18n: I18n, locale: Memo<String>, note: Note) -> impl IntoView {
    let item = NodeRef::<html::Li>::new();
    let note = StoredValue::new(note);
    let id = note.with_value(|note| note.id.clone());
    let captured = note.with_value(|note| time::day_of(note.created_at));
    let selected = Memo::new(move |_| today.selected.with(|s| s.as_deref() == Some(id.as_str())));
    let read_only = move || !today.viewing_today();
    // Done as of the viewed day, which on a past day may differ from status.
    let done = Memo::new(move |_| {
        let viewing_today = today.viewing_today();
        let day_end = time::bounds(today.shown_day()).1;
        note.with_value(|note| is_done(note, viewing_today, day_end))
    });

    // Moving with the keyboard must never leave the selection off screen.
    Effect::new(move |_| {
        if let (true, Some(item)) = (selected.get(), item.get()) {
            let options = web_sys::ScrollIntoViewOptions::new();
            options.set_block(web_sys::ScrollLogicalPosition::Nearest);
            item.scroll_into_view_with_scroll_into_view_options(&options);
        }
    });

    let check_label = move || {
        let key = match (read_only(), done.get()) {
            (true, true) => key::TODAY_ROW_DONE_BY_END,
            (true, false) => key::TODAY_ROW_NOT_DONE_BY_END,
            (false, true) => key::TODAY_ROW_MARK_NOT_COMPLETED,
            (false, false) => key::TODAY_ROW_MARK_COMPLETED,
        };
        i18n.translator().text(key)
    };
    let came_from = move || {
        let today_is = today.today.get();
        origin(captured, today.shown_day(), today_is)
            .map(|origin| origin_words(origin, today_is, &locale.get(), i18n.translator()))
    };

    view! {
        <li
            node_ref=item
            class="today__row"
            data-selected=move || selected.get().to_string()
            data-status=move || if done.get() { "completed" } else { "inbox" }
            aria-current=move || selected.get().then_some("true")
        >
            <button
                type="button"
                role="checkbox"
                class="check"
                aria-checked=move || done.get().to_string()
                aria-label=check_label
                disabled=read_only
                on:click=move |_| note.with_value(|note| today.toggle(note))
            >
                <Icon name=IconName::Check size=12 />
            </button>
            <p class="today__text selectable">{note.with_value(|note| note.content.clone())}</p>
            {move || came_from().map(|(shown, day)| view! {
                <button
                    type="button"
                    class="today__origin"
                    aria-label=i18n.translator().format(key::TODAY_ROW_SHOW_CAPTURED, &[("day", &day)])
                    title=i18n.text(key::TODAY_ROW_SHOW_CAPTURED_TIP)
                    on:click=move |_| note.with_value(|note| today.show_captured_day(note))
                >
                    {shown}
                </button>
            })}
            <NoteBadges note=note.get_value() i18n=i18n />
            <IconButton
                icon=IconName::Open
                class="today__open"
                label=Signal::derive(move || i18n.translator().text(key::TODAY_ROW_OPEN_DETAILS).to_owned())
                on_click=move |()| note.with_value(|note| today.open_details(note))
            />
        </li>
    }
}

/// The keys that do things, along the bottom.
#[component]
fn Hints(today: Today, i18n: I18n) -> impl IntoView {
    let hint = move |keys: &[&str], label| {
        view! {
            <span class="today__hint">
                <Kbd keys=caps(keys) />
                " "
                {i18n.text(label)}
            </span>
        }
    };
    view! {
        <footer class="today__hints" aria-hidden="true">
            <Show when=move || !today.viewing_today()>
                <span class="today__hint today__past">{i18n.text(key::TODAY_HINT_PAST)}</span>
            </Show>
            <span class="today__hint">
                <Kbd keys=caps(&["↑", "↓"]) />
            </span>
            {hint(&["←", "→"], key::TODAY_HINT_DAY)}
            {move || {
                if today.viewing_today() {
                    view! {
                        {hint(&["X"], key::TODAY_HINT_DONE)}
                        {hint(&["L"], key::TODAY_HINT_LATER)}
                    }
                    .into_any()
                } else {
                    hint(&["T"], key::TODAY_HINT_TODAY).into_any()
                }
            }}
            {hint(&["↵"], key::TODAY_HINT_DETAILS)}
            {hint(&["esc"], key::TODAY_HINT_CLOSE)}
        </footer>
    }
}
