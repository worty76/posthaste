//! Look gallery, live theme/scene preview, share codes, and `.phlook` package commands.

use posthaste_core::look::code::{b64_encode_std, encode_code};
use posthaste_core::look::derive::{resolve, ResolvedLook};
use posthaste_core::look::package::{read_package, write_package};
use posthaste_core::look::store::LookSummary;
use posthaste_core::look::{resolve_live, validate_scene_parts, LookDocExt as _};
use posthaste_core::look::{AssetRef, LookDoc, ResolvedScene, SceneEdit, ThemeEdit};
use posthaste_core::LookFilePreview;
use posthaste_i18n::key;
use tauri::{AppHandle, Emitter, State, Window};
use tauri_plugin_dialog::DialogExt;

use super::{dialog_answer, looks_changed};
use crate::error::{AppError, AppResult};
use crate::state::{lock, AppState};

#[tauri::command]
pub fn look_list(state: State<'_, AppState>) -> AppResult<Vec<LookSummary>> {
    state.with_db(|db| db.list_looks())
}

#[tauri::command]
pub fn look_active(state: State<'_, AppState>) -> AppResult<ResolvedLook> {
    state.with_db(|db| db.active_look())
}

#[tauri::command]
pub fn look_theme(state: State<'_, AppState>, id: String) -> AppResult<ThemeEdit> {
    state.with_db(|db| db.look_theme(&id))
}

/// Validates and derives a draft for the editor's live preview. Stores
/// nothing.
#[tauri::command]
pub fn look_preview(theme: ThemeEdit) -> AppResult<ResolvedLook> {
    Ok(resolve("preview", &theme)?)
}

#[tauri::command]
pub fn look_save(
    app: AppHandle,
    state: State<'_, AppState>,
    theme: ThemeEdit,
    scene: Option<SceneEdit>,
    id: Option<String>,
) -> AppResult<LookSummary> {
    let summary = state.with_db(|db| db.save_look(theme, scene, id.as_deref()))?;
    looks_changed(&app);
    Ok(summary)
}

/// The scene of the look the user wears, ready to draw. Empty under
/// Simple looks and for colour-only looks.
#[tauri::command]
pub fn look_active_scene(state: State<'_, AppState>) -> AppResult<ResolvedScene> {
    state.with_db(|db| db.active_scene())
}

/// A look's scene and assets, as the editor edits them.
#[tauri::command]
pub fn look_scene(state: State<'_, AppState>, id: String) -> AppResult<SceneEdit> {
    state.with_db(|db| db.look_scene(&id))
}

/// Checks a scene edit and resolves it for the editor's live preview.
/// Stores nothing.
#[tauri::command]
pub fn look_scene_preview(edit: SceneEdit) -> AppResult<ResolvedScene> {
    validate_scene_parts(&edit.scene, &edit.assets)?;
    Ok(resolve_live(&edit.scene, &edit.assets))
}

/// One look image as a `data:` URL, for `<img>` sources.
#[tauri::command]
pub fn look_asset(state: State<'_, AppState>, id: String) -> AppResult<String> {
    let (mime, bytes) = state.with_db(|db| db.look_asset_bytes(&id))?;
    Ok(format!("data:{mime};base64,{}", b64_encode_std(&bytes)))
}

/// Asks for an image file, checks it, stores it, and answers the
/// document's claim about it. The file's name is never logged or kept.
#[tauri::command]
pub async fn look_asset_pick(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
) -> AppResult<Option<AssetRef>> {
    let Some(src) = dialog_answer(|tx| {
        app.dialog()
            .file()
            .set_parent(&window)
            .set_title(state.translator().text(key::DESKTOP_DIALOG_PICK_IMAGE))
            .add_filter(
                state.translator().text(key::DESKTOP_FILTER_IMAGES),
                &["png", "jpg", "jpeg", "webp"],
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
    let len = std::fs::metadata(&src)
        .map_err(|e| AppError::new("io", e.to_string()))?
        .len();
    if len > posthaste_core::look::assets::MAX_ASSET_BYTES as u64 {
        let size = format!("{:.1}", len as f64 / (1024.0 * 1024.0));
        return Err(AppError::new(
            "asset_invalid",
            state
                .translator()
                .format(key::DESKTOP_ERROR_IMAGE_TOO_BIG, &[("size", size.as_str())]),
        ));
    }
    let bytes = std::fs::read(&src).map_err(|e| AppError::new("io", e.to_string()))?;
    let asset = state.with_db(|db| db.store_look_asset(&bytes))?;
    Ok(Some(asset))
}

#[tauri::command]
pub fn look_import(
    app: AppHandle,
    state: State<'_, AppState>,
    code: String,
) -> AppResult<LookSummary> {
    let summary = state.with_db(|db| db.import_look(&code))?;
    looks_changed(&app);
    Ok(summary)
}

#[tauri::command]
pub fn look_export_code(state: State<'_, AppState>, id: String) -> AppResult<String> {
    // The document as stored: a shared look keeps its scene and every
    // field this build does not model.
    let doc = state.with_db(|db| db.look_document(&id))?;
    if look_has_images(&doc) {
        // A code carries no image bytes, so a look that lists images
        // would arrive with a backdrop that never draws.
        return Err(AppError::new(
            "look_shared_as_file",
            state
                .translator()
                .text(key::LOOKS_SHARE_HAS_IMAGES)
                .to_string(),
        ));
    }
    Ok(encode_code(&doc))
}

/// True when the document lists look images, which a share code cannot
/// carry.
fn look_has_images(doc: &LookDoc) -> bool {
    !doc.assets.is_empty()
}

/// A look's name as a file's base name: separators out, never empty.
fn file_name_for(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == ':' {
                '-'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "look".into()
    } else {
        trimmed.chars().take(60).collect()
    }
}

/// Asks where to save, then writes the look and its images as one
/// `.phlook` file. `None` means the dialog was cancelled.
#[tauri::command]
pub async fn look_file_export(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Option<String>> {
    let doc = state.with_db(|db| db.look_document(&id))?;
    let name = doc.name.clone();
    // Every listed image must be in storage: write_package refuses a
    // dangling reference, and its error is what the UI shows.
    let file = state.with_db(|db| {
        write_package(&doc, |asset_id| {
            let (_, bytes) = db.look_asset_bytes(asset_id)?;
            Ok(bytes)
        })
    })?;

    let Some(dest) = dialog_answer(|tx| {
        app.dialog()
            .file()
            .set_parent(&window)
            .set_title(state.translator().text(key::DESKTOP_DIALOG_EXPORT_LOOK))
            .set_file_name(format!("{}.phlook", file_name_for(&name)))
            .add_filter(
                state.translator().text(key::DESKTOP_FILTER_LOOKS),
                &["phlook"],
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
    std::fs::write(&dest, file).map_err(|e| AppError::new("io", e.to_string()))?;
    Ok(Some(dest.to_string_lossy().to_string()))
}

/// Asks for a look file, checks every byte of it, and holds what passed
/// for the user's OK. Nothing touches the database until
/// `look_file_install`. The preview travels in the answer, images as
/// `data:` URLs, so nothing needs to be stored to show it.
#[tauri::command]
pub async fn look_file_open(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
) -> AppResult<Option<LookFilePreview>> {
    let Some(src) = dialog_answer(|tx| {
        app.dialog()
            .file()
            .set_parent(&window)
            .set_title(state.translator().text(key::DESKTOP_DIALOG_OPEN_LOOK))
            .add_filter(
                state.translator().text(key::DESKTOP_FILTER_LOOKS),
                &["phlook"],
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
    let text = std::fs::read_to_string(&src).map_err(|e| AppError::new("io", e.to_string()))?;
    let pending = read_package(&text)?;
    pending.doc.validate()?;

    let doc = pending.doc.clone();
    let theme = doc.theme_edit();
    let summary = posthaste_core::look::store::LookSummary {
        id: String::new(),
        name: theme.name.clone(),
        author: theme.author.clone(),
        builtin: false,
        base: theme.base,
        preview: doc.preview(),
        font_family: theme.font_family.clone(),
    };
    let tokens = resolve("file:preview", &theme)?;
    let scene = resolve_live(&doc.scene, &doc.assets);
    let asset_urls = pending
        .assets
        .iter()
        .map(|(asset, bytes)| {
            (
                asset.id.clone(),
                format!("data:{};base64,{}", asset.asset_type, b64_encode_std(bytes)),
            )
        })
        .collect();

    *lock(&state.pending_look) = Some(pending);
    Ok(Some(LookFilePreview {
        summary,
        tokens,
        scene,
        asset_urls,
    }))
}

/// Installs the held look file. The answer is the stored look's summary.
#[tauri::command]
pub fn look_file_install(app: AppHandle, state: State<'_, AppState>) -> AppResult<LookSummary> {
    let pending = lock(&state.pending_look)
        .take()
        .ok_or_else(|| AppError::new("file_install", "There is no look file to add."))?;
    let summary = state.with_db(|db| db.install_package(pending))?;
    looks_changed(&app);
    Ok(summary)
}

/// Throws the held look file away, on Cancel.
#[tauri::command]
pub fn look_file_discard(state: State<'_, AppState>) -> AppResult<()> {
    *lock(&state.pending_look) = None;
    Ok(())
}

#[tauri::command]
pub fn look_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    // The store resets a deleted selected look to the factory default in
    // the same transaction as the delete, so no window is left wearing
    // the deleted look's tokens. The cached settings follow, and every
    // window hears the change like any other.
    if state.with_db(|db| db.delete_look(&id))? {
        let settings = state.with_db(|db| db.load_settings())?;
        *state.settings.write().unwrap_or_else(|e| e.into_inner()) = settings.clone();
        let _ = app.emit("settings-changed", crate::ai::for_interface(&settings));
    }
    looks_changed(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::look_has_images;
    use posthaste_i18n::key;

    #[test]
    fn a_look_with_images_cannot_travel_as_a_code() {
        let mut doc = posthaste_core::look::LookDoc {
            format: "posthaste-look".into(),
            version: 2,
            name: "With image".into(),
            author: None,
            theme: posthaste_core::look::LookTheme {
                base: posthaste_core::look::Base::Dark,
                colors: posthaste_core::look::Colors {
                    bg: "#120d18".into(),
                    surface: "#1d1526".into(),
                    text: "#f3e9f7".into(),
                    accent: "#ff7eb6".into(),
                    ..Default::default()
                },
                shape: Default::default(),
                font: Default::default(),
                font_family: None,
                extra: Default::default(),
            },
            scene: Default::default(),
            assets: vec![posthaste_core::look::AssetRef {
                id: format!("sha256:{}", "a".repeat(64)),
                asset_type: "image/png".into(),
                bytes: 33,
                width: 2,
                height: 3,
                extra: Default::default(),
            }],
            extra: Default::default(),
        };
        assert!(look_has_images(&doc));
        doc.assets.clear();
        assert!(!look_has_images(&doc));
    }

    #[test]
    fn the_image_too_big_error_names_the_size() {
        let t = posthaste_i18n::Translator::new("en");
        let message = t.format(key::DESKTOP_ERROR_IMAGE_TOO_BIG, &[("size", "3.2")]);
        assert!(message.contains("3.2"), "got: {message}");
        let t = posthaste_i18n::Translator::new("vi");
        let message = t.format(key::DESKTOP_ERROR_IMAGE_TOO_BIG, &[("size", "3.2")]);
        assert!(message.contains("3.2"), "got: {message}");
    }
}
