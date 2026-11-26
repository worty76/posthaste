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

/// Reads plain text from the OS clipboard for Linux capture paste, where
/// WebKitGTK ignores virtual-keyboard `Ctrl+V` / `Shift+Insert` and blocks
/// `navigator.clipboard.readText()`. Never logs clipboard contents.
#[tauri::command]
pub async fn capture_clipboard_read(app: AppHandle) -> AppResult<String> {
    #[cfg(target_os = "linux")]
    {
        use std::time::Duration;

        let (tx, rx) = tokio::sync::oneshot::channel();
        app.run_on_main_thread(move || {
            gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).request_text(move |_, text| {
                let _ = tx.send(text.map(|s| s.to_string()).unwrap_or_default());
            });
        })
        .map_err(|e| crate::error::AppError::new("clipboard_read", e.to_string()))?;
        match tokio::time::timeout(Duration::from_secs(2), rx).await {
            Ok(Ok(text)) => Ok(text),
            Ok(Err(_)) | Err(_) => Err(crate::error::AppError::new(
                "clipboard_read",
                "Clipboard did not answer.",
            )),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        Ok(String::new())
    }
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
