//! Looking for a newer release and installing it. The interface asks through
//! these two commands; the updater plugin does the work.

use posthaste_core::UpdateInfo;
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::error::{AppError, AppResult};

/// The updater's own errors name transport details nobody can act on, so
/// each failure is turned into what to do about it.
async fn find(app: &AppHandle) -> AppResult<Option<Update>> {
    let unreachable = || {
        AppError::new(
            "update_unreachable",
            "Could not reach the update server. Check your internet connection and try again.",
        )
    };
    let updater = app.updater().map_err(|e| {
        log::warn!("updater unavailable: {e}");
        unreachable()
    })?;
    updater.check().await.map_err(|e| {
        log::warn!("update check failed: {e}");
        if e.to_string().to_lowercase().contains("release json") {
            AppError::new(
                "update_unpublished",
                "No update information has been published yet. Try again later.",
            )
        } else {
            unreachable()
        }
    })
}

/// Looks for a newer release. `None` means the app is current.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> AppResult<Option<UpdateInfo>> {
    Ok(find(&app).await?.map(|update| UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
    }))
}

/// Downloads and installs the newest release, then restarts the app. Only
/// returns when there was nothing to install or the install failed.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> AppResult<()> {
    let Some(update) = find(&app).await? else {
        return Ok(());
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| {
            log::error!("update install failed: {e}");
            AppError::new(
                "update_failed",
                "The update could not be installed. Try again from Settings → Updates.",
            )
        })?;
    app.restart()
}
