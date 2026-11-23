//! Commands for ordered one-line steps inside a note.

use posthaste_core::{NewStep, Step};
use tauri::{AppHandle, State};

use super::notes_changed;
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub fn steps_add(
    app: AppHandle,
    state: State<'_, AppState>,
    note_id: String,
    after: Option<String>,
    lines: Vec<NewStep>,
) -> AppResult<Vec<Step>> {
    let steps = state.with_db(|db| db.add_steps(&note_id, after.as_deref(), &lines))?;
    notes_changed(&app);
    Ok(steps)
}

#[tauri::command]
pub fn step_set_text(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    text: String,
) -> AppResult<Step> {
    let step = state.with_db(|db| db.set_step_text(&id, &text))?;
    notes_changed(&app);
    Ok(step)
}

#[tauri::command]
pub fn step_set_done(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    done: bool,
) -> AppResult<Step> {
    let step = state.with_db(|db| db.set_step_done(&id, done))?;
    notes_changed(&app);
    Ok(step)
}

#[tauri::command]
pub fn step_move(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    after: Option<String>,
) -> AppResult<Step> {
    let step = state.with_db(|db| db.move_step(&id, after.as_deref()))?;
    notes_changed(&app);
    Ok(step)
}

#[tauri::command]
pub fn step_remove(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_db(|db| db.remove_step(&id))?;
    notes_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn step_restore(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<Step> {
    let step = state.with_db(|db| db.restore_step(&id))?;
    notes_changed(&app);
    Ok(step)
}
