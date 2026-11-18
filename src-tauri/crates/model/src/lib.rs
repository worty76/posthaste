//! The data shapes every part of Posthaste agrees on. Storage writes them,
//! the desktop shell sends them over IPC, and the UI reads them: one
//! definition, so the two sides of the IPC boundary cannot drift apart.
//!
//! Nothing here does I/O, and the crate builds for `wasm32` as well as for
//! the desktop targets.

mod domain;
mod look;
mod wire;

pub use domain::*;
pub use look::*;
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
        // The old theme key is ignored, and with no usable look the
        // factory default is worn.
        assert_eq!(settings.look_id, DEFAULT_LOOK_ID);
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

    #[test]
    fn a_look_keeps_fields_it_does_not_model_through_serde() {
        let raw = r##"{"format":"posthaste-look","version":2,"name":"N","author":"A",
            "theme":{"base":"light","colors":{"bg":"#ffffff","surface":"#f2f2f2",
            "text":"#111111","accent":"#3366cc"},"shape":"round","font":"system","font_family":null,"laterTheme":7},
            "scene":{"all":{"backdrop":[{"kind":"shader","v":9,"config":{"src":"x"},
            "custom":"keep"}]}},"assets":[{"id":"sha256:ab12","type":"image/png",
            "bytes":1,"width":2,"height":3,"future":true}],"futureTop":{"a":1}}"##;
        let doc: LookDoc = serde_json::from_str(raw).unwrap();
        assert_eq!(
            doc.extra.get("futureTop"),
            Some(&serde_json::json!({"a":1}))
        );
        assert_eq!(doc.theme.extra.get("laterTheme"), Some(&7.into()));
        let layer = &doc.scene.all.as_ref().unwrap().backdrop[0];
        assert_eq!(layer.extra.get("custom"), Some(&"keep".into()));
        let asset = &doc.assets[0];
        assert_eq!(asset.extra.get("future"), Some(&true.into()));
        // The kept fields come back out the way they went in.
        let back = serde_json::to_value(&doc).unwrap();
        assert_eq!(back["futureTop"], serde_json::json!({"a":1}));
        assert_eq!(back["theme"]["laterTheme"], 7);
        assert_eq!(back["scene"]["all"]["backdrop"][0]["custom"], "keep");
        assert_eq!(back["assets"][0]["future"], true);
    }
}
