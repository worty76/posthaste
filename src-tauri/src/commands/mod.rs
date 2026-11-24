//! Commands the frontend can invoke. Each is a thin adapter: validation and
//! persistence live in `posthaste-core`, window behaviour in `windows`.
//!
//! - `capture`: quick capture window and capture draft persistence
//! - `notes`: listing, counts, Today glance, status/later transitions, Trash
//! - `steps`: ordered one-line steps inside a note
//! - `projects`: project CRUD, colors, archive state, note assignment
//! - `details`: Note details window, descriptions, and file attachments
//! - `focus`: floating focus pill for the one note currently in progress
//! - `settings`: preferences, launch at login, global shortcut application
//! - `data`: database info, Markdown/JSON export & import, backup, full reset
//! - `looks`: look gallery, live preview, share codes, `.phlook` packages
//! - `sync`: peer-to-peer device pairing and manual sync triggers

mod capture;
mod data;
mod details;
mod focus;
mod looks;
mod notes;
mod projects;
mod settings;
mod steps;
mod sync;

pub use capture::*;
pub use data::*;
pub use details::*;
pub use focus::*;
pub use looks::*;
pub use notes::*;
pub use projects::*;
pub use settings::*;
pub use steps::*;
pub use sync::*;

use std::sync::atomic::Ordering;

use posthaste_core::{AppInfo, CommandError, Os};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::AppState;

pub(super) fn notes_changed(app: &AppHandle) {
    // Only wake windows that are currently visible on screen; hidden windows
    // refresh when shown (`show_main`, `today-shown`, `details-open`), so
    // quick capture and step typing never trigger background SQLite queries.
    if let Some(main) = app.get_webview_window(crate::windows::MAIN) {
        if main.is_visible().unwrap_or(false) {
            let _ = main.emit("notes-changed", ());
        }
    }
    for label in [crate::windows::TODAY, crate::windows::DETAILS] {
        if let Some(win) = app.get_webview_window(label) {
            if crate::panels::is_visible(app, &win) {
                let _ = win.emit("notes-changed", ());
            }
        }
    }
    focus::sync_focus(app);
    if let Some(svc) = app.try_state::<crate::sync::SyncService>() {
        svc.notify_note_changed();
    }
}

pub(super) fn looks_changed(app: &AppHandle) {
    let _ = app.emit("looks-changed", ());
}

/// Waits for a parented file dialog. The callback API is the one that
/// honours the parent — the dialog sheets onto the calling window, above
/// the floating panels — so the answer comes back over a channel instead
/// of through the parentless blocking run.
pub(super) fn dialog_answer<T: Send + Default + 'static>(
    run: impl FnOnce(std::sync::mpsc::Sender<T>),
) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    run(tx);
    // The dialog always answers, even on cancel (`None`); a dropped
    // channel means the app is on its way out, which reads as cancelled.
    rx.recv().unwrap_or_default()
}

#[tauri::command]
pub fn app_info(app: AppHandle, state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        os: Os::current(),
        session: posthaste_platform::session().as_str().to_owned(),
        tray_available: state.tray_available.load(Ordering::Relaxed),
        startup_error: state.startup_error.clone().map(|error| CommandError {
            code: error.code,
            message: error.message,
        }),
    }
}
