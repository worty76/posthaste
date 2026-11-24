//! Commands for the floating focus pill: pinning one open note at a time.

use posthaste_core::Note;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::AppResult;
use crate::state::AppState;
use crate::windows;

fn apply_focus(app: &AppHandle, note: &Option<Note>) {
    if note.is_some() {
        windows::show_focus(app);
    } else {
        windows::hide_focus(app);
    }
    let _ = app.emit("focus-changed", note);
}

/// Re-reads the focused note from storage and updates the floating pill.
/// Called at startup and whenever notes or steps change.
pub(crate) fn sync_focus(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(note) = state.with_db(|db| db.focused_note()) else {
        return;
    };
    let Some(win) = app.get_webview_window(windows::FOCUS) else {
        return;
    };
    let visible = crate::panels::is_visible(app, &win);
    match (&note, visible) {
        (Some(_), false) => windows::show_focus(app),
        (None, true) => windows::hide_focus(app),
        _ => {}
    }
    if note.is_some() || visible {
        let _ = app.emit("focus-changed", &note);
    }
}

#[tauri::command]
pub fn focus_get(state: State<'_, AppState>) -> AppResult<Option<Note>> {
    state.with_db(|db| db.focused_note())
}

#[tauri::command]
pub fn focus_set(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Option<Note>> {
    let note = state.with_db(|db| db.set_focused_note(Some(&id)))?;
    apply_focus(&app, &note);
    Ok(note)
}

#[tauri::command]
pub fn focus_clear(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    state.with_db(|db| db.set_focused_note(None))?;
    apply_focus(&app, &None);
    Ok(())
}
