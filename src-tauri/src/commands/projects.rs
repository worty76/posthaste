//! Project creation, renaming, color, archiving, and note assignment commands.

use posthaste_core::{Note, Project};
use tauri::{AppHandle, State};

use super::notes_changed;
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub fn projects_list(
    state: State<'_, AppState>,
    include_archived: bool,
) -> AppResult<Vec<Project>> {
    state.with_db(|db| db.projects(include_archived))
}

#[tauri::command]
pub fn project_create(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> AppResult<Project> {
    let project = state.with_db(|db| db.create_project(&name))?;
    notes_changed(&app);
    Ok(project)
}

#[tauri::command]
pub fn project_rename(
    app: AppHandle,
    state: State<'_, AppState>,
    from: String,
    to: String,
    merge: bool,
) -> AppResult<u32> {
    let moved = state.with_db(|db| db.rename_project(&from, &to, merge))?;
    notes_changed(&app);
    Ok(moved)
}

#[tauri::command]
pub fn project_set_archived(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    archived: bool,
) -> AppResult<()> {
    state.with_db(|db| db.set_project_archived(&name, archived))?;
    notes_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn project_set_color(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    color: Option<String>,
) -> AppResult<()> {
    state.with_db(|db| db.set_project_color(&name, color.as_deref()))?;
    notes_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn project_clear(app: AppHandle, state: State<'_, AppState>, name: String) -> AppResult<u32> {
    let cleared = state.with_db(|db| db.clear_project(&name))?;
    notes_changed(&app);
    Ok(cleared)
}

#[tauri::command]
pub fn note_set_project(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    project: Option<String>,
) -> AppResult<Note> {
    let note = state.with_db(|db| db.set_note_project(&id, project.as_deref()))?;
    notes_changed(&app);
    Ok(note)
}
