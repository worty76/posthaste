use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use posthaste_i18n::Key;
use wasm_bindgen::JsCast;

use crate::state::I18n;

/// An on/off setting.
#[component]
pub fn Switch(
    #[prop(into)] label: Signal<String>,
    #[prop(into)] checked: Signal<bool>,
    #[prop(into)] on_change: Callback<bool>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            role="switch"
            class="switch"
            aria-checked=move || checked.get().to_string()
            aria-label=move || label.get()
            on:click=move |_| on_change.run(!checked.get_untracked())
        ></button>
    }
}

/// Moves keyboard focus to the `index`th button of a radio group, so focus
/// follows the checked choice as it does in any radio group.
pub(super) fn focus_choice(event: &KeyboardEvent, index: usize) {
    let choice = event
        .current_target()
        .and_then(|group| group.dyn_into::<web_sys::Element>().ok())
        .and_then(|group| group.children().item(index as u32))
        .and_then(|choice| choice.dyn_into::<web_sys::HtmlElement>().ok());
    if let Some(choice) = choice {
        let _ = choice.focus();
    }
}

/// A small set of mutually exclusive choices, shown side by side. The arrow
/// keys move through them.
#[component]
pub fn Segmented<T>(
    i18n: I18n,
    label: Key,
    #[prop(into)] value: Signal<T>,
    options: Vec<(T, Key)>,
    #[prop(into)] on_change: Callback<T>,
    #[prop(into, optional)] disabled: Signal<bool>,
) -> impl IntoView
where
    T: Copy + PartialEq + Send + Sync + 'static,
{
    let choices: Vec<T> = options.iter().map(|(choice, _)| *choice).collect();
    let on_key = move |event: KeyboardEvent| {
        let delta = match event.key().as_str() {
            "ArrowRight" | "ArrowDown" => 1,
            "ArrowLeft" | "ArrowUp" => choices.len().saturating_sub(1),
            _ => return,
        };
        event.prevent_default();
        let current = value.get_untracked();
        let Some(index) = choices.iter().position(|choice| *choice == current) else {
            return;
        };
        let next = (index + delta) % choices.len();
        on_change.run(choices[next]);
        focus_choice(&event, next);
    };
    view! {
        <div class="segmented" role="radiogroup" aria-label=i18n.text(label) on:keydown=on_key>
            {options
                .into_iter()
                .map(|(choice, name)| {
                    let checked = move || value.get() == choice;
                    view! {
                        <button
                            type="button"
                            role="radio"
                            aria-checked=move || checked().to_string()
                            tabindex=move || if checked() { "0" } else { "-1" }
                            disabled=move || disabled.get()
                            on:click=move |_| on_change.run(choice)
                        >
                            {i18n.text(name)}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}
