use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// the text itself comes from [`crate::Db::note_detail`].
    #[serde(default)]
    pub has_details: bool,
    /// How many files are attached. Lists never carry the files themselves.
    #[serde(default)]
    pub attachment_count: u32,
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

/// Everything the Note details window shows for one note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoteDetail {
    pub note: Note,
    pub details: String,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NewNote {
    pub content: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// Filter for listing notes. Results are always newest first.
#[derive(Debug, Clone, Default, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Glance {
    pub carried: Vec<Note>,
    pub today: Vec<Note>,
    pub done: i64,
    pub total: i64,
}

/// How many notes were captured on one day, and how many of those are still
/// open. Drives the dots in the calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapturePosition {
    /// Centre of the active display.
    Center,
    /// Horizontally centred, in the upper third (launcher style).
    Upper,
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

/// Which AI provider polishes notes, if any. Off by default: nothing is
/// sent anywhere until the user picks a provider and adds their own key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    #[default]
    None,
    Anthropic,
    OpenAI,
    Gemini,
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
    pub theme: Theme,
    pub launch_at_login: bool,
    pub start_minimized: bool,
    pub capture_shortcut: String,
    pub inbox_shortcut: String,
    /// Opens the Today window: a small glance at today's notes.
    pub today_shortcut: String,
    pub capture_position: CapturePosition,
    /// How big the capture, Today and details windows open.
    #[serde(deserialize_with = "lenient_window_size")]
    pub window_size: WindowSize,
    /// Hide the capture window after a successful save.
    pub hide_after_save: bool,
    /// Hide the capture window when it loses focus (the draft is kept).
    pub hide_on_blur: bool,
    pub enter_behavior: EnterBehavior,
    pub draft_retention: DraftRetention,
    /// The selected look. `None` is the Plain look (system/light/dark);
    /// out of the box every install wears Ink.
    pub look_id: Option<String>,
    /// AI polish: the provider, the user's own API key, and the model
    /// (empty string = the provider's small default model).
    pub ai_provider: AiProvider,
    pub ai_api_key: String,
    pub ai_model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::System,
            launch_at_login: false,
            start_minimized: false,
            capture_shortcut: DEFAULT_CAPTURE_SHORTCUT.into(),
            inbox_shortcut: DEFAULT_INBOX_SHORTCUT.into(),
            today_shortcut: DEFAULT_TODAY_SHORTCUT.into(),
            capture_position: CapturePosition::Center,
            window_size: WindowSize::Medium,
            hide_after_save: true,
            hide_on_blur: true,
            enter_behavior: EnterBehavior::Save,
            draft_retention: DraftRetention::Forever,
            look_id: Some(crate::look::DEFAULT_LOOK_ID.into()),
            ai_provider: AiProvider::None,
            ai_api_key: String::new(),
            ai_model: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    pub imported: usize,
    pub skipped_existing: usize,
    pub skipped_invalid: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataInfo {
    pub path: String,
    pub size_bytes: u64,
    pub note_count: i64,
    pub schema_version: i64,
}
