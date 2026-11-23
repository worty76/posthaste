//! Notes, Today glance, and Trash commands.

use posthaste_core::{now_ms, Counts, DayMark, Glance, Note, NoteQuery, Status, TrashedNote};
use tauri::{AppHandle, Emitter, Manager, State};

use super::notes_changed;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::windows;

// ---- notes -------------------------------------------------------------------

#[tauri::command]
pub fn notes_list(state: State<'_, AppState>, query: NoteQuery) -> AppResult<Vec<Note>> {
    state.with_db(|db| db.list_notes(&query))
}

#[tauri::command]
pub fn notes_counts(
    state: State<'_, AppState>,
    today_from: i64,
    today_to: i64,
) -> AppResult<Counts> {
    state.with_db(|db| db.counts(today_from, today_to))
}

#[tauri::command]
pub fn note_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    content: String,
    category: Option<String>,
) -> AppResult<Note> {
    let note = state.with_db(|db| db.update_note(&id, &content, category.as_deref()))?;
    notes_changed(&app);
    Ok(note)
}

#[tauri::command]
pub fn note_set_status(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    status: Status,
) -> AppResult<Note> {
    let note = state.with_db(|db| db.set_status(&id, status))?;
    notes_changed(&app);
    Ok(note)
}

/// Parks a note in Later, or brings it back.
#[tauri::command]
pub fn note_set_later(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    later: bool,
) -> AppResult<Note> {
    let note = state.with_db(|db| db.set_later(&id, later))?;
    notes_changed(&app);
    Ok(note)
}

#[tauri::command]
pub fn note_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_db(|db| db.trash_note(&id))?;
    notes_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn note_undelete(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<Note> {
    let note = state.with_db(|db| db.restore_trashed_at(&id, now_ms()))?;
    notes_changed(&app);
    Ok(note)
}

// ---- today -------------------------------------------------------------------

/// What the Today window shows. Bounds are the caller's local day, exactly
/// as `notes_counts` takes them.
/// A day that has already ended is rebuilt as it stood then; today uses the
/// live rules.
#[tauri::command]
pub fn glance_get(state: State<'_, AppState>, today_from: i64, today_to: i64) -> AppResult<Glance> {
    state.with_db(|db| db.glance_at(today_from, today_to, now_ms()))
}

/// How many notes were captured on each day between the edges, for the
/// calendar's dots. Edges are local day starts plus the end of the last day.
#[tauri::command]
pub fn notes_day_marks(state: State<'_, AppState>, edges: Vec<i64>) -> AppResult<Vec<DayMark>> {
    state.with_db(|db| db.day_marks(&edges))
}

/// Opens the inbox filtered to the notes captured on `day` (`YYYY-MM-DD`).
/// Used by the "where it came from" tag in Today, so Today steps aside.
#[tauri::command]
pub fn inbox_open_date(app: AppHandle, day: String) -> AppResult<()> {
    if !valid_day(&day) {
        return Err(AppError::new(
            "bad_request",
            "Expected a date like 2026-11-09.",
        ));
    }
    windows::hide_today(&app, false);
    // One event: the inbox switches view itself, so nothing can arrive while
    // its listeners are being replaced.
    windows::show_main(&app, None);
    if let Some(window) = app.get_webview_window(windows::MAIN) {
        let _ = window.emit("inbox-show-date", day);
    }
    Ok(())
}

/// True for a real calendar date written `YYYY-MM-DD`.
fn valid_day(day: &str) -> bool {
    let b = day.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let s = &day[r];
        s.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| s.parse().ok())
            .flatten()
    };
    let (Some(y), Some(m), Some(d)) = (num(0..4), num(5..7), num(8..10)) else {
        return false;
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&d)
}

#[tauri::command]
pub fn today_dismiss(app: AppHandle) {
    windows::hide_today(&app, true);
}

// ---- trash -------------------------------------------------------------------

/// What the Trash section lists: newest deletion first, within the keep time.
#[tauri::command]
pub fn trash_list(state: State<'_, AppState>) -> AppResult<Vec<TrashedNote>> {
    state.with_db(|db| db.list_trashed())
}

/// Erases one trashed note for good.
#[tauri::command]
pub fn trash_purge(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_db(|db| db.purge_note(&id))?;
    notes_changed(&app);
    Ok(())
}

/// Erases for good the trash as the user saw it: notes deleted up to
/// `up_to`, the newest deletion in the list. Returns how many notes.
#[tauri::command]
pub fn trash_empty(app: AppHandle, state: State<'_, AppState>, up_to: i64) -> AppResult<usize> {
    let n = state.with_db(|db| db.empty_trash_at(up_to, now_ms()))?;
    notes_changed(&app);
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::valid_day;

    #[test]
    fn inbox_open_date_rejects_a_malformed_day() {
        assert!(valid_day("2026-11-09"));
        assert!(valid_day("2024-02-29"));
        assert!(!valid_day("2026-13-01"));
        assert!(!valid_day("2026-02-30"));
        assert!(!valid_day("2026-1-09"));
        assert!(!valid_day("2026-11-09T00"));
        assert!(!valid_day(""));
    }
}
