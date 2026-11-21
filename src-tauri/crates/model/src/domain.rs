use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Inbox,
    Completed,
    Archived,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Inbox => "inbox",
            Status::Completed => "completed",
            Status::Archived => "archived",
        }
    }

    pub fn parse(s: &str) -> Option<Status> {
        match s {
            "inbox" => Some(Status::Inbox),
            "completed" => Some(Status::Completed),
            "archived" => Some(Status::Archived),
            _ => None,
        }
    }
}

/// A captured note. Timestamps are UTC milliseconds since the Unix epoch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub content: String,
    pub status: Status,
    pub category: Option<String>,
    /// Name of the application that was in front when the note was captured.
    pub source: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub archived_at: Option<i64>,
    /// True when the note has a description. Lists carry only this flag;
    /// the text itself comes from `Db::note_detail`.
    #[serde(default)]
    pub has_details: bool,
    /// How many files are attached. Lists never carry the files themselves.
    #[serde(default)]
    pub attachment_count: u32,
    /// How many steps the note holds, and how many of them are ticked.
    /// Tombstoned steps are in neither count. Lists never carry the steps
    /// themselves; those come from `Db::list_steps`.
    #[serde(default)]
    pub step_total: u32,
    #[serde(default)]
    pub step_done: u32,
    /// When the note was parked in Later. `None` means it is not parked.
    /// Only an open (inbox) note can be parked.
    #[serde(default)]
    pub later_at: Option<i64>,
}

/// Largest file that can be attached to a note.
pub const MAX_ATTACHMENT_BYTES: usize = 15 * 1024 * 1024;
/// Most files one note can hold.
pub const MAX_ATTACHMENTS_PER_NOTE: usize = 20;
/// Longest description a note can have, in characters.
pub const MAX_DETAILS_CHARS: usize = 100_000;
/// Most steps one note can hold when a person adds them on one device.
/// Steps that arrive by sync are never discarded, so a note can hold more.
pub const MAX_STEPS_PER_NOTE: usize = 100;
/// Longest step text, in characters.
pub const MAX_STEP_CHARS: usize = 500;

/// A file attached to a note. The bytes live in the same database and are
/// read only on request, never with the note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub note_id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub created_at: i64,
}

/// One step of a note's ordered list. `rank` is a base-62 fraction; the
/// display order is `rank`, then `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub note_id: String,
    pub text: String,
    pub done: bool,
    pub rank: String,
}

/// A step to add. The id and rank are chosen when it is stored.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewStep {
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

/// Everything the Note details window shows for one note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteDetail {
    pub note: Note,
    pub details: String,
    pub attachments: Vec<Attachment>,
    /// The note's live steps, in display order.
    #[serde(default)]
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewNote {
    pub content: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// Filter for listing notes. Results are always newest first.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NoteQuery {
    /// Free-text search over content and category. Empty means no text filter.
    pub text: String,
    /// Statuses to include. Empty means all.
    pub statuses: Vec<Status>,
    /// Inclusive lower bound on `created_at`.
    pub created_from: Option<i64>,
    /// Exclusive upper bound on `created_at`.
    pub created_to: Option<i64>,
    /// `Some(true)`: only notes parked in Later. `Some(false)`: only notes
    /// that are not. `None`: either.
    pub later: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub inbox: i64,
    pub today: i64,
    /// Open notes parked in Later.
    pub later: i64,
    pub archive: i64,
    /// Notes in the trash that can still be restored.
    pub trash: i64,
}

/// How long a deleted note waits in the trash before it is erased for good.
pub const TRASH_RETENTION_MS: i64 = 30 * 86_400_000;

/// A note in the trash, with when it was put there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashedNote {
    pub note: Note,
    pub deleted_at: i64,
}

/// How many rows of each glance group are returned. The `done` and `total`
/// counts are computed over every matching note and are never capped.
pub const GLANCE_GROUP_LIMIT: i64 = 50;

/// What the Today window shows for one day: open notes carried over from
/// before it, notes that belong to it, and how many of them are done.
/// Carry-over is derived, never written: nothing here moves or rewrites notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Glance {
    pub carried: Vec<Note>,
    pub today: Vec<Note>,
    pub done: i64,
    pub total: i64,
}

/// How many notes were captured on one day, and how many of those are still
/// open. Drives the dots in the calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayMark {
    pub total: i64,
    pub open: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub content: String,
    pub category: Option<String>,
    pub updated_at: i64,
}

/// Where a floating window (capture or Today) opens on the display the user
/// is working on: one spot of a 3×3 grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePosition {
    TopLeft,
    /// Centred along the top edge. Stored as "upper" before 0.3.2.
    #[serde(alias = "upper")]
    Top,
    TopRight,
    Left,
    #[default]
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

/// Which column and row of the 3×3 grid a position is in: -1, 0 or 1.
impl CapturePosition {
    pub fn grid(self) -> (i8, i8) {
        use CapturePosition::*;
        match self {
            TopLeft => (-1, -1),
            Top => (0, -1),
            TopRight => (1, -1),
            Left => (-1, 0),
            Center => (0, 0),
            Right => (1, 0),
            BottomLeft => (-1, 1),
            Bottom => (0, 1),
            BottomRight => (1, 1),
        }
    }
}

fn lenient_position<'de, D: serde::Deserializer<'de>>(d: D) -> Result<CapturePosition, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// A `look_id` the stored row cannot supply usefully — absent, `null`, a
/// non-string, or empty — wears the factory default.
fn lenient_look_id<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(match value {
        serde_json::Value::String(s) if !s.is_empty() => s,
        _ => crate::DEFAULT_LOOK_ID.into(),
    })
}

/// How big the floating windows (capture, Today, note details) open. Only
/// the room changes; text keeps its size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowSize {
    /// "Compact" in Settings.
    Small,
    /// "Standard" in Settings: the original sizes.
    #[default]
    Medium,
    /// "Roomy" in Settings.
    Large,
}

impl WindowSize {
    /// What the base window sizes are multiplied by.
    pub fn factor(self) -> f64 {
        match self {
            WindowSize::Small => 0.9,
            WindowSize::Medium => 1.0,
            WindowSize::Large => 1.2,
        }
    }
}

/// Reads a window size, taking anything it does not know as Standard, so one
/// odd value never resets the other settings to their defaults.
fn lenient_window_size<'de, D: serde::Deserializer<'de>>(d: D) -> Result<WindowSize, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnterBehavior {
    /// Enter saves; Shift+Enter inserts a newline.
    Save,
    /// Enter inserts a newline; Cmd/Ctrl+Enter saves.
    Newline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DraftRetention {
    Forever,
    Week,
    Day,
}

impl DraftRetention {
    pub fn max_age_ms(self) -> Option<i64> {
        const DAY: i64 = 24 * 60 * 60 * 1000;
        match self {
            DraftRetention::Forever => None,
            DraftRetention::Week => Some(7 * DAY),
            DraftRetention::Day => Some(DAY),
        }
    }
}

pub const DEFAULT_SYNC_RELAY_URL: &str = "https://relay.posthaste.ink";
pub const DEFAULT_SYNC_DEVICE_NAME: &str = "This computer";

pub const DEFAULT_CAPTURE_SHORTCUT: &str = "CmdOrCtrl+Shift+Space";
pub const DEFAULT_INBOX_SHORTCUT: &str = if cfg!(target_os = "macos") {
    "Cmd+Ctrl+N"
} else {
    "Ctrl+Alt+N"
};
pub const DEFAULT_TODAY_SHORTCUT: &str = if cfg!(target_os = "macos") {
    "Cmd+Ctrl+G"
} else {
    "Ctrl+Alt+G"
};

/// User preferences. Unknown or missing fields fall back to defaults, so
/// settings written by older or newer versions always load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub launch_at_login: bool,
    pub start_minimized: bool,
    pub capture_shortcut: String,
    pub inbox_shortcut: String,
    /// Opens the Today window: a small glance at today's notes.
    pub today_shortcut: String,
    /// Where the capture window opens.
    #[serde(deserialize_with = "lenient_position")]
    pub capture_position: CapturePosition,
    /// Where the Today window opens. Settings from before 0.3.2 have none
    /// and load with the capture position, which Today used to share.
    #[serde(deserialize_with = "lenient_position")]
    pub today_position: CapturePosition,
    /// How big the capture, Today and details windows open.
    #[serde(deserialize_with = "lenient_window_size")]
    pub window_size: WindowSize,
    /// Hide the capture window after a successful save.
    pub hide_after_save: bool,
    /// Hide the capture window when it loses focus (the draft is kept).
    pub hide_on_blur: bool,
    pub enter_behavior: EnterBehavior,
    pub draft_retention: DraftRetention,
    /// The selected look. There is always one: a row with no usable
    /// `look_id` (an old version, a deleted look) wears the factory
    /// default, so every install is dressed by a look, never by a bare
    /// light/dark setting.
    #[serde(deserialize_with = "lenient_look_id")]
    pub look_id: String,
    /// Simple looks: keep a look's colours, turn off every scene layer.
    /// Applied in storage, so no window can forget to obey it.
    #[serde(default)]
    pub simple_looks: bool,
    /// Whether the main window's sidebar is collapsed to its icon rail.
    #[serde(default)]
    pub sidebar_collapsed: bool,
    /// The interface language, as a code from the language list ("en",
    /// "vi"). A code this build does not know is shown in English.
    pub language: String,
    /// AI polish: the id of the provider that polishes notes, or "none".
    /// Off by default: nothing is sent anywhere until the user picks a
    /// provider (and adds a key, for those that need one).
    pub ai_provider: String,
    /// The model chosen per provider id. Missing or empty means that
    /// provider's small default.
    pub ai_models: BTreeMap<String, String>,
    /// The address chosen per provider id, for providers that can point at
    /// another server (a local Ollama, any OpenAI-compatible service).
    pub ai_base_urls: BTreeMap<String, String>,
    /// Before 0.3.2 the API key lived here, in plain text. Kept on disk
    /// only until startup moves it to the keychain, so a failed move or an
    /// early save never loses it; empty (and not written) after that.
    /// Never sent to the interface.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub ai_api_key: String,
    /// Before 0.3.2 one model for whichever provider was on. Read once and
    /// moved into `ai_models`; never written again.
    #[serde(skip_serializing)]
    pub ai_model: String,
    /// Experimental peer-to-peer device sync (off by default).
    pub sync_enabled: bool,
    /// Name shown for this computer on paired devices. Empty falls back to
    /// the OS host name or "This computer".
    pub sync_device_name: String,
    /// Custom iroh relay URL. Empty means same-network only.
    pub sync_relay_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            simple_looks: false,
            sidebar_collapsed: false,
            launch_at_login: false,
            start_minimized: false,
            capture_shortcut: DEFAULT_CAPTURE_SHORTCUT.into(),
            inbox_shortcut: DEFAULT_INBOX_SHORTCUT.into(),
            today_shortcut: DEFAULT_TODAY_SHORTCUT.into(),
            capture_position: CapturePosition::Center,
            today_position: CapturePosition::Center,
            window_size: WindowSize::Medium,
            hide_after_save: true,
            hide_on_blur: true,
            enter_behavior: EnterBehavior::Save,
            draft_retention: DraftRetention::Forever,
            look_id: crate::DEFAULT_LOOK_ID.into(),
            language: crate::DEFAULT_LANGUAGE.into(),
            ai_provider: "none".into(),
            ai_models: BTreeMap::new(),
            ai_base_urls: BTreeMap::new(),
            ai_api_key: String::new(),
            ai_model: String::new(),
            sync_enabled: false,
            sync_device_name: String::new(),
            sync_relay_url: DEFAULT_SYNC_RELAY_URL.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub imported: usize,
    pub skipped_existing: usize,
    pub skipped_invalid: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataInfo {
    pub path: String,
    pub size_bytes: u64,
    pub note_count: i64,
    pub schema_version: i64,
}
