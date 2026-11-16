//! One typed function per shell command. Adding a command to a window means
//! adding it here, next to its argument struct; the names and shapes match
//! `src-tauri/src/commands.rs`.

use std::collections::BTreeMap;

use posthaste_model::{CommandError, DayMark, Draft, Glance, Os, SettingsState, Status};
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use super::tauri::{invoke, NoArgs};

pub type Response<T> = Result<T, CommandError>;

/// The part of `app_info` the Rust windows use.
#[derive(Debug, Clone, Deserialize)]
pub struct AppInfo {
    pub os: Os,
}

/// The active look as `look_active` sends it: tokens ready to apply.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ActiveLook {
    /// "light" or "dark": the scheme the look is drawn on.
    pub base: String,
    pub shape: String,
    pub font: String,
    /// CSS custom properties, name to value.
    pub tokens: BTreeMap<String, String>,
}

pub async fn app_info() -> Response<AppInfo> {
    invoke("app_info", &NoArgs {}).await
}

pub async fn settings_get() -> Response<SettingsState> {
    invoke("settings_get", &NoArgs {}).await
}

/// `None` is the Plain look.
pub async fn look_active() -> Response<Option<ActiveLook>> {
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

/// The window Note details was opened from, and goes back to.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DetailsFrom {
    Today,
}

#[derive(Serialize)]
struct DetailsOpen<'a> {
    id: &'a str,
    from: DetailsFrom,
}

pub async fn details_open(id: &str, from: DetailsFrom) -> Response<()> {
    invoke("details_open", &DetailsOpen { id, from }).await
}
