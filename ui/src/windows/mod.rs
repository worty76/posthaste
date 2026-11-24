//! One module per window. `mount` reads the window's label and shows the
//! window that goes with it; a window is added by writing its module and
//! adding one arm here.

mod capture;
mod details;
mod fatal;
mod focus;
mod main;
mod note_badges;
mod today;

use leptos::prelude::*;
use posthaste_model::{AppInfo, Os, Window as SceneWindow};

use crate::components::SceneHost;
use crate::ipc::{self, api};

/// Labels the shell gives its windows (`src-tauri/src/windows.rs`).
const MAIN: &str = "main";
const CAPTURE: &str = "capture";
const TODAY: &str = "today";
const DETAILS: &str = "details";
const FOCUS: &str = "focus";

pub fn mount() {
    let label = ipc::window_label().unwrap_or_default();
    if let Some(root) = document().document_element() {
        // Stylesheets size and paint each window through this class.
        let _ = root.class_list().add_1(&format!("is-{label}"));
        // Stylesheets also branch on the platform: Linux (WebKitGTK) cannot
        // reliably composite transparent windows, so there the windows paint
        // edge to edge instead of floating on an invisible margin.
        let os_class = match guess_os() {
            Os::Macos => "is-macos",
            Os::Windows => "is-windows",
            Os::Linux => "is-linux",
        };
        let _ = root.class_list().add_1(os_class);
    }
    // Leptos starts its own task runner when it mounts; this runs before.
    wasm_bindgen_futures::spawn_local(async move {
        let info = api::app_info().await;
        let os = info.as_ref().map_or_else(|_| guess_os(), |info| info.os);
        match label.as_str() {
            CAPTURE => show(move || {
                view! {
                    <SceneHost window=SceneWindow::Capture>
                        <capture::CaptureWindow os=os />
                    </SceneHost>
                }
            }),
            TODAY => show(move || {
                view! {
                    <SceneHost window=SceneWindow::Today>
                        <today::TodayWindow os=os />
                    </SceneHost>
                }
            }),
            DETAILS => show(move || {
                view! {
                    <SceneHost window=SceneWindow::Details>
                        <details::DetailsWindow os=os />
                    </SceneHost>
                }
            }),
            FOCUS => show(|| view! { <focus::FocusWindow /> }),
            MAIN => show_main(info),
            // The shell makes no other window.
            _ => {}
        }
    });
}

/// The main window needs the database: without it there is nothing to list
/// and nowhere to save, so it says what went wrong instead.
fn show_main(info: Result<AppInfo, posthaste_model::CommandError>) {
    let failure = match &info {
        Ok(info) => info.startup_error.clone(),
        Err(error) => Some(error.clone()),
    };
    match (info, failure) {
        // The main window's scene wraps its content pane, inside the
        // window's own view: stickers belong beside the notes, never on
        // the sidebar's buttons.
        (Ok(info), None) => show(move || view! { <main::MainWindow info=info /> }),
        (_, failure) => {
            let (code, message) = failure
                .map(|error| (error.code, error.message))
                .unwrap_or_default();
            show(move || view! { <fatal::Fatal code=code message=message /> });
        }
    }
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

/// The platform, from the user agent. Used for the stylesheet's platform
/// class before the shell is asked, and for the few things that must work
/// without it (shortcuts still need a modifier).
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
