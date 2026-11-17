use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;

use crate::components::{Calendar, DayMarks, Icon, IconName};
use crate::ipc::{api, events};
use crate::logic::dates::Day;
use crate::state::I18n;
use crate::time::{self, DateStyle, Width};

/// The search's "Captured on" control: a button that opens a calendar
/// showing which days have notes.
#[component]
pub(super) fn DateFilter(
    i18n: I18n,
    locale: Memo<String>,
    #[prop(into)] value: Signal<Option<Day>>,
    #[prop(into)] on_change: Callback<Option<Day>>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let button = NodeRef::<html::Button>::new();
    let close = move || {
        open.set(false);
        if let Some(button) = button.get_untracked() {
            let _ = button.focus();
        }
    };
    let pick = move |day: Option<Day>| {
        on_change.run(day);
        close();
    };
    let date = move || {
        value.get().map(|day| {
            let style = DateStyle {
                weekday: Some(Width::Short),
                month: Some(Width::Short),
                day: true,
                year: true,
            };
            time::format_day(day, &locale.get(), style)
        })
    };
    let label = move || {
        date().unwrap_or_else(|| {
            i18n.translator()
                .text(key::INBOX_FILTER_CAPTURED_ON)
                .to_owned()
        })
    };
    let name = move || {
        let t = i18n.translator();
        match date() {
            Some(date) => t.format(key::INBOX_FILTER_CAPTURED_ON_DAY, &[("date", &date)]),
            None => t.text(key::INBOX_FILTER_CAPTURED_ON).to_owned(),
        }
    };

    view! {
        <div class="datefilter">
            <button
                node_ref=button
                type="button"
                class="datefilter__button field"
                aria-label=name
                aria-expanded=move || open.get().to_string()
                aria-haspopup="dialog"
                title=i18n.text(key::INBOX_FILTER_CAPTURED_TIP)
                data-empty=move || value.get().is_none().then_some("true")
                // While open, a press here toggles; it is not an "outside" click.
                on:mousedown=move |event| {
                    if open.get_untracked() {
                        event.stop_propagation();
                    }
                }
                on:click=move |_| open.update(|open| *open = !*open)
            >
                <Icon name=IconName::Calendar />
                {label}
            </button>
            <Show when=move || open.get()>
                <Popover
                    i18n=i18n
                    value=value.get_untracked()
                    on_pick=pick
                    on_close=move |()| close()
                />
            </Show>
        </div>
    }
}

/// The open calendar. Its own component so the dots stay current only
/// while it is on screen.
#[component]
fn Popover(
    i18n: I18n,
    value: Option<Day>,
    #[prop(into)] on_pick: Callback<Option<Day>>,
    #[prop(into)] on_close: Callback<()>,
) -> impl IntoView {
    let marks = RwSignal::new(DayMarks::new());
    // The grid on screen, so a change to notes can reload its dots.
    let edges = StoredValue::new(Vec::<i64>::new());
    let load = move || {
        let edges = edges.get_value();
        if edges.len() < 2 {
            return;
        }
        spawn_local(async move {
            let found = match api::notes_day_marks(&edges).await {
                Ok(counts) => edges
                    .iter()
                    .zip(counts)
                    .filter(|(_, mark)| mark.total > 0)
                    .map(|(start, mark)| (time::day_of(*start), mark))
                    .collect(),
                // The calendar still works without its dots.
                Err(_) => DayMarks::new(),
            };
            marks.set(found);
        });
    };
    events::on::<events::NotesChanged>(move |()| load());

    let calendar = move |clear: Option<Callback<()>>| {
        let pick = move |day| on_pick.run(Some(day));
        let month = move |grid| {
            edges.set_value(grid);
            load();
        };
        match clear {
            Some(clear) => view! {
                <Calendar i18n=i18n value=value marks=marks on_month_change=month
                    on_pick=pick on_clear=clear on_close=on_close />
            }
            .into_any(),
            None => view! {
                <Calendar i18n=i18n value=value marks=marks on_month_change=month
                    on_pick=pick on_close=on_close />
            }
            .into_any(),
        }
    };
    // "Clear" is offered only when a day is picked.
    let clear = value.map(|_| Callback::new(move |()| on_pick.run(None)));
    view! { <div class="datefilter__pop">{calendar(clear)}</div> }
}
