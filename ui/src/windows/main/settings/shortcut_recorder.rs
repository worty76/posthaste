use std::cell::RefCell;

use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Key};
use posthaste_model::Os;
use wasm_bindgen::JsCast;

use crate::components::Kbd;
use crate::ipc::api;
use crate::logic::shortcut::{accelerator_from_press, format_accelerator, Modifiers};
use crate::state::I18n;

thread_local! {
    /// The one recorder that is recording, if any: starting one stops the
    /// previous, so a press never lands in two rows at once.
    static RECORDING: RefCell<Option<RwSignal<bool>>> = const { RefCell::new(None) };
}

/// True while any shortcut recorder in Settings is actively waiting for keys.
pub fn is_recording() -> bool {
    RECORDING.with_borrow(|slot| slot.is_some())
}

fn set_shortcuts_paused(paused: bool) {
    spawn_local(async move {
        let _ = api::shortcuts_set_paused(paused).await;
    });
}

/// Click, then press the combination to use. Click again, press Esc, or click outside to cancel.
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

    let stop = move |resume_shortcuts: bool| {
        recording.set(false);
        let was_active = RECORDING.with_borrow_mut(|slot| {
            if slot.as_ref() == Some(&recording) {
                slot.take();
                true
            } else {
                false
            }
        });
        if resume_shortcuts && was_active {
            set_shortcuts_paused(false);
        }
    };

    let toggle = move |()| {
        if recording.get_untracked() {
            stop(true);
            return;
        }
        let had_previous = RECORDING.with_borrow_mut(|slot| {
            let had = if let Some(previous) = slot.take() {
                previous.set(false);
                true
            } else {
                false
            };
            slot.replace(recording);
            had
        });
        recording.set(true);
        if !had_previous {
            set_shortcuts_paused(true);
        }
        // WebKit does not focus a button on click, so this is for the
        // focus ring; the capture listens on the window regardless.
        if let Some(button) = button.get() {
            let _ = button.focus();
        }
    };

    // Component-scoped window listeners (automatically removed on unmount):
    // avoids leaking listeners across Effect re-runs and catches keys even
    // when WebKit leaves the button unfocused.
    let keydown = window_event_listener(leptos::ev::keydown, move |event| {
        if !recording.get_untracked() {
            return;
        }
        event.prevent_default();
        event.stop_propagation();
        if event.key() == "Escape" {
            stop(true);
            return;
        }
        let held = Modifiers {
            ctrl: event.ctrl_key(),
            alt: event.alt_key(),
            shift: event.shift_key(),
            meta: event.meta_key(),
        };
        if let Some(accelerator) = accelerator_from_press(&event.code(), held, os) {
            // `settings_update` will re-apply shortcuts if it changed; resume
            // here too in case the user recorded the same shortcut as before.
            stop(true);
            on_change.run(accelerator);
        }
    });

    let outside = window_event_listener(leptos::ev::mousedown, move |event| {
        if !recording.get_untracked() {
            return;
        }
        let target = event
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Node>().ok());
        let inside = button
            .get_untracked()
            .is_some_and(|button| button.contains(target.as_ref()));
        if !inside {
            stop(true);
        }
    });

    on_cleanup(move || {
        keydown.remove();
        outside.remove();
        stop(true);
    });

    view! {
        <button
            type="button"
            class="btn recorder"
            aria-label=name
            node_ref=button
            data-recording=move || recording.get().to_string()
            on:click=move |_| toggle(())
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
