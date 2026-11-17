//! Shapes that exist only on the IPC boundary: what commands return and
//! what events carry, beyond the stored types in `domain`.

use serde::{Deserialize, Serialize};

use crate::{Attachment, Settings};

/// The registration state of one global shortcut.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutStatus {
    pub accelerator: String,
    /// Whether the operating system accepted the registration.
    pub registered: bool,
    /// Why registration failed, in plain language.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutReport {
    pub capture: ShortcutStatus,
    pub inbox: ShortcutStatus,
    pub today: ShortcutStatus,
    /// Set when shortcuts register but cannot be relied on (Wayland).
    pub limitation: Option<String>,
}

/// What `settings_get` and `settings_update` return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsState {
    pub settings: Settings,
    pub shortcuts: ShortcutReport,
}

/// The failure shape of every command: a stable `code` to branch on and a
/// `message` written for the person using the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

/// Payload of the `capture-shown` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureShown {
    /// Name of the application that was in front, when it is known.
    pub source: Option<String>,
}

/// The operating system the app runs on, as `app_info` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Macos,
    Windows,
    Linux,
}

impl Os {
    /// The system this code was compiled for.
    pub const fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::Macos
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

/// What `app_info` returns: facts about the running app that never change
/// while it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    pub version: String,
    pub os: Os,
    /// "macos", "windows", "x11", "wayland" or "unknown".
    pub session: String,
    pub tray_available: bool,
    /// Present when the database could not be opened at startup.
    pub startup_error: Option<CommandError>,
}

/// Where the details window was opened from; Esc goes back there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DetailsFrom {
    Today,
    Inbox,
}

/// The note the details window shows. Sent with `details-open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailsTarget {
    pub id: String,
    pub from: DetailsFrom,
}

/// One file that could not be attached, named so the user knows which.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachFailure {
    pub name: String,
    pub code: String,
    pub message: String,
}

/// What happened to a batch of files: the ones stored and the ones refused.
/// One bad file never stops the others.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachReport {
    pub added: Vec<Attachment>,
    pub failed: Vec<AttachFailure>,
}

/// Whether an AI provider needs an API key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyNeed {
    Required,
    /// Self-hosted servers may or may not ask for one.
    Optional,
    /// Runs on this computer; nothing to sign in to.
    None,
}

/// Where AI provider keys are kept on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyStore {
    Keychain,
    /// There is no keychain here, so keys sit in the database.
    Database,
}

/// One AI provider as Settings shows it. Never carries the key itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiProviderInfo {
    pub id: String,
    pub name: String,
    pub key: KeyNeed,
    pub has_key: bool,
    pub key_url: Option<String>,
    pub key_help: String,
    pub default_model: String,
    pub base_url: String,
    pub base_url_editable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiOverview {
    pub backend: KeyStore,
    pub providers: Vec<AiProviderInfo>,
}

/// A newer release the updater found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
}
