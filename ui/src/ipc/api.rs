//! One typed function per shell command. Adding a command to a window means
//! adding it here, next to its argument struct; the names and shapes match
//! `src-tauri/src/commands.rs`.

use posthaste_model::{
    AiOverview, AppInfo, AssetRef, AttachReport, Attachment, CommandError, Counts, DataInfo,
    DayMark, DetailsFrom, DetailsTarget, Draft, Glance, ImportReport, LookFilePreview, LookSummary,
    Note, NoteDetail, NoteQuery, ResolvedLook, ResolvedScene, SceneEdit, Settings, SettingsState,
    Status, SyncDevice, SyncStatus, ThemeEdit, TrashedNote, UpdateInfo,
};
use serde::de::IgnoredAny;
use serde::Serialize;

use super::tauri::{invoke, invoke_for_bytes, invoke_with_bytes, NoArgs};

pub type Response<T> = Result<T, CommandError>;

pub async fn app_info() -> Response<AppInfo> {
    invoke("app_info", &NoArgs {}).await
}

#[derive(Serialize)]
struct ShortcutsPaused {
    paused: bool,
}

/// Temporarily pauses or resumes OS global shortcuts while recording one in Settings.
pub async fn shortcuts_set_paused(paused: bool) -> Response<()> {
    invoke("shortcuts_set_paused", &ShortcutsPaused { paused }).await
}

pub async fn settings_get() -> Response<SettingsState> {
    invoke("settings_get", &NoArgs {}).await
}

/// The look the user wears. There is always one: the store falls back to
/// the factory default when a selection cannot resolve.
pub async fn look_active() -> Response<ResolvedLook> {
    invoke("look_active", &NoArgs {}).await
}

pub async fn categories_list() -> Response<Vec<String>> {
    invoke("categories_list", &NoArgs {}).await
}

#[derive(Serialize)]
struct CaptureSave<'a> {
    content: &'a str,
    category: Option<&'a str>,
    hide: bool,
}

/// Files the note. Resolves only after the database has committed it.
/// The shell answers with the note, which the capture window has no use
/// for; it is not decoded, so a change to its shape can never make a
/// committed note look unsaved.
pub async fn capture_save(content: &str, category: Option<&str>, hide: bool) -> Response<()> {
    let args = CaptureSave {
        content,
        category,
        hide,
    };
    invoke::<_, IgnoredAny>("capture_save", &args)
        .await
        .map(drop)
}

pub async fn capture_dismiss() -> Response<()> {
    invoke("capture_dismiss", &NoArgs {}).await
}

pub async fn draft_get() -> Response<Option<Draft>> {
    invoke("draft_get", &NoArgs {}).await
}

#[derive(Serialize)]
struct DraftSet<'a> {
    content: &'a str,
    category: Option<&'a str>,
}

pub async fn draft_set(content: &str, category: Option<&str>) -> Response<()> {
    invoke("draft_set", &DraftSet { content, category }).await
}

pub async fn draft_clear() -> Response<()> {
    invoke("draft_clear", &NoArgs {}).await
}

#[derive(Serialize)]
struct AiPolish<'a> {
    text: &'a str,
}

pub async fn ai_polish(text: &str) -> Response<String> {
    invoke("ai_polish", &AiPolish { text }).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DayBounds {
    today_from: i64,
    today_to: i64,
}

/// What the Today window shows for the day between these instants.
pub async fn glance_get(from: i64, to: i64) -> Response<Glance> {
    let args = DayBounds {
        today_from: from,
        today_to: to,
    };
    invoke("glance_get", &args).await
}

pub async fn today_dismiss() -> Response<()> {
    invoke("today_dismiss", &NoArgs {}).await
}

#[derive(Serialize)]
struct Edges<'a> {
    edges: &'a [i64],
}

/// Note counts for each day between consecutive `edges`: n edges, n-1 days.
pub async fn notes_day_marks(edges: &[i64]) -> Response<Vec<DayMark>> {
    invoke("notes_day_marks", &Edges { edges }).await
}

#[derive(Serialize)]
struct InboxDate<'a> {
    day: &'a str,
}

/// Opens the inbox filtered to one day, written `YYYY-MM-DD`.
pub async fn inbox_open_date(day: &str) -> Response<()> {
    invoke("inbox_open_date", &InboxDate { day }).await
}

#[derive(Serialize)]
struct SetStatus<'a> {
    id: &'a str,
    status: Status,
}

pub async fn note_set_status(id: &str, status: Status) -> Response<()> {
    invoke::<_, IgnoredAny>("note_set_status", &SetStatus { id, status })
        .await
        .map(drop)
}

#[derive(Serialize)]
struct SetLater<'a> {
    id: &'a str,
    later: bool,
}

pub async fn note_set_later(id: &str, later: bool) -> Response<()> {
    invoke::<_, IgnoredAny>("note_set_later", &SetLater { id, later })
        .await
        .map(drop)
}

#[derive(Serialize)]
struct DetailsOpen<'a> {
    id: &'a str,
    from: DetailsFrom,
}

/// Opens Note details on a note. `from` is the window Esc goes back to.
pub async fn details_open(id: &str, from: DetailsFrom) -> Response<()> {
    invoke("details_open", &DetailsOpen { id, from }).await
}

pub async fn details_dismiss() -> Response<()> {
    invoke("details_dismiss", &NoArgs {}).await
}

/// The note the details window should show, for when it loads after the
/// `details-open` event was sent.
pub async fn details_current() -> Response<Option<DetailsTarget>> {
    invoke("details_current", &NoArgs {}).await
}

#[derive(Serialize)]
struct ById<'a> {
    id: &'a str,
}

pub async fn note_detail_get(id: &str) -> Response<NoteDetail> {
    invoke("note_detail_get", &ById { id }).await
}

#[derive(Serialize)]
struct DetailsSet<'a> {
    id: &'a str,
    details: &'a str,
}

pub async fn note_details_set(id: &str, details: &str) -> Response<()> {
    invoke::<_, IgnoredAny>("note_details_set", &DetailsSet { id, details })
        .await
        .map(drop)
}

#[derive(Serialize)]
struct NoteUpdate<'a> {
    id: &'a str,
    content: &'a str,
    category: Option<&'a str>,
}

pub async fn note_update(id: &str, content: &str, category: Option<&str>) -> Response<()> {
    let args = NoteUpdate {
        id,
        content,
        category,
    };
    invoke::<_, IgnoredAny>("note_update", &args)
        .await
        .map(drop)
}

// ---- attachments -----------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ForNote<'a> {
    note_id: &'a str,
}

/// Lets the user pick files to attach. `None` when they cancelled.
pub async fn attachment_add_files(note_id: &str) -> Response<Option<AttachReport>> {
    invoke("attachment_add_files", &ForNote { note_id }).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddPaths<'a> {
    note_id: &'a str,
    paths: &'a [String],
}

/// Attaches files by their paths on this computer (a drop).
pub async fn attachment_add_paths(note_id: &str, paths: &[String]) -> Response<AttachReport> {
    invoke("attachment_add_paths", &AddPaths { note_id, paths }).await
}

/// Attaches bytes that have no path (a pasted image).
pub async fn attachment_add_bytes(
    note_id: &str,
    name: &str,
    mime: &str,
    bytes: &[u8],
) -> Response<Attachment> {
    let headers = [("x-note-id", note_id), ("x-name", name), ("x-mime", mime)];
    invoke_with_bytes("attachment_add_bytes", bytes, &headers).await
}

/// The bytes of an image attachment, to show as its thumbnail.
pub async fn attachment_preview(id: &str) -> Response<Vec<u8>> {
    invoke_for_bytes("attachment_preview", &ById { id }).await
}

pub async fn attachment_open(id: &str) -> Response<()> {
    invoke("attachment_open", &ById { id }).await
}

/// Asks where to save a copy. `None` when the user cancelled.
pub async fn attachment_save_copy(id: &str) -> Response<Option<String>> {
    invoke("attachment_save_copy", &ById { id }).await
}

pub async fn attachment_remove(id: &str) -> Response<()> {
    invoke("attachment_remove", &ById { id }).await
}

// ---- inbox and trash -------------------------------------------------------

pub async fn capture_open() -> Response<()> {
    invoke("capture_open", &NoArgs {}).await
}

#[derive(Serialize)]
struct NotesList<'a> {
    query: &'a NoteQuery,
}

/// Notes matching `query`, newest first.
pub async fn notes_list(query: &NoteQuery) -> Response<Vec<Note>> {
    invoke("notes_list", &NotesList { query }).await
}

/// How many notes each section holds; `from` and `to` bound today.
pub async fn notes_counts(from: i64, to: i64) -> Response<Counts> {
    let args = DayBounds {
        today_from: from,
        today_to: to,
    };
    invoke("notes_counts", &args).await
}

/// Moves a note to the trash.
pub async fn note_delete(id: &str) -> Response<()> {
    invoke("note_delete", &ById { id }).await
}

/// Takes a note back out of the trash.
pub async fn note_undelete(id: &str) -> Response<()> {
    invoke::<_, IgnoredAny>("note_undelete", &ById { id })
        .await
        .map(drop)
}

pub async fn trash_list() -> Response<Vec<TrashedNote>> {
    invoke("trash_list", &NoArgs {}).await
}

/// Erases one trashed note for good.
pub async fn trash_purge(id: &str) -> Response<()> {
    invoke("trash_purge", &ById { id }).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrashEmpty {
    up_to: i64,
}

/// Erases every note trashed at or before `up_to`. Answers how many.
pub async fn trash_empty(up_to: i64) -> Response<usize> {
    invoke("trash_empty", &TrashEmpty { up_to }).await
}

// ---- settings and data -----------------------------------------------------

#[derive(Serialize)]
struct SettingsUpdate<'a> {
    settings: &'a Settings,
}

/// Saves the settings. Answers with what was applied, which may differ
/// (a shortcut the system refused, for one).
pub async fn settings_update(settings: &Settings) -> Response<SettingsState> {
    invoke("settings_update", &SettingsUpdate { settings }).await
}

pub async fn data_info() -> Response<DataInfo> {
    invoke("data_info", &NoArgs {}).await
}

pub async fn data_reveal() -> Response<()> {
    invoke("data_reveal", &NoArgs {}).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DataExport {
    utc_offset_minutes: i32,
}

/// Asks where to export to. Answers how many notes were written, or `None`
/// when the user cancelled. The offset dates the notes in local time.
pub async fn data_export(utc_offset_minutes: i32) -> Response<Option<usize>> {
    invoke("data_export", &DataExport { utc_offset_minutes }).await
}

pub async fn data_import() -> Response<Option<ImportReport>> {
    invoke("data_import", &NoArgs {}).await
}

/// Asks where to put a backup. `None` when the user cancelled.
pub async fn data_backup() -> Response<Option<String>> {
    invoke("data_backup", &NoArgs {}).await
}

#[derive(Serialize)]
struct DataClear {
    confirm: &'static str,
}

/// Deletes every note, draft, look and setting.
pub async fn data_clear() -> Response<SettingsState> {
    // The shell refuses the command without this exact word.
    invoke("data_clear", &DataClear { confirm: "DELETE" }).await
}

// ---- looks -----------------------------------------------------------------

pub async fn look_list() -> Response<Vec<LookSummary>> {
    invoke("look_list", &NoArgs {}).await
}

pub async fn look_theme(id: &str) -> Response<ThemeEdit> {
    invoke("look_theme", &ById { id }).await
}

#[derive(Serialize)]
struct LookPreview<'a> {
    theme: &'a ThemeEdit,
}

/// Checks a look being edited and answers with its tokens.
pub async fn look_preview(theme: &ThemeEdit) -> Response<ResolvedLook> {
    invoke("look_preview", &LookPreview { theme }).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LookSave<'a> {
    theme: &'a ThemeEdit,
    scene: Option<&'a SceneEdit>,
    id: Option<&'a str>,
}

/// Saves a look: over the one with `id`, or as a new one. The scene edit
/// replaces the look's scene and assets; `None` leaves them as they are.
pub async fn look_save(
    theme: &ThemeEdit,
    scene: Option<&SceneEdit>,
    id: Option<&str>,
) -> Response<LookSummary> {
    invoke("look_save", &LookSave { theme, scene, id }).await
}

#[derive(Serialize)]
struct LookImport<'a> {
    code: &'a str,
}

pub async fn look_import(code: &str) -> Response<LookSummary> {
    invoke("look_import", &LookImport { code }).await
}

pub async fn look_export_code(id: &str) -> Response<String> {
    invoke("look_export_code", &ById { id }).await
}

/// The scene of the look the user wears, ready to draw. Empty under
/// Simple looks, for Plain and for colour-only looks.
// The scene host draws from these; they are wired in its commit.
pub async fn look_active_scene() -> Response<ResolvedScene> {
    invoke("look_active_scene", &NoArgs {}).await
}

/// A look's scene and assets, as the editor edits them.
pub async fn look_scene(id: &str) -> Response<SceneEdit> {
    invoke("look_scene", &ById { id }).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScenePreview<'a> {
    edit: &'a SceneEdit,
}

/// Checks a scene edit and resolves it for the editor's live preview.
pub async fn look_scene_preview(edit: &SceneEdit) -> Response<ResolvedScene> {
    invoke("look_scene_preview", &ScenePreview { edit }).await
}

/// One look image as a `data:` URL, for `<img>` sources.
pub async fn look_asset(id: &str) -> Response<String> {
    invoke("look_asset", &ById { id }).await
}

/// Asks for an image file and stores it. `None` means the dialog was
/// cancelled.
pub async fn look_asset_pick() -> Response<Option<AssetRef>> {
    invoke("look_asset_pick", &NoArgs {}).await
}

/// Asks where to save, then writes the look and its images as one
/// `.phlook` file. `None` means the dialog was cancelled.
pub async fn look_file_export(id: &str) -> Response<Option<String>> {
    invoke("look_file_export", &ById { id }).await
}

/// Asks for a look file, checks it, and holds it for the user's OK.
/// `None` means the dialog was cancelled.
pub async fn look_file_open() -> Response<Option<LookFilePreview>> {
    invoke("look_file_open", &NoArgs {}).await
}

/// Installs the held look file.
pub async fn look_file_install() -> Response<LookSummary> {
    invoke("look_file_install", &NoArgs {}).await
}

/// Throws the held look file away.
pub async fn look_file_discard() -> Response<()> {
    invoke("look_file_discard", &NoArgs {}).await
}

pub async fn look_delete(id: &str) -> Response<()> {
    invoke("look_delete", &ById { id }).await
}

// ---- AI polish -------------------------------------------------------------

pub async fn ai_providers() -> Response<AiOverview> {
    invoke("ai_providers", &NoArgs {}).await
}

#[derive(Serialize)]
struct ForProvider<'a> {
    provider: &'a str,
}

#[derive(Serialize)]
struct KeySet<'a> {
    provider: &'a str,
    key: &'a str,
}

pub async fn ai_key_set(provider: &str, key: &str) -> Response<AiOverview> {
    invoke("ai_key_set", &KeySet { provider, key }).await
}

pub async fn ai_key_clear(provider: &str) -> Response<AiOverview> {
    invoke("ai_key_clear", &ForProvider { provider }).await
}

/// Opens the provider's page for making a key, in the browser.
pub async fn ai_key_page(provider: &str) -> Response<()> {
    invoke("ai_key_page", &ForProvider { provider }).await
}

/// Polishes a sample note and answers with the result.
pub async fn ai_test(provider: &str) -> Response<String> {
    invoke("ai_test", &ForProvider { provider }).await
}

// ---- updates ---------------------------------------------------------------

/// Looks for a newer release. `None` means the app is current.
pub async fn update_check() -> Response<Option<UpdateInfo>> {
    invoke("update_check", &NoArgs {}).await
}

/// Installs the newest release and restarts. Only answers on failure, or
/// when there was nothing to install.
pub async fn update_install() -> Response<()> {
    invoke("update_install", &NoArgs {}).await
}

// ---- device sync -----------------------------------------------------------

#[derive(Serialize)]
struct SyncSetEnabled {
    on: bool,
}

#[derive(Serialize)]
struct SyncPairJoin<'a> {
    code: &'a str,
}

pub async fn sync_status() -> Response<SyncStatus> {
    invoke("sync_status", &NoArgs {}).await
}

pub async fn sync_set_enabled(on: bool) -> Response<SyncStatus> {
    invoke("sync_set_enabled", &SyncSetEnabled { on }).await
}

pub async fn sync_pair_offer() -> Response<String> {
    invoke("sync_pair_offer", &NoArgs {}).await
}

pub async fn sync_pair_join(code: &str) -> Response<SyncDevice> {
    invoke("sync_pair_join", &SyncPairJoin { code }).await
}

pub async fn sync_now() -> Response<()> {
    invoke("sync_now", &NoArgs {}).await
}

pub async fn sync_forget(id: &str) -> Response<()> {
    invoke("sync_forget", &ById { id }).await
}
