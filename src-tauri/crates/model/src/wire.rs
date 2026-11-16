//! Shapes that exist only on the IPC boundary: what commands return and
//! what events carry, beyond the stored types in `domain`.

use serde::{Deserialize, Serialize};

use crate::Settings;

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
