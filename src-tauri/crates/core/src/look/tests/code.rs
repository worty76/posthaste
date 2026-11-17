use super::*;

mod code_tests {
    use super::doc;
    use crate::look::code::{
        b64_decode, b64_encode, decode_code, encode_code, encode_document, CODE_PREFIX,
    };
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;
    use crate::look::LookDocument;

    fn coded() -> String {
        encode_code(&doc())
    }

    /// A v1 code decodes to the v1 document it names.
    fn decoded_v1(code: &str) -> LookDocument {
        decode_code(code).unwrap()
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
        assert_eq!(decoded_v1(&code), LookDocument::V1(doc()));
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
        assert_eq!(decoded_v1(&wrapped), LookDocument::V1(doc()));
        assert_eq!(decoded_v1(&format!("\"{code}\"")), LookDocument::V1(doc()));
        assert_eq!(decoded_v1(&format!("`{code}`")), LookDocument::V1(doc()));
        assert_eq!(
            decoded_v1(&format!("  \n{code}  \n")),
            LookDocument::V1(doc())
        );
        // A raw document pasted as JSON reads the same way.
        assert_eq!(decoded_v1(&doc().to_json()), LookDocument::V1(doc()));
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
        assert!(err.contains("posthaste-look:1:"), "got: {err}");
    }

    #[test]
    fn a_code_from_a_newer_version_reports_just_that() {
        // Version 2 is this build's own format; version 3 is not.
        let err = decode_code("posthaste-look:3:AAAA")
            .unwrap_err()
            .to_string();
        assert!(err.contains("newer version"), "got: {err}");
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
        bad.colors.text = bad.colors.bg.clone();
        let err = decode_code(&encode_code(&bad)).unwrap_err().to_string();
        assert!(err.contains("too similar to read"), "got: {err}");
        assert!(!err.contains("incomplete"), "got: {err}");
    }

    #[test]
    fn every_bundled_look_round_trips_through_its_code() {
        for (id, d) in crate::look::bundled::bundled() {
            assert_eq!(decode_code(&encode_document(&d)).unwrap(), d, "{id}");
        }
    }
}

mod v2_code_tests {
    use serde_json::{json, Map};

    use super::{doc, v2doc};
    use crate::look::code::{decode_code, encode_code, encode_document, CODE_PREFIX_V2};
    #[allow(unused_imports)]
    use crate::look::LookDocV2Ext as _;
    use crate::look::{LayerRecord, LookDocument, WindowScene};

    #[test]
    fn a_v1_code_still_decodes_to_a_v1_document() {
        assert_eq!(
            decode_code(&encode_code(&doc())).unwrap(),
            LookDocument::V1(doc())
        );
    }

    #[test]
    fn a_v2_code_round_trips_the_whole_document() {
        let mut d = v2doc();
        d.scene.all = Some(WindowScene {
            backdrop: vec![LayerRecord {
                kind: "image".into(),
                v: 1,
                on: true,
                config: json!({"asset": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "dim": 0.45}),
                extra: Map::new(),
            }],
            margin: Vec::new(),
            extra: Map::new(),
        });
        d.assets = vec![super::listed_asset(&super::asset_id())];
        let code = encode_document(&LookDocument::V2(d.clone()));
        assert!(code.starts_with(CODE_PREFIX_V2));
        assert_eq!(decode_code(&code).unwrap(), LookDocument::V2(d));
    }
}

mod bundled_tests {
    use crate::look::bundled::bundled;
    use crate::look::code::{decode_code, encode_document};
    use crate::look::derive::resolve;
    use crate::look::LookDocument;

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
            if let Err(e) = resolve(id, &d.v1_view()) {
                panic!("{id} fails to resolve: {e}");
            }
        }
        let mut names: Vec<String> = looks.iter().map(|(_, d)| d.v1_view().name).collect();
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
        let tokens = resolve("builtin:ink", &ink.v1_view()).unwrap().tokens;
        assert_eq!(tokens.get("--on-accent").unwrap(), "#0f0f0f");
    }

    #[test]
    fn every_bundled_look_survives_its_share_code() {
        for (id, d) in bundled() {
            assert_eq!(decode_code(&encode_document(&d)).unwrap(), d, "{id}");
        }
    }

    #[test]
    fn the_demo_looks_keep_their_stickers_until_stickers_ship() {
        // Stickers are not shipped yet: the demo looks carry theirs in
        // the document, and nothing draws them.
        let looks = bundled();
        for (id, doc) in &looks {
            let LookDocument::V2(doc) = doc else {
                continue;
            };
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
