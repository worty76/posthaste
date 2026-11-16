//! System tray (Windows, Linux) / menu bar (macOS) icon and its menu.

use posthaste_i18n::{key, Key, Translator};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Wry};

use crate::windows;

/// The tray menu's items and the message each one shows, kept so the menu
/// can be rewritten when the language changes. Managed Tauri state.
pub struct TrayMenu {
    items: Vec<(MenuItem<Wry>, Key)>,
}

impl TrayMenu {
    /// Rewrites every item in the translator's language.
    pub fn translate(&self, t: Translator) {
        for (item, label) in &self.items {
            if let Err(e) = item.set_text(t.text(*label)) {
                log::warn!("tray menu item could not be renamed: {e}");
            }
        }
    }
}

pub fn create(app: &AppHandle, t: Translator) -> tauri::Result<TrayMenu> {
    let item = |id: &str, label: Key| -> tauri::Result<(MenuItem<Wry>, Key)> {
        let item = MenuItem::with_id(app, id, t.text(label), true, None::<&str>)?;
        Ok((item, label))
    };
    let open = item("open", key::DESKTOP_TRAY_OPEN)?;
    let capture = item("capture", key::DESKTOP_TRAY_CAPTURE)?;
    let inbox = item("inbox", key::DESKTOP_TRAY_INBOX)?;
    let settings = item("settings", key::DESKTOP_TRAY_SETTINGS)?;
    let quit = item("quit", key::DESKTOP_TRAY_QUIT)?;
    let menu = Menu::with_items(
        app,
        &[
            &open.0,
            &capture.0,
            &inbox.0,
            &PredefinedMenuItem::separator(app)?,
            &settings.0,
            &PredefinedMenuItem::separator(app)?,
            &quit.0,
        ],
    )?;

    // macOS gets a monochrome template image; elsewhere panels can be light
    // or dark, so the full-colour icon is the one that is always visible.
    #[cfg(target_os = "macos")]
    let icon = Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    #[cfg(not(target_os = "macos"))]
    let icon = Image::from_bytes(include_bytes!("../icons/tray-color.png"))?;
    let builder = TrayIconBuilder::with_id("posthaste")
        .icon(icon)
        .tooltip("Posthaste")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => windows::show_main(app, None),
            "capture" => windows::show_capture(app),
            "inbox" => windows::show_main(app, Some("inbox")),
            "settings" => windows::show_main(app, Some("settings")),
            "quit" => app.exit(0),
            _ => {}
        });
    // A template image lets macOS tint the icon for light and dark menu bars.
    #[cfg(target_os = "macos")]
    let builder = builder.icon_as_template(true);
    builder.build(app)?;
    Ok(TrayMenu {
        items: vec![open, capture, inbox, settings, quit],
    })
}
