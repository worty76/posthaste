//! One module per window. `mount` reads the window's label and shows the
//! window that goes with it; a window is added by writing its module and
//! adding one arm here.

mod capture;

use leptos::prelude::*;
use posthaste_model::Os;

use crate::ipc::{self, api};

/// Labels the shell gives its windows (`src-tauri/src/windows.rs`).
const CAPTURE: &str = "capture";

pub fn mount() {
    let label = ipc::window_label().unwrap_or_default();
    if let Some(root) = document().document_element() {
        // Stylesheets size and paint each window through this class.
        let _ = root.class_list().add_1(&format!("is-{label}"));
    }
    // Leptos starts its own task runner when it mounts; this runs before.
    wasm_bindgen_futures::spawn_local(async move {
        let os = match api::app_info().await {
            Ok(info) => info.os,
            Err(_) => guess_os(),
        };
        // One arm per window; the next ported window adds the second.
        #[allow(clippy::single_match)]
        match label.as_str() {
            CAPTURE => show(move || view! { <capture::CaptureWindow os=os /> }),
            // A window that has not moved to Rust yet is never sent here.
            _ => {}
        }
    });
}

/// Renders into `#root`, which the stylesheets size to fill the window.
fn show<V: IntoView + 'static>(view: impl FnOnce() -> V + 'static) {
    use wasm_bindgen::JsCast;
    let root = document()
        .get_element_by_id("root")
        .and_then(|root| root.dyn_into::<web_sys::HtmlElement>().ok());
    if let Some(root) = root {
        // The page lives exactly as long as its window.
        mount_to(root, view).forget();
    }
}

/// Only for when the shell cannot be asked: shortcuts still need a modifier.
fn guess_os() -> Os {
    let agent = window().navigator().user_agent().unwrap_or_default();
    if agent.contains("Mac") {
        Os::Macos
    } else if agent.contains("Windows") {
        Os::Windows
    } else {
        Os::Linux
    }
}
