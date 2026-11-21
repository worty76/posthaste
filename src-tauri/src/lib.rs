//! Posthaste desktop shell: wires storage, windows, shortcuts and the tray
//! together. Business rules live in `posthaste-core`.

mod ai;
mod commands;
mod error;
mod panels;
mod shortcuts;
mod state;
mod sync;
mod tray;
mod updates;
mod windows;

use std::sync::atomic::Ordering;

use posthaste_core::Db;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::MacosLauncher;

use crate::error::AppError;
use crate::state::{lock, AppState};

/// What a launch (or a second launch forwarded to the running instance)
/// asked for on the command line.
#[derive(Debug, PartialEq, Eq)]
enum Launch {
    Main,
    Capture,
    Today,
    View(&'static str),
    Background,
}

fn parse_launch(args: &[String]) -> Launch {
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--capture") {
        Launch::Capture
    } else if has("--today") {
        Launch::Today
    } else if has("--inbox") {
        Launch::View("inbox")
    } else if has("--settings") {
        Launch::View("settings")
    } else if has("--autostart") || has("--minimized") {
        Launch::Background
    } else {
        Launch::Main
    }
}

fn perform(app: &AppHandle, launch: Launch) {
    match launch {
        Launch::Main => windows::show_main(app, None),
        Launch::Capture => windows::show_capture(app),
        Launch::Today => windows::show_today(app),
        Launch::View(view) => windows::show_main(app, Some(view)),
        Launch::Background => {}
    }
}

/// Database file name: debug/dev builds use a separate file so testing new
/// migrations never upgrades the installed release app's database.
fn database_filename(is_debug: bool) -> &'static str {
    if is_debug {
        "posthaste-dev.db"
    } else {
        "posthaste.db"
    }
}

fn open_database(app: &AppHandle) -> Result<Db, AppError> {
    let dir = app.path().app_data_dir().map_err(|e| {
        AppError::new(
            "db_open",
            format!("The application data folder could not be determined: {e}"),
        )
    })?;
    let mut db = Db::open(&dir.join(database_filename(cfg!(debug_assertions))))?;
    let cutoff = posthaste_core::now_ms() - posthaste_core::TRASH_RETENTION_MS;
    match db.purge_trashed(cutoff) {
        Ok(0) => {}
        Ok(n) => log::info!("emptied {n} old note(s) from the trash"),
        Err(e) => log::warn!("trash could not be emptied: {e}"),
    }
    match db.purge_unused_look_assets(posthaste_core::now_ms()) {
        Ok(0) => {}
        Ok(n) => log::info!("purged {n} unused look image(s)"),
        Err(e) => log::warn!("look images could not be purged: {e}"),
    }
    Ok(db)
}

pub fn run() {
    let app = tauri::Builder::default()
        // Must be first: a second launch hands its arguments to the running
        // instance and exits, so there is never more than one writer.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let launch = match parse_launch(&args) {
                // Launching the app again always surfaces something.
                Launch::Background => Launch::Main,
                other => other,
            };
            perform(app, launch);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .on_page_load(|webview, page| {
            if tracing() {
                log::info!(
                    "trace: {} page {:?} {}",
                    webview.label(),
                    page.event(),
                    page.url()
                );
            }
        })
        .invoke_handler(traced(tauri::generate_handler![
            commands::app_info,
            commands::capture_save,
            commands::capture_dismiss,
            commands::capture_open,
            commands::draft_get,
            commands::draft_set,
            commands::draft_clear,
            commands::notes_list,
            commands::notes_counts,
            commands::projects_list,
            commands::project_create,
            commands::project_rename,
            commands::project_set_archived,
            commands::project_set_color,
            commands::project_clear,
            commands::note_set_project,
            commands::steps_add,
            commands::step_set_text,
            commands::step_set_done,
            commands::step_move,
            commands::step_remove,
            commands::step_restore,
            commands::glance_get,
            commands::notes_day_marks,
            commands::inbox_open_date,
            commands::today_dismiss,
            commands::note_update,
            commands::note_set_status,
            commands::note_set_later,
            commands::note_delete,
            commands::note_undelete,
            commands::trash_list,
            commands::trash_purge,
            commands::trash_empty,
            commands::details_open,
            commands::details_dismiss,
            commands::details_current,
            commands::note_detail_get,
            commands::note_details_set,
            commands::attachment_add_files,
            commands::attachment_add_paths,
            commands::attachment_add_bytes,
            commands::attachment_preview,
            commands::attachment_open,
            commands::attachment_save_copy,
            commands::attachment_remove,
            commands::settings_get,
            commands::settings_update,
            commands::shortcuts_set_paused,
            commands::data_info,
            commands::data_reveal,
            commands::data_export,
            commands::data_import,
            commands::data_backup,
            commands::data_clear,
            commands::look_list,
            commands::look_active,
            commands::look_theme,
            commands::look_preview,
            commands::look_save,
            commands::look_active_scene,
            commands::look_scene,
            commands::look_scene_preview,
            commands::look_asset,
            commands::look_asset_pick,
            commands::look_import,
            commands::look_file_export,
            commands::look_file_open,
            commands::look_file_install,
            commands::look_file_discard,
            commands::look_export_code,
            commands::look_delete,
            ai::ai_polish,
            ai::ai_providers,
            ai::ai_key_set,
            ai::ai_key_clear,
            ai::ai_test,
            ai::ai_key_page,
            updates::update_check,
            updates::update_install,
            commands::sync_status,
            commands::sync_set_enabled,
            commands::sync_pair_offer,
            commands::sync_pair_join,
            commands::sync_now,
            commands::sync_forget,
        ]))
        .setup(|app| {
            let handle = app.handle().clone();

            let db = open_database(&handle);
            let db_failed = db.is_err();
            app.manage(AppState::new(db));
            app.manage(ai::SecretsCell::default());
            app.manage(sync::SyncService::default());
            // The keychain may prompt or wait on D-Bus: set it up off the
            // main thread. Saving settings meanwhile keeps any old key.
            ai::start(&handle);
            sync::SyncService::start_if_enabled(&handle);
            let state = handle.state::<AppState>();
            let settings = state.settings();

            windows::create(&handle)?;

            // Copies written when an attachment was opened last session.
            if let Ok(dir) = commands::opened_dir(&handle) {
                let _ = std::fs::remove_dir_all(dir);
            }

            match tray::create(&handle, state.translator()) {
                Ok(menu) => {
                    handle.manage(menu);
                    state.tray_available.store(true, Ordering::Relaxed);
                }
                Err(e) => log::warn!("system tray unavailable, continuing without it: {e}"),
            }

            let report = shortcuts::apply(&handle, &settings);
            let shortcut_failed = !report.capture.registered;
            *lock(&state.shortcuts) = report;

            let args: Vec<String> = std::env::args().collect();
            let launch = match parse_launch(&args) {
                Launch::Main if settings.start_minimized => Launch::Background,
                other => other,
            };
            // Problems the user has to know about override "start minimized":
            // a silent launch with a dead shortcut would look like it works.
            if launch == Launch::Background && (db_failed || shortcut_failed) {
                windows::show_main(&handle, None);
            } else {
                perform(&handle, launch);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Posthaste could not start");

    app.run(|app, event| {
        // Clicking the Dock icon while no window is open brings the inbox back.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            windows::show_main(app, None);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}

/// Whether `PH_TRACE` is set: then every command a window runs and every
/// page load is logged, to see what a window did and in which order.
fn tracing() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("PH_TRACE").is_some())
}

/// Wraps the command handler so `PH_TRACE` can log each call.
fn traced(
    handler: impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    move |invoke| {
        if tracing() {
            log::info!(
                "trace: {} ran {}",
                invoke.message.webview().label(),
                invoke.message.command()
            );
        }
        handler(invoke)
    }
}

#[cfg(test)]
mod tests {
    use super::{database_filename, parse_launch, Launch};

    #[test]
    fn debug_builds_use_a_separate_database_file() {
        assert_eq!(database_filename(true), "posthaste-dev.db");
        assert_eq!(database_filename(false), "posthaste.db");
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn launch_flags() {
        assert_eq!(parse_launch(&args(&["posthaste"])), Launch::Main);
        assert_eq!(
            parse_launch(&args(&["posthaste", "--capture"])),
            Launch::Capture
        );
        assert_eq!(
            parse_launch(&args(&["posthaste", "--today"])),
            Launch::Today
        );
        assert_eq!(
            parse_launch(&args(&["posthaste", "--inbox"])),
            Launch::View("inbox")
        );
        assert_eq!(
            parse_launch(&args(&["posthaste", "--settings"])),
            Launch::View("settings")
        );
        assert_eq!(
            parse_launch(&args(&["posthaste", "--autostart"])),
            Launch::Background
        );
    }
}
