//! Events the shell sends. Each has a marker type that fixes its name and
//! payload, so a listener cannot ask for an event that does not exist or
//! read its payload as the wrong shape.

use leptos::prelude::on_cleanup;
use leptos::task::spawn_local;
use posthaste_model::{CaptureShown as CaptureShownPayload, Settings};
use send_wrapper::SendWrapper;
use serde::{de::DeserializeOwned, Deserialize};
use wasm_bindgen::prelude::*;

use super::tauri::tauri_listen;

pub trait Event {
    const NAME: &'static str;
    type Payload: DeserializeOwned + 'static;
}

macro_rules! event {
    ($(#[$doc:meta])* $marker:ident, $name:literal, $payload:ty) => {
        $(#[$doc])*
        pub struct $marker;
        impl Event for $marker {
            const NAME: &'static str = $name;
            type Payload = $payload;
        }
    };
}

event!(
    /// Settings were saved, in this window or another.
    SettingsChanged, "settings-changed", Settings
);
event!(
    /// A look was added, edited or removed.
    LooksChanged, "looks-changed", ()
);
event!(
    /// The capture window was just shown.
    CaptureShown, "capture-shown", CaptureShownPayload
);
event!(
    /// The capture window was hidden by something other than its own keys.
    CaptureHidden, "capture-hidden", ()
);

event!(
    /// A note was added, changed or removed, in any window.
    NotesChanged, "notes-changed", ()
);
event!(
    /// The Today window was just shown by its shortcut.
    TodayShown, "today-shown", ()
);
event!(
    /// Note details closed and gave the Today window back.
    TodayReturned, "today-returned", ()
);

#[derive(Deserialize)]
struct Envelope<T> {
    payload: T,
}

/// Listens for `E` for as long as the calling component lives.
pub fn on<E: Event>(mut handler: impl FnMut(E::Payload) + 'static) {
    let closure = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
        if let Ok(envelope) = serde_wasm_bindgen::from_value::<Envelope<E::Payload>>(event) {
            handler(envelope.payload);
        }
    });
    // Shared between the pending registration and the cleanup, whichever
    // comes first. Single-threaded: the wrapper only satisfies `Send`.
    let state = std::rc::Rc::new(std::cell::RefCell::new(Listener {
        closure: Some(closure),
        unlisten: None,
        dropped: false,
    }));

    let registering = state.clone();
    spawn_local(async move {
        // Registering is a plain call; only its answer is awaited, so the
        // borrow is released before anything else can run.
        let registered = match registering.borrow().closure.as_ref() {
            Some(closure) => tauri_listen(E::NAME, closure),
            None => return,
        };
        let Ok(promise) = registered else {
            return;
        };
        let Ok(unlisten) = wasm_bindgen_futures::JsFuture::from(promise).await else {
            return;
        };
        let unlisten: js_sys::Function = unlisten.unchecked_into();
        let mut state = registering.borrow_mut();
        if state.dropped {
            let _ = unlisten.call0(&JsValue::NULL);
            state.closure = None;
        } else {
            state.unlisten = Some(unlisten);
        }
    });

    let cleanup = SendWrapper::new(state);
    on_cleanup(move || {
        let mut state = cleanup.borrow_mut();
        state.dropped = true;
        if let Some(unlisten) = state.unlisten.take() {
            let _ = unlisten.call0(&JsValue::NULL);
            state.closure = None;
        }
    });
}

struct Listener {
    closure: Option<Closure<dyn FnMut(JsValue)>>,
    unlisten: Option<js_sys::Function>,
    dropped: bool,
}
