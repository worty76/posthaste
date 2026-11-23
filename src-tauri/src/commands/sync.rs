//! Peer-to-peer device pairing and sync status commands.

use tauri::{AppHandle, State};

use crate::error::AppResult;

#[tauri::command]
pub async fn sync_status(
    app: AppHandle,
    sync: State<'_, crate::sync::SyncService>,
) -> AppResult<posthaste_core::SyncStatus> {
    sync.status(&app).await
}

#[tauri::command]
pub async fn sync_set_enabled(
    app: AppHandle,
    sync: State<'_, crate::sync::SyncService>,
    on: bool,
) -> AppResult<posthaste_core::SyncStatus> {
    sync.set_enabled(&app, on).await
}

#[tauri::command]
pub async fn sync_pair_offer(
    app: AppHandle,
    sync: State<'_, crate::sync::SyncService>,
) -> AppResult<String> {
    sync.create_pair_offer(&app).await
}

#[tauri::command]
pub async fn sync_pair_join(
    app: AppHandle,
    sync: State<'_, crate::sync::SyncService>,
    code: String,
) -> AppResult<posthaste_core::SyncDevice> {
    sync.join_pair_code(&app, &code).await
}

#[tauri::command]
pub async fn sync_now(app: AppHandle, sync: State<'_, crate::sync::SyncService>) -> AppResult<()> {
    sync.sync_now(&app).await
}

#[tauri::command]
pub async fn sync_forget(
    app: AppHandle,
    sync: State<'_, crate::sync::SyncService>,
    id: String,
) -> AppResult<()> {
    sync.forget(&app, &id).await
}
