//! Quick capture window and capture draft commands.

use posthaste_core::{Draft, NewNote, Note};
use tauri::{AppHandle, State};

use super::notes_changed;
use crate::error::AppResult;
use crate::state::{lock, AppState};
use crate::windows;

#[tauri::command]
pub fn capture_save(
    app: AppHandle,
    state: State<'_, AppState>,
    content: String,
    category: Option<String>,
    hide: bool,
) -> AppResult<Note> {
    let source = lock(&state.previous_app)
        .as_ref()
        .and_then(|p| p.name.clone());
    // Returns only after SQLite has committed; the window stays up on error
    // so the text is still there to retry.
    let note = state.with_db(|db| {
        db.capture_note(NewNote {
            content,
            category,
            source,
        })
    })?;
    notes_changed(&app);
    if hide {
        windows::hide_capture(&app, true);
    }
    Ok(note)
}

#[tauri::command]
pub fn capture_dismiss(app: AppHandle) {
    windows::hide_capture(&app, true);
}

#[tauri::command]
pub fn capture_open(app: AppHandle) {
    windows::show_capture(&app);
}

#[tauri::command]
pub fn draft_get(state: State<'_, AppState>) -> AppResult<Option<Draft>> {
    let retention = state.settings().draft_retention;
    state.with_db(|db| db.load_draft(retention))
}

#[tauri::command]
pub fn draft_set(
    state: State<'_, AppState>,
    content: String,
    category: Option<String>,
) -> AppResult<()> {
    state.with_db(|db| db.save_draft(&content, category.as_deref()))
}

#[tauri::command]
pub fn draft_clear(state: State<'_, AppState>) -> AppResult<()> {
    state.with_db(|db| db.clear_draft())
}
