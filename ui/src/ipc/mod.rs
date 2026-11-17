//! The boundary with the desktop shell. Everything else calls `api::*` and
//! `events::on`, so this is the one place that knows Tauri exists and the
//! one place a test has to replace.

pub mod api;
pub mod events;
mod tauri;

pub use tauri::window_label;

use posthaste_i18n::{key, Translator};
use posthaste_model::CommandError;

/// What a failed command says to the person using the app. The shell's own
/// message is already written for them; the fallback covers a failure that
/// never reached the shell.
pub fn message(error: &CommandError, t: Translator) -> String {
    if error.message.is_empty() {
        t.text(key::COMMON_UNKNOWN_ERROR).to_owned()
    } else {
        error.message.clone()
    }
}
