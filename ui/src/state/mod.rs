//! State every window shares: the settings, the language and the look.
//! Each is loaded once per window and kept current from the shell's events.

mod appearance;
mod i18n;
mod settings;

pub use appearance::use_appearance;
pub use i18n::{use_i18n, I18n};
pub use settings::{use_settings, SettingsHandle};
