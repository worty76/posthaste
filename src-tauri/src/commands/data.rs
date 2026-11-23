//! Database info, export, import, backup, and full data reset commands.

use posthaste_core::{DataInfo, ExportFormat, ImportReport, Settings, SettingsState};
use posthaste_i18n::key;
use tauri::{AppHandle, Emitter, Manager, State, Window};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::{dialog_answer, looks_changed, notes_changed, settings_update};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[tauri::command]
pub fn data_info(state: State<'_, AppState>) -> AppResult<DataInfo> {
    state.with_db(|db| db.info())
}

#[tauri::command]
pub fn data_reveal(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let path = state.with_db(|db| db.info())?.path;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| AppError::new("io", format!("The folder could not be opened: {e}")))
}

fn stamp() -> String {
    // UTC date for default file names; avoids pulling in a time-zone library.
    let days = posthaste_core::now_ms().div_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 to (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Asks where to save, then writes a Markdown or JSON export. `None` means cancelled.
#[tauri::command]
pub async fn data_export(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    utc_offset_minutes: Option<i32>,
) -> AppResult<Option<usize>> {
    let Some(dest) = dialog_answer(|tx| {
        app.dialog()
            .file()
            .set_parent(&window)
            .set_title(state.translator().text(key::DESKTOP_DIALOG_EXPORT))
            .set_file_name(format!("posthaste-notes-{}.md", stamp()))
            .add_filter("Markdown", &["md"])
            .add_filter("JSON", &["json"])
            .save_file(move |dest| {
                let _ = tx.send(dest);
            });
    }) else {
        return Ok(None);
    };
    let dest = dest
        .into_path()
        .map_err(|e| AppError::new("io", e.to_string()))?;
    let format = export_format(&dest, utc_offset_minutes.unwrap_or(0));
    state.with_db(|db| db.export_to(&dest, format)).map(Some)
}

/// JSON when the chosen name ends in .json; Markdown otherwise.
fn export_format(dest: &std::path::Path, utc_offset_minutes: i32) -> ExportFormat {
    let json = dest
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if json {
        ExportFormat::Json
    } else {
        ExportFormat::Markdown { utc_offset_minutes }
    }
}

#[tauri::command]
pub async fn data_import(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
) -> AppResult<Option<ImportReport>> {
    let Some(src) = dialog_answer(|tx| {
        app.dialog()
            .file()
            .set_parent(&window)
            .set_title(state.translator().text(key::DESKTOP_DIALOG_IMPORT))
            .add_filter(
                state.translator().text(key::DESKTOP_FILTER_EXPORT),
                &["md", "markdown", "json"],
            )
            .pick_file(move |src| {
                let _ = tx.send(src);
            });
    }) else {
        return Ok(None);
    };
    let src = src
        .into_path()
        .map_err(|e| AppError::new("io", e.to_string()))?;
    let report = state.with_db(|db| db.import_from(&src))?;
    notes_changed(&app);
    Ok(Some(report))
}

/// Asks where to save, then writes a full copy of the database.
#[tauri::command]
pub async fn data_backup(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
) -> AppResult<Option<String>> {
    let Some(dest) = dialog_answer(|tx| {
        app.dialog()
            .file()
            .set_parent(&window)
            .set_title(state.translator().text(key::DESKTOP_DIALOG_BACKUP))
            .set_file_name(format!("posthaste-backup-{}.db", stamp()))
            .add_filter(
                state.translator().text(key::DESKTOP_FILTER_DATABASE),
                &["db"],
            )
            .save_file(move |dest| {
                let _ = tx.send(dest);
            });
    }) else {
        return Ok(None);
    };
    let dest = dest
        .into_path()
        .map_err(|e| AppError::new("io", e.to_string()))?;
    state.with_db(|db| db.backup_to(&dest))?;
    Ok(Some(dest.to_string_lossy().into_owned()))
}

/// Deletes all notes, drafts and settings. The frontend asks for explicit
/// confirmation first; `confirm` is a second guard against stray calls.
#[tauri::command]
pub async fn data_clear(app: AppHandle, confirm: String) -> AppResult<SettingsState> {
    if confirm != "DELETE" {
        return Err(AppError::new(
            "not_confirmed",
            "Clearing data was not confirmed.",
        ));
    }
    app.state::<AppState>().with_db(|db| db.clear_all())?;
    log::warn!("all application data cleared at the user's request");
    notes_changed(&app);
    looks_changed(&app);
    if let Some(svc) = app.try_state::<crate::sync::SyncService>() {
        svc.stop().await;
        let _ = app.emit("sync-changed", ());
    }
    // Keys come off the main thread (the keychain may prompt) and never
    // stop the reset: the notes are already gone.
    let keys_app = app.clone();
    let left = tauri::async_runtime::spawn_blocking(move || {
        let secrets = keys_app.state::<crate::ai::SecretsCell>().wait();
        crate::ai::forget_all_keys(&keys_app.state::<AppState>(), &secrets)
    })
    .await
    .unwrap_or(0);
    if left > 0 {
        log::warn!("{left} AI key(s) could not be removed from the keychain");
    }
    // Settings are back to defaults; apply them like any other change.
    settings_update(app.clone(), app.state::<AppState>(), Settings::default())
}

#[cfg(test)]
mod tests {
    use super::{civil_from_days, export_format};
    use posthaste_core::ExportFormat;

    #[test]
    fn the_chosen_file_name_decides_the_export_format() {
        let md = ExportFormat::Markdown {
            utc_offset_minutes: 420,
        };
        assert_eq!(export_format("/x/notes.md".as_ref(), 420), md);
        assert_eq!(export_format("/x/notes".as_ref(), 420), md);
        assert_eq!(
            export_format("/x/notes.JSON".as_ref(), 420),
            ExportFormat::Json
        );
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(20_770), (2026, 11, 13));
    }
}
