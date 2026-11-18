use super::*;

mod one_format_tests {
    use super::OLD_VERSION;
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;

    #[test]
    fn a_version_1_document_is_refused() {
        let raw = format!(
            r##"{{"format":"posthaste-look","version":{OLD_VERSION},"name":"X","base":"light",
            "colors":{{"bg":"#ffffff","surface":"#f2f2f2","text":"#111111","accent":"#3366cc"}}}}"##
        );
        let err = crate::look::LookDoc::from_json(&raw)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("This isn't a Posthaste look file."),
            "got: {err}"
        );
    }

    #[test]
    fn every_bundled_look_is_version_2() {
        for (id, doc) in crate::look::bundled::bundled() {
            assert_eq!(doc.version, 2, "{id}");
            assert_eq!(doc.format, "posthaste-look", "{id}");
        }
    }
}

mod validation_tests {
    use super::doc;
    use crate::look::LookDoc;
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;

    fn assert_invalid(d: &LookDoc, needle: &str) {
        let err = d.validate().unwrap_err().to_string();
        assert!(
            err.contains(needle),
            "expected {needle:?} in the message, got: {err}"
        );
    }

    #[test]
    fn the_strawberry_milk_document_is_valid() {
        assert!(doc().validate().is_ok());
    }

    #[test]
    fn uppercase_three_digit_hex_is_accepted() {
        let mut d = doc();
        d.theme.colors.bg = "#FFF".into();
        assert!(d.validate().is_ok());
    }

    #[test]
    fn a_foreign_format_is_rejected() {
        let mut d = doc();
        d.format = "other".into();
        assert_invalid(&d, "isn't a Posthaste look");
    }

    #[test]
    fn a_newer_version_is_rejected() {
        let mut d = doc();
        d.version = 3;
        assert_invalid(&d, "newer version");
    }

    #[test]
    fn blank_names_are_rejected() {
        let mut d = doc();
        d.name = "".into();
        assert_invalid(&d, "Give the look a name");
        d.name = "   ".into();
        assert_invalid(&d, "Give the look a name");
    }

    #[test]
    fn names_over_forty_characters_are_rejected() {
        let mut d = doc();
        d.name = "x".repeat(41);
        assert_invalid(&d, "at most 40 characters");
    }

    #[test]
    fn names_with_control_characters_are_rejected() {
        let mut d = doc();
        d.name = "a\nb".into();
        assert_invalid(&d, "control characters");
    }

    #[test]
    fn emoji_and_accented_names_pass_and_the_limit_counts_characters() {
        let mut d = doc();
        d.name = "🍓 Strawberry".into();
        assert!(d.validate().is_ok());
        d.name = "🍓".repeat(40);
        assert!(d.validate().is_ok());
        d.name = "🍓".repeat(41);
        assert_invalid(&d, "at most 40 characters");
    }

    #[test]
    fn a_long_author_name_is_rejected() {
        let mut d = doc();
        d.author = Some("x".repeat(41));
        assert_invalid(&d, "author name can be at most 40");
    }

    #[test]
    fn only_hex_colours_are_accepted() {
        for bad in ["red", "#ff8fb1ff", "rgb(1,2,3)", "url(x)"] {
            let mut d = doc();
            d.theme.colors.bg = bad.into();
            let err = d.validate().unwrap_err().to_string();
            assert!(err.contains("Color \"bg\""), "got: {err}");
            assert!(err.contains(bad), "got: {err}");
        }
    }

    #[test]
    fn text_matching_the_background_is_rejected_with_the_ratio() {
        let mut d = doc();
        d.theme.colors.text = d.theme.colors.bg.clone();
        let err = d.validate().unwrap_err().to_string();
        assert!(err.contains("too similar to read"), "got: {err}");
        assert!(err.contains("1.0:1"), "got: {err}");
    }

    #[test]
    fn text_matching_the_surface_is_rejected() {
        let mut d = doc();
        d.theme.colors.bg = "#222222".into();
        d.theme.colors.text = "#ffffff".into();
        d.theme.colors.surface = "#ffffff".into();
        assert_invalid(&d, "Text and surface");
    }

    #[test]
    fn an_accent_invisible_on_the_background_is_rejected() {
        let mut d = doc();
        d.theme.colors.accent = d.theme.colors.bg.clone();
        assert_invalid(&d, "almost invisible");
    }

    #[test]
    fn a_missing_accent_rejects_the_look() {
        assert!(LookDoc::from_json(
            r##"{"format":"posthaste-look","version":2,"name":"x","theme":{"base":"light",
                "colors":{"bg":"#ffffff","surface":"#ffffff","text":"#000000"}}}"##
        )
        .is_err());
    }

    #[test]
    fn a_document_over_the_limit_is_too_large() {
        let mut d = doc();
        d.name = "y".repeat(20_000);
        let err = LookDoc::from_json(&d.to_json()).unwrap_err().to_string();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn to_json_round_trips_and_omits_missing_colours() {
        let d = doc();
        let json = d.to_json();
        assert_eq!(LookDoc::from_json(&json).unwrap(), d);
        assert!(!json.contains("danger"));
        assert!(!json.contains("bg_sunken"));

        let mut with_optional = doc();
        with_optional.theme.colors.danger = Some("#ff0000".into());
        let json = with_optional.to_json();
        assert!(json.contains("danger"));
        assert_eq!(LookDoc::from_json(&json).unwrap(), with_optional);
    }
}

mod format_layer_tests {
    use serde_json::{json, Map};

    use crate::error::CoreError;
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;
    use crate::look::{LayerRecord, LookDoc, WindowScene};

    use super::doc;

    /// A document whose colours carry a key this build does not model.
    const FUTURE_LOOK: &str = r##"{"format":"posthaste-look","version":2,"name":"N",
        "theme":{"base":"light","colors":{"bg":"#ffffff","surface":"#f2f2f2",
        "text":"#111111","accent":"#3366cc","border":"#999999"}}}"##;

    fn image_layer(dim: f64) -> LayerRecord {
        LayerRecord {
            kind: "image".into(),
            v: 1,
            on: true,
            config: json!({"asset": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "dim": dim}),
            extra: Map::new(),
        }
    }

    fn assert_invalid(d: &LookDoc, needle: &str) {
        match d.validate() {
            Ok(()) => panic!("expected refusal with \"{needle}\""),
            Err(CoreError::InvalidLook(msg)) => assert!(msg.contains(needle), "got: {msg}"),
            Err(e) => panic!("wrong error kind: {e}"),
        }
    }

    #[test]
    fn a_theme_only_document_is_valid_and_round_trips() {
        let d = doc();
        d.validate().unwrap();
        let back = LookDoc::from_json(&d.to_json()).unwrap();
        assert_eq!(back, d);
        // The empty scene and asset list are not written out.
        assert!(!d.to_json().contains("\"scene\""));
        assert!(!d.to_json().contains("\"assets\""));
    }

    #[test]
    fn an_unknown_colour_key_survives_a_round_trip() {
        let doc = crate::look::LookDoc::from_json(FUTURE_LOOK).unwrap();
        let back: serde_json::Value = serde_json::from_str(&doc.to_json()).unwrap();
        assert_eq!(back["theme"]["colors"]["border"], "#999999");
    }

    #[test]
    fn a_document_newer_than_this_build_is_refused() {
        let mut d = doc();
        d.version = 3;
        assert_invalid(&d, "Update the app");
    }

    #[test]
    fn an_oversized_document_is_refused_before_it_is_parsed() {
        let mut d = doc();
        d.extra.insert("pad".into(), json!("x".repeat(20_000)));
        let err = LookDoc::from_json(&d.to_json()).unwrap_err().to_string();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn a_document_nested_deeper_than_16_is_refused() {
        let mut d = doc();
        let mut deep = json!(true);
        for _ in 0..20 {
            deep = json!([deep]);
        }
        d.extra.insert("deep".into(), deep);
        let err = LookDoc::from_json(&d.to_json()).unwrap_err().to_string();
        assert!(err.contains("levels deep"), "got: {err}");
    }

    #[test]
    fn a_look_with_more_than_64_layers_is_refused() {
        let mut d = doc();
        d.scene.all = Some(WindowScene {
            backdrop: (0..65).map(|_| image_layer(0.5)).collect(),
            margin: Vec::new(),
            extra: Map::new(),
        });
        assert_invalid(&d, "at most 64 layers");
    }

    #[test]
    fn sixty_four_layers_are_accepted() {
        let mut d = doc();
        d.scene.all = Some(WindowScene {
            backdrop: (0..64).map(|_| image_layer(0.5)).collect(),
            margin: Vec::new(),
            extra: Map::new(),
        });
        d.assets = vec![super::listed_asset(&super::asset_id())];
        d.validate().unwrap();
    }

    #[test]
    fn an_image_layer_outside_the_backdrop_is_refused() {
        let mut d = doc();
        d.scene.capture = Some(WindowScene {
            backdrop: Vec::new(),
            margin: vec![image_layer(0.5)],
            extra: Map::new(),
        });
        assert_invalid(&d, "backdrop");
    }

    #[test]
    fn an_image_dimmed_out_of_range_is_refused() {
        let mut d = doc();
        d.scene.main = Some(WindowScene {
            backdrop: vec![image_layer(1.5)],
            margin: Vec::new(),
            extra: Map::new(),
        });
        assert_invalid(&d, "dim");
    }

    #[test]
    fn an_unknown_layer_kind_is_kept_untouched() {
        let mut d = doc();
        let mut extra = Map::new();
        extra.insert("custom".into(), json!("keep"));
        d.scene.capture = Some(WindowScene {
            backdrop: Vec::new(),
            margin: vec![LayerRecord {
                kind: "shader".into(),
                v: 9,
                on: false,
                config: json!({"source": "spiral"}),
                extra,
            }],
            extra: Map::new(),
        });
        d.validate().unwrap();
        let layer = &d.scene.capture.as_ref().unwrap().margin[0];
        assert_eq!(layer.kind, "shader");
        assert!(!layer.on);
        assert_eq!(layer.config, json!({"source": "spiral"}));
        assert_eq!(layer.extra.get("custom"), Some(&json!("keep")));
    }

    #[test]
    fn a_known_kind_with_a_higher_v_is_kept() {
        let mut d = doc();
        d.scene.main = Some(WindowScene {
            backdrop: vec![LayerRecord {
                kind: "image".into(),
                v: 2,
                on: true,
                config: json!({"asset": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "sharpen": 4}),
                extra: Map::new(),
            }],
            margin: Vec::new(),
            extra: Map::new(),
        });
        d.validate().unwrap();
        // The config is not readable at this build's version, so it is
        // kept verbatim and never checked.
        assert_eq!(
            d.scene.main.as_ref().unwrap().backdrop[0].config,
            json!({"asset": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "sharpen": 4})
        );
    }

    #[test]
    fn an_unknown_window_entry_is_kept() {
        let mut d = doc();
        d.scene
            .extra
            .insert("laterWindow".into(), json!({"backdrop": []}));
        d.validate().unwrap();
        assert_eq!(
            d.scene.extra.get("laterWindow"),
            Some(&json!({"backdrop": []}))
        );
    }

    #[test]
    fn unknown_fields_at_every_level_survive_a_round_trip() {
        let mut d = doc();
        d.extra.insert("futureTop".into(), json!({"a": 1}));
        d.theme.extra.insert("laterTheme".into(), json!(7));
        d.scene.extra.insert("laterWindow".into(), json!({}));
        let mut layer = image_layer(0.45);
        layer.extra.insert("futureKey".into(), json!("keep"));
        d.scene.all = Some(WindowScene {
            backdrop: vec![layer],
            margin: Vec::new(),
            extra: Map::new(),
        });
        d.assets = vec![super::listed_asset(&super::asset_id())];
        let back = LookDoc::from_json(&d.to_json()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn a_layer_that_is_on_serializes_without_on() {
        let l: crate::look::LayerRecord = serde_json::from_str(r#"{"kind":"zz","v":1}"#).unwrap();
        assert_eq!(serde_json::to_string(&l).unwrap(), r#"{"kind":"zz","v":1}"#);
    }

    #[test]
    fn a_near_limit_document_still_reimports_after_one_save() {
        // A document just under the limit whose 64 layers carry no "on"
        // key: writing every layer back with `"on":true` used to push it
        // over the limit, so the next import refused the app's own save.
        let mut pad = 20_000usize;
        let json = loop {
            let mut d = doc();
            d.extra
                .insert("pad".into(), serde_json::json!("x".repeat(pad)));
            let layers: Vec<crate::look::LayerRecord> = (0..64)
                .map(|_| crate::look::LayerRecord {
                    kind: "zz".into(),
                    v: 1,
                    on: true,
                    config: serde_json::Value::Null,
                    extra: Map::new(),
                })
                .collect();
            d.scene.all = Some(crate::look::WindowScene {
                backdrop: layers,
                margin: Vec::new(),
                extra: Map::new(),
            });
            let raw = {
                // Serialize through Value so the input has no "on" keys.
                let value = serde_json::to_value(&d).unwrap();
                let mut value = value;
                if let Some(all) = value["scene"]["all"]["backdrop"].as_array_mut() {
                    for layer in all {
                        layer.as_object_mut().unwrap().remove("on");
                    }
                }
                serde_json::to_string(&value).unwrap()
            };
            if raw.len() <= 16_384 {
                assert!(
                    raw.len() > 15_808,
                    "the fixture must be near the limit: {}",
                    raw.len()
                );
                break raw;
            }
            pad -= 50;
        };
        let parsed = crate::look::LookDoc::from_json(&json).unwrap();
        let again = crate::look::LookDoc::from_json(&parsed.to_json()).unwrap();
        assert_eq!(again, parsed);
    }
}

mod document_tests {
    use super::OLD_VERSION;
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;

    #[test]
    fn a_document_from_a_newer_version_is_refused_with_the_update_message() {
        let json = r##"{"format":"posthaste-look","version":9,"name":"X"}"##;
        let err = crate::look::LookDoc::from_json(json)
            .unwrap_err()
            .to_string();
        assert!(err.contains("Update the app"), "got: {err}");
    }

    #[test]
    fn a_version_below_two_is_not_a_look_file() {
        let json = format!(r##"{{"format":"posthaste-look","version":{OLD_VERSION},"name":"X"}}"##);
        let err = crate::look::LookDoc::from_json(&json)
            .unwrap_err()
            .to_string();
        assert!(err.contains("isn't a Posthaste look file"), "got: {err}");
    }

    #[test]
    fn garbage_is_not_a_look() {
        let err = crate::look::LookDoc::from_json("not json at all")
            .unwrap_err()
            .to_string();
        assert!(err.contains("isn't a valid look"), "got: {err}");
    }
}
