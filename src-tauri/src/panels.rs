//! How the floating windows (capture, Today) come to the front.
//!
//! On macOS they are non-activating panels, like Spotlight: they take the
//! keyboard without making Posthaste the active application. Activating it
//! instead (what focusing an ordinary window does) pulls every other
//! Posthaste window in front of the application the user is working in,
//! greys that application out, and can switch desktops. Elsewhere, and if a
//! window could not be made a panel, they are shown and focused as usual.

use tauri::{AppHandle, WebviewWindow};

#[cfg(target_os = "macos")]
mod mac {
    use tauri::{AppHandle, Manager, WebviewWindow};
    use tauri_nspanel::{
        tauri_panel, CollectionBehavior, ManagerExt, Panel, StyleMask, WebviewWindowExt,
    };

    tauri_panel! {
        panel!(FloatingPanel {
            config: {
                can_become_key_window: true,
                can_become_main_window: false,
                is_floating_panel: true
            }
        })
    }

    /// Turns `window` into a non-activating panel. Main thread only.
    pub fn adopt(app: &AppHandle, window: &WebviewWindow) {
        let label = window.label().to_string();
        if app
            .try_state::<tauri_nspanel::WebviewPanelManager<tauri::Wry>>()
            .is_none()
        {
            if let Err(e) = app.plugin(tauri_nspanel::init()) {
                log::error!("panels are unavailable: {e}");
                return;
            }
        }
        let panel = match window.to_panel::<FloatingPanel>() {
            Ok(panel) => panel,
            Err(e) => {
                log::error!("{label} could not be made a panel: {e}");
                return;
            }
        };
        if let Err(e) = panel.add_style_mask(StyleMask::empty().nonactivating_panel().into()) {
            log::error!("{label} could not be made non-activating: {e:?}");
        }
        // On every desktop, and over an application in full screen.
        panel.set_collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary()
                .into(),
        );
        panel.set_hides_on_deactivate(false);
    }

    /// Brings the panel forward with the keyboard, without activating the
    /// application. False when `window` is not a panel.
    pub fn show(app: &AppHandle, window: &WebviewWindow) -> bool {
        let Ok(panel) = app.get_webview_panel(window.label()) else {
            return false;
        };
        // AppKit calls belong on the main thread; shortcuts arrive elsewhere.
        let result = app.run_on_main_thread(move || panel.show_and_make_key());
        if let Err(e) = result {
            log::error!("could not schedule showing a panel: {e}");
        }
        true
    }
}

/// Call once per floating window, right after it is built.
pub fn adopt(app: &AppHandle, window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    mac::adopt(app, window);
    #[cfg(not(target_os = "macos"))]
    let _ = (app, window);
}

/// Shows a floating window. Returns true when it also has the keyboard
/// already, so the caller must not focus it (focusing activates the app).
pub fn show(app: &AppHandle, window: &WebviewWindow) -> tauri::Result<bool> {
    #[cfg(target_os = "macos")]
    if mac::show(app, window) {
        return Ok(true);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    window.show()?;
    Ok(false)
}
