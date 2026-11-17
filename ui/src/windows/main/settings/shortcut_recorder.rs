use leptos::prelude::*;
use posthaste_i18n::{key, Key};
use posthaste_model::Os;

use crate::components::Kbd;
use crate::logic::shortcut::{accelerator_from_press, format_accelerator, Modifiers};
use crate::state::I18n;

/// Click, then press the combination to use. Esc cancels.
#[component]
pub(super) fn ShortcutRecorder(
    i18n: I18n,
    os: Os,
    /// The name of the shortcut being set.
    label: Key,
    value: Signal<String>,
    on_change: Callback<String>,
) -> impl IntoView {
    let recording = RwSignal::new(false);
    let name = move || {
        let t = i18n.translator();
        let action = if recording.get() {
            key::SETTINGS_KEYBOARD_RECORD
        } else {
            key::SETTINGS_KEYBOARD_CHANGE
        };
        t.format(action, &[("name", t.text(label))])
    };
    view! {
        <button
            type="button"
            class="btn recorder"
            aria-label=name
            data-recording=move || recording.get().to_string()
            on:click=move |_| recording.set(true)
            on:blur=move |_| recording.set(false)
            // While recording, no key may reach the window's own shortcuts.
            on:keydown=move |event| {
                if !recording.get_untracked() {
                    return;
                }
                event.prevent_default();
                event.stop_propagation();
                if event.key() == "Escape" {
                    recording.set(false);
                    return;
                }
                let held = Modifiers {
                    ctrl: event.ctrl_key(),
                    alt: event.alt_key(),
                    shift: event.shift_key(),
                    meta: event.meta_key(),
                };
                if let Some(accelerator) = accelerator_from_press(&event.code(), held, os) {
                    recording.set(false);
                    on_change.run(accelerator);
                }
            }
        >
            {move || {
                if recording.get() {
                    i18n.translator().text(key::SETTINGS_KEYBOARD_RECORDING).into_any()
                } else if value.with(String::is_empty) {
                    i18n.translator().text(key::SETTINGS_KEYBOARD_SET).into_any()
                } else {
                    view! { <Kbd keys=format_accelerator(&value.get(), os) /> }.into_any()
                }
            }}
        </button>
    }
}
