use super::*;

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
        d.colors.bg = "#FFF".into();
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
        d.version = 2;
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
            d.colors.bg = bad.into();
            let err = d.validate().unwrap_err().to_string();
            assert!(err.contains("Color \"bg\""), "got: {err}");
            assert!(err.contains(bad), "got: {err}");
        }
    }

    #[test]
    fn text_matching_the_background_is_rejected_with_the_ratio() {
        let mut d = doc();
        d.colors.text = d.colors.bg.clone();
        let err = d.validate().unwrap_err().to_string();
        assert!(err.contains("too similar to read"), "got: {err}");
        assert!(err.contains("1.0:1"), "got: {err}");
    }

    #[test]
    fn text_matching_the_surface_is_rejected() {
        let mut d = doc();
        d.colors.bg = "#222222".into();
        d.colors.text = "#ffffff".into();
        d.colors.surface = "#ffffff".into();
        assert_invalid(&d, "Text and surface");
    }

    #[test]
    fn an_accent_invisible_on_the_background_is_rejected() {
        let mut d = doc();
        d.colors.accent = d.colors.bg.clone();
        assert_invalid(&d, "almost invisible");
    }

    #[test]
    fn an_unknown_field_rejects_the_whole_look() {
        let err = LookDoc::from_json(
            r##"{"format":"posthaste-look","version":1,"name":"x","base":"light",
                "colors":{"bg":"#ffffff","surface":"#ffffff","text":"#000000","accent":"#000000"},
                "extra":true}"##,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("unknown field"), "got: {err}");
    }

    #[test]
    fn an_unknown_colour_field_rejects_the_whole_look() {
        let err = LookDoc::from_json(
            r##"{"format":"posthaste-look","version":1,"name":"x","base":"light",
                "colors":{"bg":"#ffffff","surface":"#ffffff","text":"#000000","accent":"#000000","glow":"#ffffff"}}"##,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("unknown field"), "got: {err}");
    }

    #[test]
    fn a_missing_accent_rejects_the_look() {
        assert!(LookDoc::from_json(
            r##"{"format":"posthaste-look","version":1,"name":"x","base":"light",
                "colors":{"bg":"#ffffff","surface":"#ffffff","text":"#000000"}}"##,
        )
        .is_err());
    }

    #[test]
    fn a_document_over_4096_bytes_is_too_large() {
        let mut d = doc();
        d.name = "y".repeat(5000);
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
        with_optional.colors.danger = Some("#ff0000".into());
        let json = with_optional.to_json();
        assert!(json.contains("danger"));
        assert_eq!(LookDoc::from_json(&json).unwrap(), with_optional);
    }
}

mod migrate_tests {
    use super::doc;
    use crate::look::bundled::bundled;
    use crate::look::derive::resolve;
    use crate::look::migrate::v1_to_v2;
    #[allow(unused_imports)]
    use crate::look::LookDocV2Ext as _;
    use crate::look::LookDocument;

    #[test]
    fn every_bundled_look_migrates_to_v2_and_resolves_identically() {
        for (id, doc) in bundled() {
            // The colour looks are v1 documents; migrating them must not
            // move a single token. The demo looks are already v2.
            let LookDocument::V1(v1) = doc else {
                continue;
            };
            let before = resolve(&id, &v1).unwrap();
            let v2 = v1_to_v2(&v1);
            if let Err(e) = v2.validate() {
                panic!("{id} fails v2 validation: {e}");
            }
            let after = resolve(&id, &v2.v1_view()).unwrap();
            assert_eq!(before, after, "{id}");
        }
    }

    #[test]
    fn migration_moves_the_theme_fields_and_starts_an_empty_scene() {
        let v1 = doc();
        let v2 = v1_to_v2(&v1);
        assert_eq!(v2.format, "posthaste-look");
        assert_eq!(v2.version, 2);
        assert_eq!(v2.name, v1.name);
        assert_eq!(v2.author, v1.author);
        assert_eq!(v2.theme.base, v1.base);
        assert_eq!(v2.theme.colors, v1.colors);
        assert_eq!(v2.theme.shape, v1.shape);
        assert_eq!(v2.theme.font, v1.font);
        assert!(v2.scene.is_empty());
        assert!(v2.assets.is_empty());
        assert!(v2.extra.is_empty());
        assert!(v2.theme.extra.is_empty());
    }
}

mod v2_validation_tests {
    use serde_json::{json, Map};

    use crate::error::CoreError;
    #[allow(unused_imports)]
    use crate::look::LookDocV2Ext as _;
    use crate::look::{LayerRecord, LookDocV2, WindowScene};

    use super::v2doc;

    fn image_layer(dim: f64) -> LayerRecord {
        LayerRecord {
            kind: "image".into(),
            v: 1,
            on: true,
            config: json!({"asset": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "dim": dim}),
            extra: Map::new(),
        }
    }

    fn assert_invalid(d: &LookDocV2, needle: &str) {
        match d.validate() {
            Ok(()) => panic!("expected refusal with \"{needle}\""),
            Err(CoreError::InvalidLook(msg)) => assert!(msg.contains(needle), "got: {msg}"),
            Err(e) => panic!("wrong error kind: {e}"),
        }
    }

    #[test]
    fn a_theme_only_v2_document_is_valid_and_round_trips() {
        let d = v2doc();
        d.validate().unwrap();
        let back = LookDocV2::from_json(&d.to_json()).unwrap();
        assert_eq!(back, d);
        // The empty scene and asset list are not written out.
        assert!(!d.to_json().contains("\"scene\""));
        assert!(!d.to_json().contains("\"assets\""));
    }

    #[test]
    fn a_document_newer_than_v2_is_refused() {
        let mut d = v2doc();
        d.version = 3;
        assert_invalid(&d, "Update the app");
    }

    #[test]
    fn an_oversized_v2_document_is_refused_before_it_is_parsed() {
        let mut d = v2doc();
        d.extra.insert("pad".into(), json!("x".repeat(20_000)));
        let err = LookDocV2::from_json(&d.to_json()).unwrap_err().to_string();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn a_document_nested_deeper_than_16_is_refused() {
        let mut d = v2doc();
        let mut deep = json!(true);
        for _ in 0..20 {
            deep = json!([deep]);
        }
        d.extra.insert("deep".into(), deep);
        let err = LookDocV2::from_json(&d.to_json()).unwrap_err().to_string();
        assert!(err.contains("levels deep"), "got: {err}");
    }

    #[test]
    fn a_v2_look_with_more_than_64_layers_is_refused() {
        let mut d = v2doc();
        d.scene.all = Some(WindowScene {
            backdrop: (0..65).map(|_| image_layer(0.5)).collect(),
            margin: Vec::new(),
            extra: Map::new(),
        });
        assert_invalid(&d, "at most 64 layers");
    }

    #[test]
    fn sixty_four_layers_are_accepted() {
        let mut d = v2doc();
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
        let mut d = v2doc();
        d.scene.capture = Some(WindowScene {
            backdrop: Vec::new(),
            margin: vec![image_layer(0.5)],
            extra: Map::new(),
        });
        assert_invalid(&d, "backdrop");
    }

    #[test]
    fn an_image_dimmed_out_of_range_is_refused() {
        let mut d = v2doc();
        d.scene.main = Some(WindowScene {
            backdrop: vec![image_layer(1.5)],
            margin: Vec::new(),
            extra: Map::new(),
        });
        assert_invalid(&d, "dim");
    }

    #[test]
    fn an_unknown_layer_kind_is_kept_untouched() {
        let mut d = v2doc();
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
        let mut d = v2doc();
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
        let mut d = v2doc();
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
        let mut d = v2doc();
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
        let back = LookDocV2::from_json(&d.to_json()).unwrap();
        assert_eq!(back, d);
    }
}

mod document_tests {
    #[allow(unused_imports)]
    use crate::look::{LookDocExt as _, LookDocV2Ext as _, LookDocument};

    #[test]
    fn from_json_dispatches_on_the_version() {
        let v1 = r##"{"format":"posthaste-look","version":1,"name":"A","base":"light","colors":{"bg":"#fff5f7","surface":"#ffffff","text":"#4a2c3a","accent":"#ff8fb1"}}"##;
        let v2 = r##"{"format":"posthaste-look","version":2,"name":"B","theme":{"base":"dark","colors":{"bg":"#120d18","surface":"#1d1526","text":"#f3e9f7","accent":"#ff7eb6"}}}"##;
        assert!(matches!(
            LookDocument::from_json(v1),
            Ok(LookDocument::V1(_))
        ));
        assert!(matches!(
            LookDocument::from_json(v2),
            Ok(LookDocument::V2(_))
        ));
    }

    #[test]
    fn a_document_from_a_newer_version_is_refused_with_the_update_message() {
        let json = r##"{"format":"posthaste-look","version":9,"name":"X"}"##;
        let err = LookDocument::from_json(json).unwrap_err().to_string();
        assert!(err.contains("Update the app"), "got: {err}");
    }

    #[test]
    fn a_v1_document_keeps_its_own_size_limit() {
        let mut d = super::doc();
        d.name = "x".repeat(5_000);
        let err = LookDocument::from_json(&d.to_json())
            .unwrap_err()
            .to_string();
        assert!(err.contains("4096"), "got: {err}");
    }

    #[test]
    fn garbage_is_not_a_look() {
        let err = LookDocument::from_json("not json at all")
            .unwrap_err()
            .to_string();
        assert!(err.contains("isn't a valid look"), "got: {err}");
    }
}

mod part_a_tests {
    use serde_json::Map;

    use super::super::store::v2_store_tests::FUTURE_LOOK;
    use crate::look::{LookDocExt as _, LookDocV2Ext as _};

    #[test]
    fn an_unknown_colour_key_survives_a_v2_round_trip() {
        let doc = crate::look::LookDocV2::from_json(FUTURE_LOOK).unwrap();
        let back: serde_json::Value = serde_json::from_str(&doc.to_json()).unwrap();
        assert_eq!(back["theme"]["colors"]["border"], "#999999");
    }

    #[test]
    fn a_v1_look_with_an_unknown_colour_key_is_still_refused() {
        let raw = r##"{"format":"posthaste-look","version":1,"name":"N","base":"light",
              "colors":{"bg":"#ffffff","surface":"#f2f2f2","text":"#111111","accent":"#3366cc","border":"#999999"}}"##;
        let err = crate::look::LookDoc::from_json(raw)
            .unwrap_err()
            .to_string();
        assert!(err.contains("border"), "{err}");
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
            let mut d = super::v2doc();
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
        let parsed = crate::look::LookDocV2::from_json(&json).unwrap();
        let again = crate::look::LookDocV2::from_json(&parsed.to_json()).unwrap();
        assert_eq!(again, parsed);
    }
}
