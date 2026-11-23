//! Header controls and timestamp metadata for Note details.

use leptos::prelude::*;
use posthaste_i18n::{key, Translator};
use posthaste_model::{Note, Os};

use super::Details;
use crate::components::{Icon, IconName, ProjectDot, ProjectPicker};
use crate::logic::details::{badge, carried_days, was_edited, Badge};
use crate::logic::project_picker::{PickerMode, PickerRow};
use crate::logic::shortcut::mod_cap;
use crate::state::I18n;
use crate::time::{self, DateStyle, Width};

/// The project as a small chip next to the status pill, or a quiet
/// "Add project" control when the note has none. Enter or a click opens the
/// shared project picker; Esc closes it.
#[component]
pub(super) fn CategoryChip(details: Details, i18n: I18n) -> impl IntoView {
    let current = Memo::new(move |_| {
        details
            .detail
            .with(|detail| detail.as_ref().and_then(|d| d.note.category.clone()))
    });
    let open = move |_| details.open_project_picker();

    view! {
        <span class="details__cat-editor">
            {move || match current.get() {
                Some(name) => {
                    let color = details.project_color(&name);
                    view! {
                        <button
                            type="button"
                            class="details__category"
                            title=i18n.text(key::DETAILS_CATEGORY_EDIT)
                            on:click=open
                        >
                            {color.map(|c| view! { <ProjectDot color=c /> })}
                            <span class="details__category-name">{name}</span>
                        </button>
                    }
                    .into_any()
                }
                None => view! {
                    <button
                        type="button"
                        class="details__category"
                        data-empty=""
                        title=i18n.text(key::DETAILS_CATEGORY_EDIT)
                        on:click=open
                    >
                        {i18n.text(key::DETAILS_CATEGORY_ADD)}
                    </button>
                }
                .into_any(),
            }}
            <Show when=move || details.picker_open.get()>
                <ProjectPicker
                    i18n=i18n
                    mode=PickerMode::SetNote
                    projects=Signal::derive(move || details.projects.get())
                    on_pick=move |row| {
                        let project = match row {
                            PickerRow::NoProject => None,
                            PickerRow::Project { name, .. }
                            | PickerRow::Create(name)
                            | PickerRow::Unarchive(name) => Some(name),
                        };
                        details.set_project(project);
                    }
                    on_close=move |()| details.close_project_picker()
                />
            </Show>
        </span>
    }
}

/// The button that says where the note is and marks it done.
#[component]
pub(super) fn StatusButton(details: Details, i18n: I18n, os: Os) -> impl IntoView {
    let state = Memo::new(move |_| details.detail.with(|d| d.as_ref().map(|d| badge(&d.note))));
    let keys = format!("{}+Enter", mod_cap(os));
    move || {
        let state = state.get()?;
        let done = state == Badge::Done;
        let name = match state {
            Badge::Done => key::DETAILS_STATUS_DONE,
            Badge::Archived => key::DETAILS_STATUS_ARCHIVED,
            Badge::Later => key::DETAILS_STATUS_LATER,
            Badge::Inbox => key::DETAILS_STATUS_INBOX,
        };
        let tip = if done {
            key::DETAILS_STATUS_MARK_NOT_COMPLETED
        } else {
            key::DETAILS_STATUS_MARK_COMPLETED
        };
        let keys = keys.clone();
        Some(view! {
            <button
                type="button"
                class="details__status"
                data-done=done.to_string()
                aria-pressed=done.to_string()
                title=move || i18n.translator().format(tip, &[("keys", &keys)])
                on:click=move |_| details.toggle_done()
            >
                {done.then(|| view! { <Icon name=IconName::Check size=12 /> })}
                {i18n.text(name)}
            </button>
        })
    }
}

/// "today", "yesterday" or a date, for the line under the title.
enum When {
    Today,
    Yesterday,
    On(String),
}

fn when(ms: i64, locale: &str) -> When {
    let (day, today) = (time::day_of(ms), time::today());
    match day.days_until(today) {
        0 => When::Today,
        1 => When::Yesterday,
        _ => When::On(time::format_day(
            day,
            locale,
            DateStyle {
                weekday: Some(Width::Long),
                month: Some(Width::Long),
                day: true,
                year: day.year != today.year,
            },
        )),
    }
}

/// When the note was captured, where from, and what has happened since.
fn meta_line(note: &Note, locale: &str, t: Translator) -> String {
    let stamp = |ms: i64, today, yesterday, on| {
        let time = time::format_time(ms, locale);
        match when(ms, locale) {
            When::Today => t.format(today, &[("time", &time)]),
            When::Yesterday => t.format(yesterday, &[("time", &time)]),
            When::On(date) => t.format(on, &[("date", &date), ("time", &time)]),
        }
    };
    let mut parts = vec![stamp(
        note.created_at,
        key::DETAILS_META_CREATED_TODAY,
        key::DETAILS_META_CREATED_YESTERDAY,
        key::DETAILS_META_CREATED_ON,
    )];
    if let Some(source) = &note.source {
        parts.push(t.format(key::DETAILS_META_FROM, &[("source", source)]));
    }
    let carried = carried_days(note, time::day_of(note.created_at), time::today());
    if carried > 0 {
        parts.push(t.plural(key::DETAILS_META_CARRIED, carried, &[]));
    }
    if was_edited(note) {
        parts.push(stamp(
            note.updated_at,
            key::DETAILS_META_EDITED_TODAY,
            key::DETAILS_META_EDITED_YESTERDAY,
            key::DETAILS_META_EDITED_ON,
        ));
    }
    parts.join(" · ")
}

#[component]
pub(super) fn Meta(details: Details, i18n: I18n, locale: Memo<String>) -> impl IntoView {
    move || {
        details.detail.with(|detail| {
            let note = &detail.as_ref()?.note;
            let line = meta_line(note, &locale.get(), i18n.translator());
            Some(view! { <p class="details__meta">{line}</p> })
        })
    }
}
