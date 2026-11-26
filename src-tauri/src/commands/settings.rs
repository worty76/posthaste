//! User settings, autostart, and global shortcut reconfiguration commands.

use std::sync::atomic::Ordering;

use posthaste_core::{Settings, SettingsState};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as _;

use crate::error::{AppError, AppResult};
use crate::shortcuts;
use crate::state::{lock, AppState};

#[tauri::command]
pub fn shortcuts_set_paused(app: AppHandle, state: State<'_, AppState>, paused: bool) {
    if paused {
        shortcuts::pause(&app);
    } else {
        let settings = state.settings();
        *lock(&state.shortcuts) = shortcuts::apply(&app, &settings);
    }
}

#[tauri::command]
pub fn settings_get(app: AppHandle, state: State<'_, AppState>) -> SettingsState {
    let mut settings = state.settings();
    // The OS is the source of truth: the user may have removed the login
    // item themselves.
    if let Ok(enabled) = app.autolaunch().is_enabled() {
        settings.launch_at_login = enabled;
    }
    SettingsState {
        settings: crate::ai::for_interface(&settings),
        shortcuts: lock(&state.shortcuts).clone(),
    }
}

#[tauri::command]
pub fn settings_update(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> AppResult<SettingsState> {
    let old = state.settings();
    let settings = crate::ai::keep_legacy_key(settings, &old);

    if settings.launch_at_login != app.autolaunch().is_enabled().unwrap_or(old.launch_at_login) {
        let result = if settings.launch_at_login {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
        result.map_err(|e| {
            AppError::new(
                "autostart",
                format!("Launch at login could not be changed: {e}"),
            )
        })?;
    }

    let shortcuts_changed = settings.capture_shortcut != old.capture_shortcut
        || settings.inbox_shortcut != old.inbox_shortcut
        || settings.today_shortcut != old.today_shortcut;
    if shortcuts_changed {
        let report = shortcuts::apply(&app, &settings);
        let failed = [
            (&report.capture, &old.capture_shortcut),
            (&report.inbox, &old.inbox_shortcut),
            (&report.today, &old.today_shortcut),
        ]
        .into_iter()
        .find(|(s, previous)| {
            !s.registered && !s.accelerator.is_empty() && s.accelerator != **previous
        })
        .and_then(|(s, _)| s.error.clone());
        if let Some(message) = failed {
            // Put the working shortcuts back rather than leave the user
            // with none.
            *lock(&state.shortcuts) = shortcuts::apply(&app, &old);
            return Err(AppError::new("shortcut_unavailable", message));
        }
        *lock(&state.shortcuts) = report;
    }

    state.with_db(|db| db.save_settings(&settings))?;
    *state.settings.write().unwrap_or_else(|e| e.into_inner()) = settings.clone();
    if settings.window_size != old.window_size {
        // The details window keeps the size it was given; place it afresh
        // next time so the new size shows.
        state.details_placed.store(false, Ordering::Relaxed);
    }
    if settings.macos_glass != old.macos_glass || settings.look_id != old.look_id {
        crate::windows::sync_glass(&app);
    }
    if settings.language != old.language {
        // No tray on this system means no menu to rename.
        if let Some(menu) = app.try_state::<crate::tray::TrayMenu>() {
            menu.translate(state.translator());
        }
    }
    if settings.sync_relay_url != old.sync_relay_url || settings.sync_enabled != old.sync_enabled {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Some(svc) = handle.try_state::<crate::sync::SyncService>() {
                let _ = svc.reconfigure(&handle).await;
            }
        });
    }
    let _ = app.emit("settings-changed", crate::ai::for_interface(&settings));

    Ok(SettingsState {
        settings,
        shortcuts: lock(&state.shortcuts).clone(),
    })
}
