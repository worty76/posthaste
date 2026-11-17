use std::collections::BTreeMap;

use leptos::ev::{KeyboardEvent, MouseEvent};
use leptos::html;
use leptos::prelude::*;
use posthaste_i18n::key;
use posthaste_model::DayMark;
use wasm_bindgen::JsCast;

use super::{caps, IconButton, IconName, Kbd};
use crate::logic::dates::{month_grid, Day, MonthCell};
use crate::state::I18n;
use crate::time::{self, DateStyle, Width};

/// Notes per day. A day that is not listed has none.
pub type DayMarks = BTreeMap<Day, DayMark>;

/// What a day's dot says about its notes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// Some notes are still open: a filled dot.
    Open(i64),
    /// Every note is done: a hollow dot.
    Done,
}

impl Mark {
    fn of(marks: &DayMarks, day: Day) -> Option<Mark> {
        let mark = marks.get(&day).filter(|mark| mark.total > 0)?;
        Some(if mark.open > 0 {
            Mark::Open(mark.open)
        } else {
            Mark::Done
        })
    }

    fn attribute(self) -> &'static str {
        match self {
            Mark::Open(_) => "open",
            Mark::Done => "done",
        }
    }
}

/// A one-month day picker that shows which days have notes.
///
/// Keys stay inside it: arrows move by day and week, Page keys by month,
/// Enter picks, T picks today, Esc closes. It also closes on a click
/// outside.
#[component]
pub fn Calendar(
    i18n: I18n,
    /// The selected day, if any. The calendar opens on its month.
    value: Option<Day>,
    #[prop(into)] marks: Signal<DayMarks>,
    /// The last day that can be picked.
    #[prop(optional)]
    max: Option<Day>,
    /// Called with the visible grid's day edges (43 instants for 42 days)
    /// whenever the month changes, so the owner can load its marks.
    #[prop(into)]
    on_month_change: Callback<Vec<i64>>,
    #[prop(into)] on_pick: Callback<Day>,
    /// Offers "Clear" when given.
    #[prop(optional)]
    on_clear: Option<Callback<()>>,
    #[prop(into)] on_close: Callback<()>,
) -> impl IntoView {
    let today = time::today();
    let focused = RwSignal::new(value.unwrap_or(today));
    let root = NodeRef::<html::Dialog>::new();
    let locale = Memo::new(move |_| time::locale_for(i18n.translator().language().code));

    // Changes only when the month does, so moving within a month keeps the
    // grid's buttons (and the focus on them) in place.
    let month = Memo::new(move |_| Day {
        day: 1,
        ..focused.get()
    });
    let cells = Memo::new(move |_| month_grid(month.get(), time::week_start(&locale.get())));

    Effect::new(move |_| {
        let cells = cells.get();
        let mut edges: Vec<i64> = cells.iter().map(|cell| time::start_of(cell.day)).collect();
        if let Some(last) = cells.last() {
            edges.push(time::bounds(last.day).1);
        }
        on_month_change.run(edges);
    });

    // Focus follows the focused day, after the grid has been drawn.
    Effect::new(move |_| {
        let selector = format!("[data-day=\"{}\"]", focused.get());
        cells.track();
        request_animation_frame(move || {
            let button = root
                .get_untracked()
                .and_then(|root| root.query_selector(&selector).ok().flatten())
                .and_then(|button| button.dyn_into::<web_sys::HtmlElement>().ok());
            if let Some(button) = button {
                let _ = button.focus();
            }
        });
    });

    let outside = window_event_listener(leptos::ev::mousedown, move |event: MouseEvent| {
        let target = event
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Node>().ok());
        let inside = root
            .get_untracked()
            .is_some_and(|root| root.contains(target.as_ref()));
        if !inside {
            on_close.run(());
        }
    });
    on_cleanup(move || outside.remove());

    let allowed = move |day: Day| max.is_none_or(|max| day <= max);
    let go = move |day: Day| {
        if allowed(day) {
            focused.set(day);
        } else if let Some(max) = max.filter(|max| day.same_month(*max) || day < *max) {
            // Asked for a day past the limit in the limit's month: stop there.
            focused.set(max);
        }
    };

    let on_grid_key = move |event: KeyboardEvent| {
        if event.is_composing() || event.meta_key() || event.ctrl_key() || event.alt_key() {
            return;
        }
        let day = focused.get_untracked();
        let key = event.key();
        let moved = match key.as_str() {
            "ArrowLeft" => Some(day.plus_days(-1)),
            "ArrowRight" => Some(day.plus_days(1)),
            "ArrowUp" => Some(day.plus_days(-7)),
            "ArrowDown" => Some(day.plus_days(7)),
            "PageUp" => Some(day.plus_months(-1)),
            "PageDown" => Some(day.plus_months(1)),
            _ => None,
        };
        if let Some(target) = moved {
            go(target);
        } else if key == "Enter" {
            if allowed(day) {
                on_pick.run(day);
            }
        } else if key.eq_ignore_ascii_case("t") {
            on_pick.run(today);
        } else {
            return;
        }
        event.prevent_default();
        event.stop_propagation();
    };

    // Keys stay inside: the window behind has its own single-key shortcuts.
    let on_root_key = move |event: KeyboardEvent| {
        event.stop_propagation();
        if event.key() == "Escape" && !event.is_composing() {
            event.prevent_default();
            on_close.run(());
        }
    };

    let month_label = move || {
        let style = DateStyle {
            month: Some(Width::Long),
            year: true,
            ..DateStyle::default()
        };
        time::format_day(month.get(), &locale.get(), style)
    };
    let weekday = move |day: Day, width: Width| {
        let style = DateStyle {
            weekday: Some(width),
            ..DateStyle::default()
        };
        time::format_day(day, &locale.get(), style)
    };
    let day_label = move |day: Day| {
        let style = DateStyle {
            weekday: Some(Width::Long),
            month: Some(Width::Long),
            day: true,
            year: true,
        };
        let date = time::format_day(day, &locale.get(), style);
        let t = i18n.translator();
        match marks.with(|marks| Mark::of(marks, day)) {
            Some(Mark::Open(open)) => t.format(
                key::COMMON_CALENDAR_DAY_OPEN,
                &[("date", &date), ("count", &open.to_string())],
            ),
            Some(Mark::Done) => t.format(key::COMMON_CALENDAR_DAY_DONE, &[("date", &date)]),
            None => date,
        }
    };
    let next_month_allowed = move || max.is_none_or(|max| month.get().plus_months(1) <= max);

    let day_button = move |cell: MonthCell| {
        let day = cell.day;
        view! {
            <button
                type="button"
                role="gridcell"
                class="calendar__day"
                data-day=day.key()
                data-outside=(!cell.in_month).then_some("true")
                data-today=(day == today).then_some("true")
                data-mark=move || marks.with(|marks| Mark::of(marks, day)).map(Mark::attribute)
                aria-selected=(Some(day) == value).to_string()
                aria-label=move || day_label(day)
                tabindex=move || if focused.get() == day { "0" } else { "-1" }
                disabled=!allowed(day)
                on:click=move |_| on_pick.run(day)
                on:focus=move |_| {
                    if day != focused.get_untracked() && day.same_month(month.get_untracked()) {
                        focused.set(day);
                    }
                }
            >
                {day.day}
            </button>
        }
    };

    view! {
        // Non-modal: it sits beside its button and closes on an outside click.
        <dialog
            node_ref=root
            open
            class="calendar"
            aria-label=i18n.text(key::COMMON_CALENDAR_CHOOSE_DAY)
            on:keydown=on_root_key
        >
            <div class="calendar__head">
                <IconButton
                    icon=IconName::Prev
                    label=Signal::derive(move || i18n.translator().text(key::COMMON_CALENDAR_PREVIOUS_MONTH).to_owned())
                    on_click=move |()| go(focused.get_untracked().plus_months(-1))
                />
                <p class="calendar__month" aria-live="polite">{month_label}</p>
                <IconButton
                    icon=IconName::Next
                    label=Signal::derive(move || i18n.translator().text(key::COMMON_CALENDAR_NEXT_MONTH).to_owned())
                    disabled=Signal::derive(move || !next_month_allowed())
                    on_click=move |()| go(focused.get_untracked().plus_months(1))
                />
            </div>
            <div class="calendar__grid" role="grid" aria-label=month_label on:keydown=on_grid_key>
                {move || {
                    let cells = cells.get();
                    let header = cells
                        .iter()
                        .take(7)
                        .map(|cell| {
                            let day = cell.day;
                            view! {
                                <span
                                    class="calendar__weekday"
                                    role="columnheader"
                                    title=move || weekday(day, Width::Long)
                                >
                                    {move || weekday(day, Width::Narrow)}
                                </span>
                            }
                        })
                        .collect_view();
                    let weeks = cells
                        .chunks(7)
                        .map(|week| {
                            let days = week.iter().copied().map(day_button).collect_view();
                            view! { <div class="calendar__row" role="row">{days}</div> }
                        })
                        .collect_view();
                    view! {
                        <div class="calendar__row" role="row">{header}</div>
                        {weeks}
                    }
                }}
            </div>
            <div class="calendar__foot">
                <button type="button" class="calendar__link" on:click=move |_| on_pick.run(today)>
                    {i18n.text(key::COMMON_CALENDAR_TODAY)}
                </button>
                {on_clear.map(|on_clear| view! {
                    <button type="button" class="calendar__link" on:click=move |_| on_clear.run(())>
                        {i18n.text(key::COMMON_CALENDAR_CLEAR)}
                    </button>
                })}
            </div>
            <p class="calendar__hints">
                <Kbd keys=caps(&["←", "→", "↑", "↓"]) />
                " "
                {i18n.text(key::COMMON_CALENDAR_HINT_DAY)}
                " "
                <Kbd keys=caps(&["PgUp", "PgDn"]) />
                " "
                {i18n.text(key::COMMON_CALENDAR_HINT_MONTH)}
                " "
                <Kbd keys=caps(&["T"]) />
                " "
                {i18n.text(key::COMMON_CALENDAR_HINT_TODAY)}
            </p>
        </dialog>
    }
}
