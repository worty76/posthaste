//! The data shapes every part of Posthaste agrees on. Storage writes them,
//! the desktop shell sends them over IPC, and the UI reads them: one
//! definition, so the two sides of the IPC boundary cannot drift apart.
//!
//! Nothing here does I/O, and the crate builds for `wasm32` as well as for
//! the desktop targets.

mod domain;
mod wire;

pub use domain::*;
pub use wire::*;

/// The look every install wears out of the box.
pub const DEFAULT_LOOK_ID: &str = "builtin:ink";
/// The language the interface uses until the user picks another.
pub const DEFAULT_LANGUAGE: &str = "en";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_saved_before_languages_existed_load_in_english() {
        let settings: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(settings.language, DEFAULT_LANGUAGE);
        assert_eq!(settings.theme, Theme::Dark);
    }

    #[test]
    fn the_chosen_language_survives_a_save_and_load() {
        let settings = Settings {
            language: "vi".into(),
            ..Settings::default()
        };
        let stored = serde_json::to_string(&settings).unwrap();
        let loaded: Settings = serde_json::from_str(&stored).unwrap();
        assert_eq!(loaded.language, "vi");
    }

    #[test]
    fn a_settings_state_reads_back_the_way_the_shell_wrote_it() {
        let state = SettingsState {
            settings: Settings::default(),
            shortcuts: ShortcutReport::default(),
        };
        let wire = serde_json::to_value(&state).unwrap();
        // The legacy key field is never sent to the interface.
        assert!(wire["settings"].get("ai_api_key").is_none());
        let read: SettingsState = serde_json::from_value(wire).unwrap();
        assert_eq!(read, state);
    }
}
