use std::cell::RefCell;

use leptos::html;
use leptos::prelude::*;
use posthaste_i18n::{key, Key};
use posthaste_model::Os;
use wasm_bindgen::JsCast;

use crate::components::Kbd;
use crate::logic::shortcut::{accelerator_from_press, format_accelerator, Modifiers};
use crate::state::I18n;

thread_local! {
    /// The one recorder that is recording, if any: starting one stops the
    /// previous, so a press never lands in two rows at once.
    static RECORDING: RefCell<Option<RwSignal<bool>>> = const { RefCell::new(None) };
}

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
    let button = NodeRef::<html::Button>::new();
    let name = move || {
        let t = i18n.translator();
        let action = if recording.get() {
            key::SETTINGS_KEYBOARD_RECORD
        } else {
            key::SETTINGS_KEYBOARD_CHANGE
        };
        t.format(action, &[("name", t.text(label))])
    };

    let start = move |()| {
        RECORDING.with_borrow_mut(|slot| {
            // A recorder already listening is stopped, and its window
            // listener comes down with its flag.
            if let Some(previous) = slot.take() {
                previous.set(false);
            }
            slot.replace(recording);
        });
        recording.set(true);
        // WebKit does not focus a button on click, so this is for the
        // focus ring; the capture listens on the window regardless.
        if let Some(button) = button.get() {
            let _ = button.focus();
        }
    };

    // The capture listens on the window while recording, so it catches the
    // keys whatever has focus: WebKit leaves a button unfocused after a
    // click, and a listener on the button would hear nothing. Recording
    // stops on Esc, on a usable combination, and on a click outside.
    Effect::new(move |_| {
        if !recording.get() {
            return;
        }
        let capture = window_event_listener(leptos::ev::keydown, move |event| {
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
        });
        let outside = window_event_listener(leptos::ev::mousedown, move |event| {
            let target = event
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Node>().ok());
            let inside = button
                .get_untracked()
                .is_some_and(|button| button.contains(target.as_ref()));
            if !inside {
                recording.set(false);
                RECORDING.with_borrow_mut(|slot| {
                    if slot.as_ref() == Some(&recording) {
                        slot.take();
                    }
                });
            }
        });
        on_cleanup(move || {
            capture.remove();
            outside.remove();
        });
    });

    view! {
        <button
            type="button"
            class="btn recorder"
            aria-label=name
            node_ref=button
            data-recording=move || recording.get().to_string()
            on:click=move |_| start(())
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
