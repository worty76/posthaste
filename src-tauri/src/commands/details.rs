//! Note details window, description, and file attachment commands.

use posthaste_core::{
    AttachFailure, AttachReport, Attachment, Note, NoteDetail, MAX_ATTACHMENT_BYTES,
};
use posthaste_i18n::key;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::{AppHandle, Manager, State, Window};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::{dialog_answer, notes_changed};
use crate::error::{AppError, AppResult};
use crate::state::{lock, AppState};
use crate::windows::{self, DetailsTarget};

/// Opens the Note details window for a note. `from` is "today" or "inbox":
/// where Esc goes back to.
#[tauri::command]
pub fn details_open(app: AppHandle, id: String, from: String) {
    windows::show_details(&app, windows::details_target(id, &from));
}

#[tauri::command]
pub fn details_dismiss(app: AppHandle) {
    windows::hide_details(&app);
}

/// The note the details window should show, for when it loads after the
/// `details-open` event was sent.
#[tauri::command]
pub fn details_current(state: State<'_, AppState>) -> Option<DetailsTarget> {
    lock(&state.details_target).clone()
}

#[tauri::command]
pub fn note_detail_get(state: State<'_, AppState>, id: String) -> AppResult<NoteDetail> {
    state.with_db(|db| db.note_detail(&id))
}

#[tauri::command]
pub fn note_details_set(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    details: String,
) -> AppResult<Note> {
    let note = state.with_db(|db| db.set_details(&id, &details))?;
    notes_changed(&app);
    Ok(note)
}

fn attach_paths(
    app: &AppHandle,
    state: &AppState,
    note_id: &str,
    paths: Vec<std::path::PathBuf>,
) -> AttachReport {
    let mut report = AttachReport::default();
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into());
        let result = std::fs::metadata(&path)
            .map_err(|e| AppError::new("io", format!("The file could not be read: {e}")))
            .and_then(|m| {
                if !m.is_file() {
                    Err(AppError::new(
                        "not_a_file",
                        "Folders can't be attached. Attach the files inside it instead.",
                    ))
                } else if m.len() as usize > MAX_ATTACHMENT_BYTES {
                    // Refuse before reading a huge file into memory.
                    Err(AppError::from(
                        posthaste_core::CoreError::AttachmentTooLarge {
                            size_mb: format!("{:.1}", m.len() as f64 / (1024.0 * 1024.0)),
                        },
                    ))
                } else {
                    std::fs::read(&path).map_err(|e| {
                        AppError::new("io", format!("The file could not be read: {e}"))
                    })
                }
            })
            .and_then(|bytes| state.with_db(|db| db.add_attachment(note_id, &name, "", &bytes)));
        match result {
            Ok(a) => report.added.push(a),
            Err(e) => report.failed.push(AttachFailure {
                name,
                code: e.code,
                message: e.message,
            }),
        }
    }
    if !report.added.is_empty() {
        notes_changed(app);
    }
    report
}

/// Asks which files to attach. `None` means cancelled.
#[tauri::command]
pub async fn attachment_add_files(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    note_id: String,
) -> AppResult<Option<AttachReport>> {
    let Some(picked) = dialog_answer(|tx| {
        app.dialog()
            .file()
            .set_parent(&window)
            .set_title(state.translator().text(key::DESKTOP_DIALOG_ATTACH))
            .pick_files(move |picked| {
                let _ = tx.send(picked);
            });
    }) else {
        return Ok(None);
    };
    let paths = picked
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .collect();
    Ok(Some(attach_paths(&app, &state, &note_id, paths)))
}

/// Attaches files dropped on the details window.
#[tauri::command]
pub fn attachment_add_paths(
    app: AppHandle,
    state: State<'_, AppState>,
    note_id: String,
    paths: Vec<String>,
) -> AttachReport {
    let paths = paths.into_iter().map(std::path::PathBuf::from).collect();
    attach_paths(&app, &state, &note_id, paths)
}

/// Attaches bytes sent as a raw body (a pasted image). The note id, name and
/// type travel in headers, the name percent-encoded.
#[tauri::command]
pub fn attachment_add_bytes(
    app: AppHandle,
    state: State<'_, AppState>,
    request: Request<'_>,
) -> AppResult<Attachment> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(AppError::new("bad_request", "Expected file bytes."));
    };
    let header = |key: &str| {
        request
            .headers()
            .get(key)
            .and_then(|v| v.to_str().ok())
            .map(percent_decode)
            .unwrap_or_default()
    };
    let note_id = header("x-note-id");
    let attachment = state
        .with_db(|db| db.add_attachment(&note_id, &header("x-name"), &header("x-mime"), bytes))?;
    notes_changed(&app);
    Ok(attachment)
}

/// The bytes of an image attachment, for the preview. Not for other types.
#[tauri::command]
pub fn attachment_preview(state: State<'_, AppState>, id: String) -> AppResult<Response> {
    let (meta, bytes) = state.with_db(|db| db.attachment_bytes(&id))?;
    if !meta.mime.starts_with("image/") {
        return Err(AppError::new("no_preview", "Only images have a preview."));
    }
    Ok(Response::new(bytes))
}

/// Opens an attachment in the app the system uses for its type. The file
/// is written to a private cache folder first; that folder is emptied at
/// the next start.
#[tauri::command]
pub fn attachment_open(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    let (meta, bytes) = state.with_db(|db| db.attachment_bytes(&id))?;
    let dir = opened_dir(&app)?.join(&meta.id);
    let path = dir.join(&meta.name);
    std::fs::create_dir_all(&dir)
        .and_then(|_| std::fs::write(&path, &bytes))
        .map_err(|e| AppError::new("io", format!("The file could not be prepared: {e}")))?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::new("io", format!("No app could open this file: {e}")))
}

/// Where opened attachments are written. Inside the app's own cache folder.
pub fn opened_dir(app: &AppHandle) -> AppResult<std::path::PathBuf> {
    app.path()
        .app_cache_dir()
        .map(|d| d.join("opened-attachments"))
        .map_err(|e| AppError::new("io", format!("The cache folder is unavailable: {e}")))
}

/// Asks where to save a copy of an attachment. `None` means cancelled.
#[tauri::command]
pub async fn attachment_save_copy(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Option<String>> {
    let (meta, bytes) = state.with_db(|db| db.attachment_bytes(&id))?;
    let Some(dest) = dialog_answer(|tx| {
        app.dialog()
            .file()
            .set_parent(&window)
            .set_title(state.translator().text(key::DESKTOP_DIALOG_SAVE_COPY))
            .set_file_name(&meta.name)
            .save_file(move |dest| {
                let _ = tx.send(dest);
            });
    }) else {
        return Ok(None);
    };
    let dest = dest
        .into_path()
        .map_err(|e| AppError::new("io", e.to_string()))?;
    std::fs::write(&dest, bytes)
        .map_err(|e| AppError::new("io", format!("The copy could not be saved: {e}")))?;
    Ok(Some(dest.to_string_lossy().into_owned()))
}

#[tauri::command]
pub fn attachment_remove(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_db(|db| db.remove_attachment(&id))?;
    notes_changed(&app);
    Ok(())
}

/// Decodes `%XX` escapes (what `encodeURIComponent` produces). Invalid
/// escapes are kept as written.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::percent_decode;

    #[test]
    fn percent_decoding_matches_encode_uri_component() {
        assert_eq!(percent_decode("Pasted%20image.png"), "Pasted image.png");
        assert_eq!(percent_decode("caf%C3%A9.txt"), "café.txt");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("a%zzb"), "a%zzb");
        assert_eq!(percent_decode("%41"), "A");
    }
}
