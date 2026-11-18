use super::*;

mod emoji_tests {
    use serde_json::{json, Map, Value};

    use super::doc;
    use crate::error::CoreError;
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;
    use crate::look::{Known, LayerRecord, LookDoc, WindowScene};

    fn sticker(char: &str, anchor: &str) -> LayerRecord {
        LayerRecord {
            kind: "emoji".into(),
            v: 1,
            on: true,
            config: json!({"char": char, "anchor": anchor, "tilt": 0}),
            extra: Map::new(),
        }
    }

    fn tilted(mut layer: LayerRecord, tilt: i32) -> LayerRecord {
        layer
            .config
            .as_object_mut()
            .unwrap()
            .insert("tilt".into(), json!(tilt));
        layer
    }

    fn with_margin(layer: LayerRecord) -> LookDoc {
        let mut d = doc();
        d.scene.capture = Some(WindowScene {
            backdrop: Vec::new(),
            margin: vec![layer],
            extra: Map::new(),
        });
        d
    }

    fn assert_invalid(d: &LookDoc, needle: &str) {
        match d.validate() {
            Ok(()) => panic!("expected refusal with \"{needle}\""),
            Err(CoreError::InvalidLook(msg)) => assert!(msg.contains(needle), "got: {msg}"),
            Err(e) => panic!("wrong error kind: {e}"),
        }
    }

    #[test]
    fn a_bad_anchor_is_refused() {
        assert_invalid(&with_margin(sticker("🌸", "middle")), "cannot read");
    }

    #[test]
    fn a_tilt_off_the_step_is_refused() {
        assert_invalid(&with_margin(tilted(sticker("🌸", "top-left"), 20)), "tilt");
    }

    #[test]
    fn a_tilt_over_45_is_refused() {
        assert_invalid(&with_margin(tilted(sticker("🌸", "top-left"), 60)), "tilt");
    }

    #[test]
    fn a_letter_is_refused() {
        assert_invalid(&with_margin(sticker("A", "top-left")), "emoji, not");
    }

    #[test]
    fn markup_characters_are_refused() {
        assert_invalid(&with_margin(sticker("<b>", "top-left")), "emoji, not");
    }

    #[test]
    fn an_empty_char_is_refused() {
        assert_invalid(&with_margin(sticker("", "top-left")), "one emoji");
    }

    #[test]
    fn a_17_byte_char_is_refused() {
        let big = "🌸".repeat(5);
        assert_invalid(&with_margin(sticker(&big, "top-left")), "one emoji");
    }

    #[test]
    fn an_emoji_in_a_backdrop_is_refused() {
        let mut d = doc();
        d.scene.all = Some(WindowScene {
            backdrop: vec![sticker("🌸", "top-left")],
            margin: Vec::new(),
            extra: Map::new(),
        });
        assert_invalid(&d, "margin");
    }

    #[test]
    fn seven_stickers_in_one_window_are_refused() {
        let mut d = doc();
        d.scene.capture = Some(WindowScene {
            backdrop: Vec::new(),
            margin: (0..7).map(|_| sticker("🌸", "top-left")).collect(),
            extra: Map::new(),
        });
        assert_invalid(&d, "6 stickers");
    }

    #[test]
    fn joined_and_variant_emoji_are_accepted() {
        let mut d = doc();
        d.scene.capture = Some(WindowScene {
            backdrop: Vec::new(),
            margin: vec![
                sticker("🌸", "top-right"),
                tilted(sticker("👩\u{200D}💻", "left-middle"), 0),
                tilted(sticker("❤\u{FE0F}", "bottom-right"), -15),
            ],
            extra: Map::new(),
        });
        d.validate().unwrap();
    }

    #[test]
    fn six_stickers_are_accepted() {
        let mut d = doc();
        d.scene.capture = Some(WindowScene {
            backdrop: Vec::new(),
            margin: (0..6).map(|_| sticker("✨", "bottom-left")).collect(),
            extra: Map::new(),
        });
        d.validate().unwrap();
    }

    #[test]
    fn set_known_keeps_unknown_config_keys() {
        let raw =
            r#"{"kind":"emoji","v":1,"config":{"char":"🌸","anchor":"top-left","future":"keep"}}"#;
        let mut layer: LayerRecord = serde_json::from_str(raw).unwrap();
        let known = layer.known().unwrap();
        let Known::Emoji(mut emoji) = known else {
            panic!("expected an emoji layer");
        };
        emoji.char = "🌿".into();
        layer.set_known(&Known::Emoji(emoji));
        let back: Value = serde_json::to_value(&layer).unwrap();
        assert_eq!(back["config"]["char"], "🌿");
        assert_eq!(back["config"]["future"], "keep");
    }
}
