//! Today window sub-views: progress bar, empty state, grouped note rows,
//! carried-over origin tags, and keyboard hints footer.

use leptos::html;
use leptos::prelude::*;
use posthaste_i18n::{key, Translator};
use posthaste_model::Note;

use super::Today;
use crate::components::{caps, EmptyState, Icon, IconButton, IconName, Kbd, ProjectDot};
use crate::logic::dates::Day;
use crate::logic::sections;
use crate::logic::today::{is_done, origin, Origin};
use crate::state::I18n;
use crate::time::{self, DateStyle, Width};
use crate::windows::note_badges::NoteBadges;

/// "3 of 5 done" and the bar under it.
#[component]
pub(super) fn Progress(today: Today, i18n: I18n) -> impl IntoView {
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
pub(super) fn Nothing(today: Today, i18n: I18n, capture_keys: Memo<Vec<String>>) -> impl IntoView {
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
pub(super) fn Notes(today: Today, i18n: I18n, locale: Memo<String>) -> impl IntoView {
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
    let grouped = Memo::new(move |_| {
        notes.with(|notes| {
            sections::group(notes)
                .into_iter()
                .map(|section| {
                    (
                        section.project.map(str::to_owned),
                        section.notes.into_iter().cloned().collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    let keys = move || {
        grouped.with(|groups| {
            groups
                .iter()
                .map(|(project, _)| project.clone())
                .collect::<Vec<_>>()
        })
    };
    let notes_of = move |project: Option<&str>| {
        grouped.with(|groups| {
            groups
                .iter()
                .find(|(name, _)| name.as_deref() == project)
                .map(|(_, notes)| notes.clone())
                .unwrap_or_default()
        })
    };

    view! {
        <Show when=move || notes.with(|notes| !notes.is_empty())>
            <section class="today__group" aria-label=move || label.get()>
                <h2 class="today__group-label">{move || label.get()}</h2>
                <For each=keys key=|project| project.clone() let:project>
                    {match project {
                        Some(name) => {
                            let project = StoredValue::new(name);
                            let expanded = move || {
                                project.with_value(|name| !today.is_section_collapsed(name))
                            };
                            view! {
                                <div class="proj-section today__section">
                                    <button
                                        type="button"
                                        class="proj-section__header today__section-header"
                                        aria-expanded=move || expanded().to_string()
                                        on:click=move |_| {
                                            project.with_value(|name| today.toggle_section(name));
                                        }
                                        on:keydown=move |event| {
                                            if event.is_composing() {
                                                return;
                                            }
                                            match event.key().as_str() {
                                                "ArrowLeft" => {
                                                    event.prevent_default();
                                                    event.stop_propagation();
                                                    project.with_value(|name| {
                                                        today.set_section_collapsed(name, true);
                                                    });
                                                }
                                                "ArrowRight" => {
                                                    event.prevent_default();
                                                    event.stop_propagation();
                                                    project.with_value(|name| {
                                                        today.set_section_collapsed(name, false);
                                                    });
                                                }
                                                _ => {}
                                            }
                                        }
                                    >
                                        <span class="proj-section__chevron">
                                            <Icon name=IconName::Next size=12 />
                                        </span>
                                        {move || project.with_value(|name| today.project_color(name)).map(|c| view! {
                                            <ProjectDot color=c />
                                        })}
                                        <span class="proj-section__name">{project.get_value()}</span>
                                        <span class="proj-section__count">
                                            {move || project.with_value(|name| notes_of(Some(name)).len())}
                                        </span>
                                    </button>
                                    <Show when=expanded>
                                        <ul
                                            class=move || {
                                                if project.with_value(|name| today.project_color(name)).is_some() {
                                                    "today__rows today__rows--project today__rows--colored"
                                                } else {
                                                    "today__rows today__rows--project"
                                                }
                                            }
                                            data-color=move || project.with_value(|name| today.project_color(name))
                                        >
                                            {move || project.with_value(|name| today.project_color(name)).map(|c| view! {
                                                <svg
                                                    class="today__rail"
                                                    viewBox="0 0 2 10"
                                                    preserveAspectRatio="none"
                                                    aria-hidden="true"
                                                    focusable="false"
                                                >
                                                    <rect width="2" height="10" rx="1" fill=c />
                                                </svg>
                                            })}
                                            <For
                                                each=move || project.with_value(|name| notes_of(Some(name)))
                                                key=row_key
                                                let:note
                                            >
                                                <Row today=today i18n=i18n locale=locale note=note />
                                            </For>
                                        </ul>
                                    </Show>
                                </div>
                            }
                            .into_any()
                        }
                        None => view! {
                            <div class="proj-section proj-section--plain today__section">
                                <ul class="today__rows">
                                    <For each=move || notes_of(None) key=row_key let:note>
                                        <Row today=today i18n=i18n locale=locale note=note />
                                    </For>
                                </ul>
                            </div>
                        }
                        .into_any(),
                    }}
                </For>
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
            <div class="today__main">
                <p class="today__text selectable">{note.with_value(|note| note.content.clone())}</p>
                <div class="today__meta">
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
                </div>
            </div>
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
pub(super) fn Hints(today: Today, i18n: I18n) -> impl IntoView {
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
                        {hint(&["P"], key::TODAY_HINT_PROJECT)}
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
