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
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};

    use tauri::{AppHandle, Manager, WebviewWindow};
    use tauri_nspanel::{tauri_panel, CollectionBehavior, ManagerExt, StyleMask, WebviewWindowExt};

    /// Labels of panels currently visible to the user.
    ///
    /// On macOS, hiding a `WKWebView` panel with `orderOut:` lets WebKit mark
    /// the page occluded and purge its GPU backing store, which flashes a solid
    /// black rectangle and lags when ordered back in. Instead we hide by
    /// setting `alphaValue = 0.0` and `ignoresMouseEvents = true`, and track
    /// user-facing visibility here.
    fn visible_panels() -> &'static Mutex<HashSet<String>> {
        static VISIBLE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
        VISIBLE.get_or_init(|| Mutex::new(HashSet::new()))
    }

    fn set_visible(label: &str, visible: bool) {
        let mut set = visible_panels().lock().unwrap_or_else(|e| e.into_inner());
        if visible {
            set.insert(label.to_owned());
        } else {
            set.remove(label);
        }
    }

    pub fn is_visible(app: &AppHandle, window: &WebviewWindow) -> Option<bool> {
        let label = window.label();
        app.get_webview_panel(label).ok()?;
        let set = visible_panels().lock().unwrap_or_else(|e| e.into_inner());
        Some(set.contains(label))
    }

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
        // On every desktop and over full-screen apps, hidden from Mission
        // Control and Cmd+` cycling so a warm-hidden panel never leaks into OS
        // window switchers.
        panel.set_collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary()
                .transient()
                .ignores_cycle()
                .into(),
        );
        panel.set_hides_on_deactivate(false);
        panel.set_transparent(true);
        panel.set_has_shadow(false);
        panel.set_released_when_closed(false);
        // Warm the WKWebView backing surface invisibly at startup so even the
        // very first shortcut press paints immediately with no black flash.
        panel.set_alpha_value(0.0);
        panel.set_ignores_mouse_events(true);
        panel.order_front_regardless();
        set_visible(&label, false);
    }

    /// Brings the panel forward with the keyboard, without activating the
    /// application. Must be called on the main thread. False when `window` is
    /// not a panel.
    pub fn show_now(app: &AppHandle, window: &WebviewWindow) -> bool {
        let label = window.label();
        let Ok(panel) = app.get_webview_panel(label) else {
            return false;
        };
        set_visible(label, true);
        panel.set_ignores_mouse_events(false);
        panel.set_alpha_value(1.0);
        // makeKeyAndOrderFront is the one call that makes a non-activating
        // panel key while another application is frontmost — makeKeyWindow
        // alone is ignored until the app is active, which left every opened
        // window waiting for a click.
        panel.make_key_and_order_front();
        true
    }

    /// Brings the panel forward with the keyboard, without activating the
    /// application. False when `window` is not a panel.
    pub fn show(app: &AppHandle, window: &WebviewWindow) -> bool {
        let label = window.label().to_string();
        let Ok(panel) = app.get_webview_panel(&label) else {
            return false;
        };
        set_visible(&label, true);
        // AppKit calls belong on the main thread; shortcuts arrive elsewhere.
        let result = app.run_on_main_thread(move || {
            panel.set_ignores_mouse_events(false);
            panel.set_alpha_value(1.0);
            panel.make_key_and_order_front();
        });
        if let Err(e) = result {
            log::error!("could not schedule showing a panel: {e}");
        }
        true
    }

    /// Warm-hides the panel without ordering it out, keeping its WKWebView
    /// backing store alive. Must be called on the main thread. False when
    /// `window` is not a panel.
    pub fn hide_now(app: &AppHandle, window: &WebviewWindow) -> bool {
        let label = window.label();
        let Ok(panel) = app.get_webview_panel(label) else {
            return false;
        };
        set_visible(label, false);
        panel.set_alpha_value(0.0);
        panel.set_ignores_mouse_events(true);
        panel.resign_key_window();
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

/// Whether a floating window is currently visible to the user.
pub fn is_visible(app: &AppHandle, window: &WebviewWindow) -> bool {
    #[cfg(target_os = "macos")]
    if let Some(visible) = mac::is_visible(app, window) {
        return visible;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    window.is_visible().unwrap_or(false)
}

/// Shows a floating window from the main thread without another thread hop.
/// Returns true when it also has the keyboard already.
pub fn show_on_main_thread(app: &AppHandle, window: &WebviewWindow) -> tauri::Result<bool> {
    #[cfg(target_os = "macos")]
    if mac::show_now(app, window) {
        return Ok(true);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    window.show()?;
    Ok(false)
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

/// Hides a floating window from the main thread.
pub fn hide_on_main_thread(app: &AppHandle, window: &WebviewWindow) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    if mac::hide_now(app, window) {
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    window.hide()
}

/// Hides a floating window, scheduling onto the main thread on macOS.
pub fn hide(app: &AppHandle, window: &WebviewWindow) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let handle = app.clone();
        let win = window.clone();
        app.run_on_main_thread(move || {
            let _ = hide_on_main_thread(&handle, &win);
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        window.hide()
    }
}
