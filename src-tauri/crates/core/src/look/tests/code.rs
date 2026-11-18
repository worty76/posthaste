use super::*;

mod one_format_code_tests {
    use super::OLD_VERSION;
    use crate::look::code::decode_code;

    #[test]
    fn a_version_1_code_is_not_a_look_code() {
        let code = format!("posthaste-look:{}:abcd", OLD_VERSION);
        let err = decode_code(&code).unwrap_err().to_string();
        assert!(
            err.contains(r#"It should start with "posthaste-look:2:""#),
            "got: {err}"
        );
    }

    #[test]
    fn a_newer_code_says_update() {
        let err = decode_code("posthaste-look:3:abcd")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(
                "This look was made with a newer version of Posthaste. Update the app to use it."
            ),
            "got: {err}"
        );
    }
}

mod code_tests {
    use serde_json::{json, Map};

    use super::{asset_id, doc, listed_asset};
    use crate::look::code::{b64_decode, b64_encode, decode_code, encode_code, CODE_PREFIX};
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;
    use crate::look::{LayerRecord, WindowScene};

    fn coded() -> String {
        encode_code(&doc())
    }

    #[test]
    fn base64_round_trips_every_length_up_to_forty() {
        let bytes: Vec<u8> = (0..=255u8).cycle().take(41).collect();
        for len in 0..=40usize {
            let input = &bytes[..len];
            let encoded = b64_encode(input);
            assert!(
                !encoded.contains('+') && !encoded.contains('/') && !encoded.contains('='),
                "url-safe without padding: {encoded:?}"
            );
            assert_eq!(b64_decode(&encoded).as_deref(), Some(input), "length {len}");
        }
    }

    #[test]
    fn base64_rejects_wrong_lengths_and_the_standard_alphabet() {
        assert_eq!(b64_decode("a"), None);
        assert_eq!(b64_decode("a+b"), None);
    }

    #[test]
    fn a_code_encodes_and_decodes_back_to_the_same_document() {
        let code = coded();
        assert!(code.starts_with(CODE_PREFIX));
        assert_eq!(decode_code(&code).unwrap(), doc());
    }

    #[test]
    fn a_code_round_trips_a_document_with_a_scene_and_its_assets() {
        let mut d = doc();
        d.scene.all = Some(WindowScene {
            backdrop: vec![LayerRecord {
                kind: "image".into(),
                v: 1,
                on: true,
                config: json!({"asset": asset_id(), "dim": 0.45}),
                extra: Map::new(),
            }],
            margin: Vec::new(),
            extra: Map::new(),
        });
        d.assets = vec![listed_asset(&asset_id())];
        let code = encode_code(&d);
        assert!(code.starts_with(CODE_PREFIX));
        assert_eq!(decode_code(&code).unwrap(), d);
    }

    #[test]
    fn the_code_survives_what_chat_apps_do_to_text() {
        let code = coded();
        let mut wrapped = String::new();
        for (i, c) in code.chars().enumerate() {
            wrapped.push(c);
            if (i + 1) % 20 == 0 {
                wrapped.push(' ');
            }
            if (i + 1) % 40 == 0 {
                wrapped.push('\n');
            }
        }
        assert_eq!(decode_code(&wrapped).unwrap(), doc());
        assert_eq!(decode_code(&format!("\"{code}\"")).unwrap(), doc());
        assert_eq!(decode_code(&format!("`{code}`")).unwrap(), doc());
        assert_eq!(decode_code(&format!("  \n{code}  \n")).unwrap(), doc());
        // A raw document pasted as JSON reads the same way.
        assert_eq!(decode_code(&doc().to_json()).unwrap(), doc());
    }

    #[test]
    fn a_cut_off_code_reports_incomplete() {
        let cut = &coded()[..coded().len() - 10];
        let err = decode_code(cut).unwrap_err().to_string();
        assert!(err.contains("incomplete"), "got: {err}");
    }

    #[test]
    fn something_else_entirely_reports_the_missing_prefix() {
        let err = decode_code("hello").unwrap_err().to_string();
        assert!(err.contains("posthaste-look:2:"), "got: {err}");
    }

    #[test]
    fn an_oversized_input_is_too_large() {
        let big = format!("{CODE_PREFIX}{}", "A".repeat(30_000));
        let err = decode_code(&big).unwrap_err().to_string();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn a_code_for_an_unreadable_look_shows_the_validation_message() {
        let mut bad = doc();
        bad.theme.colors.text = bad.theme.colors.bg.clone();
        let err = decode_code(&encode_code(&bad)).unwrap_err().to_string();
        assert!(err.contains("too similar to read"), "got: {err}");
        assert!(!err.contains("incomplete"), "got: {err}");
    }
}

mod bundled_tests {
    use crate::look::bundled::bundled;
    use crate::look::code::{decode_code, encode_code};
    use crate::look::derive::resolve;
    use crate::look::LookDocExt as _;

    #[test]
    fn the_bundled_looks_ship_in_gallery_order() {
        let looks = bundled();
        let ids: Vec<&str> = looks.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "builtin:ink",
                "builtin:strawberry-milk",
                "builtin:matcha",
                "builtin:lavender-haze",
                "builtin:midnight-grape",
                "builtin:blueberry",
                "builtin:sakura-night",
                "builtin:paper",
            ]
        );
    }

    #[test]
    fn every_bundled_look_is_valid_resolves_and_has_a_unique_name() {
        let looks = bundled();
        for (id, d) in &looks {
            if let Err(e) = d.validate() {
                panic!("{id} fails validation: {e}");
            }
            if let Err(e) = resolve(id, &d.theme_edit()) {
                panic!("{id} fails to resolve: {e}");
            }
        }
        let mut names: Vec<String> = looks.iter().map(|(_, d)| d.theme_edit().name).collect();
        names.sort();
        let unique = names.len() == names.iter().len() && names.windows(2).all(|w| w[0] != w[1]);
        assert!(unique, "names are unique: {names:?}");
    }

    #[test]
    fn the_ink_accent_stays_dark_enough_for_its_own_text() {
        let ink = bundled()
            .into_iter()
            .find(|(id, _)| id == "builtin:ink")
            .map(|(_, d)| d)
            .expect("ink ships with the app");
        let tokens = resolve("builtin:ink", &ink.theme_edit()).unwrap().tokens;
        assert_eq!(tokens.get("--on-accent").unwrap(), "#0f0f0f");
    }

    #[test]
    fn every_bundled_look_survives_its_share_code() {
        for (id, d) in bundled() {
            assert_eq!(decode_code(&encode_code(&d)).unwrap(), d, "{id}");
        }
    }

    #[test]
    fn the_demo_looks_keep_their_stickers_until_stickers_ship() {
        // Stickers are not shipped yet: the demo looks carry theirs in
        // the document, and nothing draws them.
        let looks = bundled();
        for (id, doc) in &looks {
            if !id.contains("sakura") && !id.contains("paper") {
                continue;
            }
            assert!(doc.scene.layer_count() > 0, "{id} keeps its stickers");
            let resolved = crate::look::resolve::resolve_scene(doc);
            assert!(resolved.main.margin.is_empty(), "{id}");
            assert!(resolved.capture.margin.is_empty(), "{id}");
            assert!(resolved.today.margin.is_empty(), "{id}");
            assert!(resolved.details.margin.is_empty(), "{id}");
            assert!(resolved.details.backdrop.is_empty(), "{id} ships no images");
        }
    }
}
