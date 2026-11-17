use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use posthaste_i18n::{key, Key};
use posthaste_model::CapturePosition;

use super::controls::focus_choice;
use crate::logic::position::{step, SPOTS};
use crate::state::I18n;

/// What a screen reader calls a spot.
fn spot_name(spot: CapturePosition) -> Key {
    match spot {
        CapturePosition::TopLeft => key::COMMON_POSITION_TOP_LEFT,
        CapturePosition::Top => key::COMMON_POSITION_TOP,
        CapturePosition::TopRight => key::COMMON_POSITION_TOP_RIGHT,
        CapturePosition::Left => key::COMMON_POSITION_LEFT,
        CapturePosition::Center => key::COMMON_POSITION_CENTER,
        CapturePosition::Right => key::COMMON_POSITION_RIGHT,
        CapturePosition::BottomLeft => key::COMMON_POSITION_BOTTOM_LEFT,
        CapturePosition::Bottom => key::COMMON_POSITION_BOTTOM,
        CapturePosition::BottomRight => key::COMMON_POSITION_BOTTOM_RIGHT,
    }
}

/// A small picture of a display with nine spots: where a floating window
/// opens. A radio group; the arrow keys move through the grid.
#[component]
pub fn PositionPicker(
    i18n: I18n,
    label: Key,
    #[prop(into)] value: Signal<CapturePosition>,
    #[prop(into)] on_change: Callback<CapturePosition>,
) -> impl IntoView {
    let on_key = move |event: KeyboardEvent| {
        let Some(next) = step(value.get_untracked(), &event.key()) else {
            return;
        };
        event.prevent_default();
        on_change.run(next);
        if let Some(index) = SPOTS.iter().position(|spot| *spot == next) {
            focus_choice(&event, index);
        }
    };
    view! {
        <div class="position" role="radiogroup" aria-label=i18n.text(label) on:keydown=on_key>
            {SPOTS
                .into_iter()
                .map(|spot| {
                    let checked = move || value.get() == spot;
                    let name = i18n.text(spot_name(spot));
                    view! {
                        <button
                            type="button"
                            role="radio"
                            class="position__spot"
                            aria-label=name
                            title=name
                            aria-checked=move || checked().to_string()
                            tabindex=move || if checked() { "0" } else { "-1" }
                            on:click=move |_| on_change.run(spot)
                        >
                            <span class="position__mark" aria-hidden="true"></span>
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}
