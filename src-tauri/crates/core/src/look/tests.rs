//! Tests for the look module: colour maths, validation, derivation, share
//! codes, bundled looks, storage and the v2 format layer.

use serde_json::Map;

use crate::look::{Base, Colors, FontFace, LookDoc, Shape};
#[allow(unused_imports)]
use crate::look::{LookDocV2, LookTheme};

/// The Strawberry Milk look from the design spec, as a document.
fn doc() -> LookDoc {
    LookDoc {
        format: "posthaste-look".into(),
        version: 1,
        name: "Strawberry Milk".into(),
        author: Some("Posthaste".into()),
        base: Base::Light,
        colors: Colors {
            bg: "#fff5f7".into(),
            surface: "#ffffff".into(),
            text: "#4a2c3a".into(),
            accent: "#ff8fb1".into(),
            bg_sunken: None,
            surface_raised: None,
            danger: None,
            ok: None,
            extra: Map::new(),
        },
        shape: Shape::Round,
        font: FontFace::Rounded,
    }
}

/// The same theme in a v2 wrapper: no scene, no assets, no extra fields.
fn v2doc() -> LookDocV2 {
    LookDocV2 {
        format: "posthaste-look".into(),
        version: 2,
        name: "Strawberry Milk".into(),
        author: Some("Posthaste".into()),
        theme: LookTheme {
            base: Base::Light,
            colors: crate::look::Colors {
                bg: "#fff5f7".into(),
                surface: "#ffffff".into(),
                text: "#4a2c3a".into(),
                accent: "#ff8fb1".into(),
                bg_sunken: None,
                surface_raised: None,
                danger: None,
                ok: None,
                extra: serde_json::Map::new(),
            },
            shape: Shape::Round,
            font: FontFace::Rounded,
            extra: Default::default(),
        },
        scene: Default::default(),
        assets: Vec::new(),
        extra: Default::default(),
    }
}

mod color_tests {
    use crate::look::color::{contrast, mix, Rgb};

    #[test]
    fn parses_three_digit_and_mixed_case_hex() {
        assert_eq!(
            Rgb::parse("#fff"),
            Some(Rgb {
                r: 255,
                g: 255,
                b: 255
            })
        );
        assert_eq!(
            Rgb::parse("#FF8fB1"),
            Some(Rgb {
                r: 255,
                g: 143,
                b: 177
            })
        );
    }

    #[test]
    fn rejects_everything_but_hex_short_or_long() {
        for bad in [
            "",
            "fff",
            "#ffff",
            "#ggg",
            "#fff5f780",
            "rgb(1,2,3)",
            "red",
            "#fff ",
            "url(x)",
        ] {
            assert_eq!(Rgb::parse(bad), None, "expected {bad:?} to be rejected");
        }
    }

    #[test]
    fn white_on_black_is_21_to_1_and_contrast_is_symmetric() {
        let white = Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        let black = Rgb { r: 0, g: 0, b: 0 };
        assert!((contrast(white, black) - 21.0).abs() < 1e-9);
        let pink = Rgb {
            r: 255,
            g: 143,
            b: 177,
        };
        assert_eq!(contrast(pink, pink), 1.0);
        assert_eq!(contrast(white, black), contrast(black, white));
    }

    #[test]
    fn mixing_halfway_between_black_and_white_gives_128() {
        let black = Rgb { r: 0, g: 0, b: 0 };
        let white = Rgb {
            r: 255,
            g: 255,
            b: 255,
        };
        assert_eq!(
            mix(black, white, 0.5),
            Rgb {
                r: 128,
                g: 128,
                b: 128
            }
        );
        assert_eq!(mix(black, white, 0.0), black);
        assert_eq!(mix(black, white, 1.0), white);
    }

    #[test]
    fn formats_as_hex_and_rgba() {
        let pink = Rgb {
            r: 255,
            g: 143,
            b: 177,
        };
        assert_eq!(pink.to_hex(), "#ff8fb1");
        assert_eq!(
            Rgb {
                r: 74,
                g: 44,
                b: 58
            }
            .to_rgba(0.045),
            "rgba(74, 44, 58, 0.045)"
        );
        assert_eq!(Rgb { r: 0, g: 0, b: 0 }.to_rgba(0.1), "rgba(0, 0, 0, 0.1)");
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

mod derive_tests {
    use super::doc;
    use crate::look::color::contrast;
    use crate::look::derive::{resolve, ResolvedLook};
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;
    use crate::look::{Base, Colors, FontFace, LookDoc, Shape, TEXT_CONTRAST_MIN};
    use crate::CoreError;

    fn midnight() -> LookDoc {
        LookDoc {
            format: "posthaste-look".into(),
            version: 1,
            name: "Midnight Grape".into(),
            author: Some("Posthaste".into()),
            base: Base::Dark,
            colors: Colors {
                bg: "#1a1423".into(),
                surface: "#241b31".into(),
                text: "#f1e9ff".into(),
                accent: "#c58bff".into(),
                bg_sunken: None,
                surface_raised: None,
                danger: None,
                ok: None,
                extra: serde_json::Map::new(),
            },
            shape: Shape::Round,
            font: FontFace::Rounded,
        }
    }

    fn tokens(d: &LookDoc) -> std::collections::BTreeMap<String, String> {
        resolve("test", d).unwrap().tokens
    }

    #[test]
    fn the_light_golden_look_derives_exactly_these_tokens() {
        let t = tokens(&doc());
        let expected: [(&str, &str); 15] = [
            ("--bg", "#fff5f7"),
            ("--surface", "#ffffff"),
            ("--text", "#4a2c3a"),
            ("--accent", "#ff8fb1"),
            ("--bg-sunken", "#f5ebed"),
            ("--surface-raised", "#ffffff"),
            ("--text-muted", "#806873"),
            ("--text-faint", "#9d8891"),
            ("--border", "rgba(74, 44, 58, 0.1)"),
            ("--border-strong", "rgba(74, 44, 58, 0.18)"),
            ("--hover", "rgba(74, 44, 58, 0.045)"),
            ("--active", "rgba(74, 44, 58, 0.08)"),
            ("--accent-ink", "#96566c"),
            ("--accent-soft", "rgba(255, 143, 177, 0.1)"),
            ("--on-accent", "#4a2c3a"),
        ];
        assert_eq!(t.len(), expected.len(), "exactly the derived tokens");
        for (k, v) in expected {
            assert_eq!(t.get(k), Some(&v.to_string()), "{k}");
        }
    }

    #[test]
    fn the_dark_golden_look_derives_exactly_these_tokens() {
        let t = tokens(&midnight());
        let expected: [(&str, &str); 15] = [
            ("--bg", "#1a1423"),
            ("--surface", "#241b31"),
            ("--text", "#f1e9ff"),
            ("--accent", "#c58bff"),
            ("--bg-sunken", "#15101d"),
            ("--surface-raised", "#2b2237"),
            ("--text-muted", "#9f98ab"),
            ("--text-faint", "#746d7f"),
            ("--border", "rgba(241, 233, 255, 0.08)"),
            ("--border-strong", "rgba(241, 233, 255, 0.14)"),
            ("--hover", "rgba(241, 233, 255, 0.05)"),
            ("--active", "rgba(241, 233, 255, 0.08)"),
            ("--accent-ink", "#c58bff"),
            ("--accent-soft", "rgba(197, 139, 255, 0.14)"),
            ("--on-accent", "#1a1423"),
        ];
        assert_eq!(t.len(), expected.len(), "exactly the derived tokens");
        for (k, v) in expected {
            assert_eq!(t.get(k), Some(&v.to_string()), "{k}");
        }
    }

    #[test]
    fn the_resolved_look_carries_its_identity() {
        let r: ResolvedLook = resolve("builtin:ink", &doc()).unwrap();
        assert_eq!(r.id, "builtin:ink");
        assert_eq!(r.name, "Strawberry Milk");
        assert_eq!(r.author.as_deref(), Some("Posthaste"));
        assert_eq!(r.base, Base::Light);
        assert_eq!(r.shape, Shape::Round);
        assert_eq!(r.font, FontFace::Rounded);
    }

    #[test]
    fn optional_colours_only_appear_when_overridden() {
        let d = doc();
        let t = tokens(&d);
        for absent in ["--danger", "--danger-soft", "--ok"] {
            assert!(!t.contains_key(absent), "{absent} without an override");
        }

        let mut d = doc();
        d.colors.danger = Some("#ff0000".into());
        d.colors.ok = Some("#00ff00".into());
        d.colors.bg_sunken = Some("#123456".into());
        let t = tokens(&d);
        assert_eq!(t.get("--danger").unwrap(), "#ff0000");
        assert_eq!(t.get("--danger-soft").unwrap(), "rgba(255, 0, 0, 0.12)");
        assert_eq!(t.get("--ok").unwrap(), "#00ff00");
        assert_eq!(t.get("--bg-sunken").unwrap(), "#123456");
    }

    #[test]
    fn colours_are_normalised_to_lowercase_six_digit_hex() {
        let mut d = doc();
        d.colors.bg = "#FFF".into();
        d.colors.accent = "#FF8FB1".into();
        let t = tokens(&d);
        assert_eq!(t.get("--bg").unwrap(), "#ffffff");
        assert_eq!(t.get("--accent").unwrap(), "#ff8fb1");
    }

    #[test]
    fn an_invalid_document_is_rejected_without_tokens() {
        let mut d = doc();
        d.colors.text = d.colors.bg.clone();
        let err = resolve("test", &d).unwrap_err();
        assert!(matches!(err, CoreError::InvalidLook(_)));
    }

    /// A small linear congruential generator so the property test is
    /// deterministic without a `rand` dependency.
    struct Lcg(u64);

    impl Lcg {
        fn next_u32(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) as u32
        }

        /// A channel value in `lo..lo+span`.
        fn channel(&mut self, lo: u8, span: u8) -> u8 {
            lo + (self.next_u32() % u32::from(span)) as u8
        }

        fn color(&mut self, lo: u8, span: u8) -> [u8; 3] {
            [
                self.channel(lo, span),
                self.channel(lo, span),
                self.channel(lo, span),
            ]
        }
    }

    fn hex(c: [u8; 3]) -> String {
        format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
    }

    #[test]
    fn derived_tokens_stay_readable_for_300_generated_colour_sets() {
        let mut rng = Lcg(0x5EED_1234_ABCD_0001);
        let mut checked = 0;
        for i in 0..300 {
            let dark = i % 2 == 1;
            let (bg_lo, bg_span, text_lo, text_span) = if dark {
                (0, 40, 210, 46)
            } else {
                (216, 40, 0, 70)
            };
            let bg = rng.color(bg_lo, bg_span);
            let text = rng.color(text_lo, text_span);
            let mut surface = [
                bg[0].saturating_add(rng.channel(0, 8)),
                bg[1].saturating_add(rng.channel(0, 8)),
                bg[2].saturating_add(rng.channel(0, 8)),
            ];
            if !dark {
                // A light surface must stay bright or text contrast breaks.
                surface[0] = surface[0].max(bg[0]);
                surface[1] = surface[1].max(bg[1]);
                surface[2] = surface[2].max(bg[2]);
            }
            let accent = rng.color(0, 255);
            let d = LookDoc {
                format: "posthaste-look".into(),
                version: 1,
                name: "Property".into(),
                author: None,
                base: if dark { Base::Dark } else { Base::Light },
                colors: Colors {
                    bg: hex(bg),
                    surface: hex(surface),
                    text: hex(text),
                    accent: hex(accent),
                    bg_sunken: None,
                    surface_raised: None,
                    danger: None,
                    ok: None,
                    extra: serde_json::Map::new(),
                },
                shape: Shape::Soft,
                font: FontFace::System,
            };
            let Ok(r) = resolve("test", &d) else {
                continue; // accent or surface out of range: not a valid look
            };
            checked += 1;
            let get = |k: &str| r.tokens.get(k).unwrap().clone();
            let hex_of =
                |s: &str| crate::look::color::Rgb::parse(s).expect("tokens are parsed colours");
            let muted = hex_of(&get("--text-muted"));
            let ink = hex_of(&get("--accent-ink"));
            assert!(
                contrast(muted, bg_rgb(&r)) >= TEXT_CONTRAST_MIN,
                "muted on bg, set {i}"
            );
            assert!(
                contrast(muted, surface_rgb(&r)) >= TEXT_CONTRAST_MIN,
                "muted on surface, set {i}"
            );
            assert!(
                contrast(ink, bg_rgb(&r)) >= TEXT_CONTRAST_MIN,
                "ink on bg, set {i}"
            );
            assert!(
                contrast(ink, surface_rgb(&r)) >= TEXT_CONTRAST_MIN,
                "ink on surface, set {i}"
            );
            assert!(
                contrast(hex_of(&get("--on-accent")), hex_of(&get("--accent")))
                    >= TEXT_CONTRAST_MIN,
                "on-accent on accent, set {i}"
            );
            for v in r.tokens.values() {
                assert!(
                    v.starts_with('#') || v.starts_with("rgba("),
                    "token value {v:?}"
                );
            }
            let again = resolve("test", &d).unwrap();
            assert_eq!(again, r, "resolve is deterministic, set {i}");
        }
        assert!(checked > 0, "the generator produced valid sets");
    }

    fn bg_rgb(r: &ResolvedLook) -> crate::look::color::Rgb {
        crate::look::color::Rgb::parse(r.tokens.get("--bg").unwrap()).unwrap()
    }

    fn surface_rgb(r: &ResolvedLook) -> crate::look::color::Rgb {
        crate::look::color::Rgb::parse(r.tokens.get("--surface").unwrap()).unwrap()
    }
}

mod code_tests {
    use super::doc;
    use crate::look::code::{b64_decode, b64_encode, decode_code, encode_code, CODE_PREFIX};
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
            assert_eq!(
                decode_code(&encode_code(&d)).unwrap(),
                LookDocument::V1(d),
                "{id}"
            );
        }
    }
}

mod bundled_tests {
    use crate::look::bundled::bundled;
    use crate::look::code::{decode_code, encode_code};
    use crate::look::derive::resolve;
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;
    use crate::look::LookDocument;

    #[test]
    fn the_six_bundled_looks_ship_in_gallery_order() {
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
            if let Err(e) = resolve(id, d) {
                panic!("{id} fails to resolve: {e}");
            }
        }
        let mut names: Vec<&str> = looks.iter().map(|(_, d)| d.name.as_str()).collect();
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
        let tokens = resolve("builtin:ink", &ink).unwrap().tokens;
        assert_eq!(tokens.get("--on-accent").unwrap(), "#0f0f0f");
    }

    #[test]
    fn every_bundled_look_survives_its_share_code() {
        for (id, d) in bundled() {
            assert_eq!(
                decode_code(&encode_code(&d)).unwrap(),
                LookDocument::V1(d),
                "{id}"
            );
        }
    }
}

mod store_tests {
    use super::doc;
    use crate::error::CoreError;
    use crate::look::code::encode_code;
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;
    use crate::{Db, Settings};

    /// The Strawberry Milk look with a different name, so saved looks are
    /// distinguishable in the list.
    fn named(name: &str) -> crate::look::LookDoc {
        let mut d = doc();
        d.name = name.into();
        d
    }

    #[test]
    fn a_fresh_database_lists_the_six_bundled_looks() {
        let db = Db::open_in_memory().unwrap();
        let looks = db.list_looks().unwrap();
        assert_eq!(looks.len(), 6);
        assert!(looks.iter().all(|l| l.builtin));
        // Ink, the factory default, leads the gallery.
        assert_eq!(looks[0].id, crate::look::DEFAULT_LOOK_ID);
        assert_eq!(looks[1].id, "builtin:strawberry-milk");
    }

    #[test]
    fn saved_looks_get_user_ids_and_list_after_the_bundled_ones_newest_first() {
        let mut db = Db::open_in_memory().unwrap();
        let a = db.save_look_at(named("Alpha"), None, 1_000).unwrap();
        let b = db.save_look_at(named("Beta"), None, 2_000).unwrap();
        assert!(a.id.starts_with("user:"));
        assert!(b.id.starts_with("user:"));
        assert_ne!(a.id, b.id);

        let ids: Vec<String> = db.list_looks().unwrap().into_iter().map(|l| l.id).collect();
        assert_eq!(ids.len(), 8);
        assert_eq!(ids[6], b.id);
        assert_eq!(ids[7], a.id);
    }

    #[test]
    fn the_summary_preview_is_normalised() {
        let mut db = Db::open_in_memory().unwrap();
        let mut d = named("Upper");
        d.colors.bg = "#FFF".into();
        let s = db.save_look(d, None).unwrap();
        assert!(!s.builtin);
        assert_eq!(s.preview.bg, "#ffffff");
    }

    #[test]
    fn saving_with_an_existing_id_updates_in_place() {
        let mut db = Db::open_in_memory().unwrap();
        let s = db.save_look_at(named("First"), None, 1_000).unwrap();
        let updated = db
            .save_look_at(named("Second"), Some(&s.id), 2_000)
            .unwrap();
        assert_eq!(updated.id, s.id);
        assert_eq!(updated.name, "Second");
        let (created, updated_at): (i64, i64) = db
            .conn()
            .query_row(
                "SELECT created_at, updated_at FROM looks WHERE id = ?1",
                [&s.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((created, updated_at), (1_000, 2_000));
        assert_eq!(db.list_looks().unwrap().len(), 7);
    }

    #[test]
    fn saving_over_an_unreadable_stored_look_fails_and_keeps_the_row() {
        let mut db = Db::open_in_memory().unwrap();
        let broken = r#"{"version":2,"garbage":"#;
        db.conn()
            .execute(
                "INSERT INTO looks (id, doc, created_at, updated_at) VALUES ('user:broken', ?1, 1, 1)",
                [broken],
            )
            .unwrap();
        let result = db.save_look(named("New"), Some("user:broken"));
        assert!(result.is_err(), "the save must be refused");
        let doc: String = db
            .conn()
            .query_row("SELECT doc FROM looks WHERE id = 'user:broken'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(doc, broken, "the row must be left untouched");
    }

    #[test]
    fn updating_an_unknown_or_builtin_look_is_refused() {
        let mut db = Db::open_in_memory().unwrap();
        let d = named("X");
        assert!(matches!(
            db.save_look(d.clone(), Some("user:missing")),
            Err(CoreError::NotFound)
        ));
        assert!(matches!(
            db.save_look(d, Some("builtin:ink")),
            Err(CoreError::BuiltinLook)
        ));
    }

    #[test]
    fn an_invalid_document_is_rejected_and_the_list_is_unchanged() {
        let mut db = Db::open_in_memory().unwrap();
        let mut d = named("Bad");
        d.colors.text = d.colors.bg.clone();
        assert!(matches!(
            db.save_look(d, None),
            Err(CoreError::InvalidLook(_))
        ));
        assert_eq!(db.list_looks().unwrap().len(), 6);
    }

    #[test]
    fn the_fifty_look_limit_leaves_updates_alone() {
        let mut db = Db::open_in_memory().unwrap();
        let mut last = String::new();
        for i in 0..50 {
            last = db
                .save_look_at(named(&format!("Look {i}")), None, i as i64)
                .unwrap()
                .id;
        }
        assert!(matches!(
            db.save_look_at(named("One too many"), None, 100),
            Err(CoreError::TooManyLooks)
        ));
        db.save_look_at(named("Renamed"), Some(&last), 200).unwrap();
        assert_eq!(db.list_looks().unwrap().len(), 56);
    }

    #[test]
    fn deleting_removes_user_looks_and_refuses_builtins() {
        let mut db = Db::open_in_memory().unwrap();
        let s = db.save_look(named("Doomed"), None).unwrap();
        db.delete_look(&s.id).unwrap();
        assert!(matches!(db.look_doc(&s.id), Err(CoreError::NotFound)));
        assert!(matches!(
            db.delete_look("builtin:ink"),
            Err(CoreError::BuiltinLook)
        ));
        assert!(matches!(
            db.delete_look("user:gone"),
            Err(CoreError::NotFound)
        ));
    }

    #[test]
    fn the_active_look_follows_settings_and_falls_back_to_ink() {
        let mut db = Db::open_in_memory().unwrap();
        // Ink is the factory default look.
        let active = db.active_look().unwrap().expect("factory default resolves");
        assert_eq!(active.id, crate::look::DEFAULT_LOOK_ID);

        // A look that cannot be found lands on Ink, not on Plain.
        db.save_settings(&Settings {
            look_id: Some("user:missing".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            db.active_look().unwrap().map(|l| l.id),
            Some(crate::look::DEFAULT_LOOK_ID.into())
        );

        // Plain (no look) is a choice the user made, and is kept.
        db.save_settings(&Settings {
            look_id: None,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(db.active_look().unwrap(), None);

        let s = db.save_look(named("Mine"), None).unwrap();
        db.save_settings(&Settings {
            look_id: Some(s.id.clone()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(db.active_look().unwrap().map(|l| l.id), Some(s.id.clone()));
        db.delete_look(&s.id).unwrap();
        assert_eq!(
            db.active_look().unwrap().map(|l| l.id),
            Some(crate::look::DEFAULT_LOOK_ID.into())
        );
    }

    #[test]
    fn a_corrupt_stored_row_is_skipped_without_breaking_the_list() {
        let mut db = Db::open_in_memory().unwrap();
        db.save_look(named("Good"), None).unwrap();
        db.conn()
            .execute(
                "INSERT INTO looks (id, doc, created_at, updated_at)
                 VALUES ('user:broken', '{not json', 1, 1)",
                [],
            )
            .unwrap();
        let looks = db.list_looks().unwrap();
        assert_eq!(looks.len(), 7);
        assert!(looks.iter().all(|l| l.id != "user:broken"));

        db.save_settings(&Settings {
            look_id: Some("user:broken".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            db.active_look().unwrap().map(|l| l.id),
            Some(crate::look::DEFAULT_LOOK_ID.into())
        );
    }

    #[test]
    fn importing_a_code_stores_the_look_and_garbage_stores_nothing() {
        let mut db = Db::open_in_memory().unwrap();
        let s = db.import_look(&encode_code(&doc())).unwrap();
        assert!(s.id.starts_with("user:"));
        assert_eq!(s.name, "Strawberry Milk");
        assert!(matches!(
            db.import_look("hello"),
            Err(CoreError::InvalidLook(_))
        ));
        assert_eq!(db.list_looks().unwrap().len(), 7);
    }

    #[test]
    fn clear_all_removes_user_looks() {
        let mut db = Db::open_in_memory().unwrap();
        db.save_look(named("Mine"), None).unwrap();
        db.clear_all().unwrap();
        assert_eq!(db.list_looks().unwrap().len(), 6);
    }

    #[test]
    fn settings_default_to_ink_and_older_settings_adopt_it() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(
            db.load_settings().unwrap().look_id,
            Some(crate::look::DEFAULT_LOOK_ID.into())
        );
        // Settings saved before looks existed adopt the factory default…
        db.conn()
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('settings', '{\"theme\":\"dark\"}')",
                [],
            )
            .unwrap();
        assert_eq!(
            db.load_settings().unwrap().look_id,
            Some(crate::look::DEFAULT_LOOK_ID.into())
        );
        // …while an explicit "no look" (the user picked Plain) is kept.
        db.conn()
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('settings', '{\"theme\":\"dark\",\"look_id\":null}')",
                [],
            )
            .unwrap();
        assert_eq!(db.load_settings().unwrap().look_id, None);
    }
}

// ---- format v2 -------------------------------------------------------------

mod migrate_tests {
    use super::doc;
    use crate::look::bundled::bundled;
    use crate::look::derive::resolve;
    use crate::look::migrate::v1_to_v2;
    #[allow(unused_imports)]
    use crate::look::LookDocV2Ext as _;

    #[test]
    fn every_bundled_look_migrates_to_v2_and_resolves_identically() {
        for (id, v1) in bundled() {
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
            config: json!({"asset": "sha256:ab12", "dim": dim}),
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
                config: json!({"asset": "sha256:ab12", "sharpen": 4}),
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
            json!({"asset": "sha256:ab12", "sharpen": 4})
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
                config: json!({"asset": "sha256:ab12", "dim": 0.45}),
                extra: Map::new(),
            }],
            margin: Vec::new(),
            extra: Map::new(),
        });
        let code = encode_document(&LookDocument::V2(d.clone()));
        assert!(code.starts_with(CODE_PREFIX_V2));
        assert_eq!(decode_code(&code).unwrap(), LookDocument::V2(d));
    }
}

mod v2_store_tests {
    use serde_json::{json, Map};

    use super::{doc, v2doc};
    use crate::look::code::{decode_code, encode_code, encode_code_v2, encode_document};
    use crate::look::derive::resolve;
    use crate::look::{LayerRecord, LookDocV2, LookDocument, WindowScene};
    #[allow(unused_imports)]
    use crate::look::{LookDocExt as _, LookDocV2Ext as _};
    use crate::Db;

    /// A look written as if by a future build: an unknown top-level field,
    /// an unknown layer kind, and an unknown key inside a known layer.
    pub(super) const FUTURE_LOOK: &str = r##"{
      "format": "posthaste-look",
      "version": 2,
      "name": "Future build",
      "author": "Someone newer",
      "theme": {
        "base": "light",
        "colors": { "bg": "#fff5f7", "surface": "#ffffff", "text": "#4a2c3a", "accent": "#ff8fb1", "border": "#999999" },
        "shape": "round",
        "font": "rounded",
        "futureTheme": { "that": "stays" }
      },
      "scene": {
        "all": { "backdrop": [ { "kind": "image", "v": 1, "config": { "asset": "sha256:ab12", "fit": "cover", "dim": 0.45, "futureKey": "keep" } } ] },
        "capture": { "margin": [ { "kind": "shader", "v": 9, "on": false, "config": { "source": "spiral" }, "custom": "keep" } ] },
        "laterWindow": { "backdrop": [ { "kind": "mystery", "v": 1 } ] }
      },
      "assets": [ { "id": "sha256:ab12", "type": "image/png", "bytes": 412000, "width": 1920, "height": 1080, "future": true } ],
      "futureTop": { "still": "here" }
    }"##;

    fn scene_with_layers() -> WindowScene {
        WindowScene {
            backdrop: vec![LayerRecord {
                kind: "image".into(),
                v: 1,
                on: true,
                config: json!({"asset": "sha256:ab12", "dim": 0.45}),
                extra: Map::new(),
            }],
            margin: Vec::new(),
            extra: Map::new(),
        }
    }

    #[test]
    fn a_future_look_survives_parse_edit_save_export_and_reimport() {
        let mut db = Db::open_in_memory().unwrap();
        let mut parsed = LookDocV2::from_json(FUTURE_LOOK).unwrap();
        // The edit an editor makes: one colour.
        parsed.theme.colors.bg = "#eef2f7".into();
        parsed.validate().unwrap();

        // Save, export, and import into a fresh database.
        let saved = db
            .save_look_document(LookDocument::V2(parsed.clone()), None)
            .unwrap();
        let code = encode_document(&db.look_document(&saved.id).unwrap());
        let mut other = Db::open_in_memory().unwrap();
        let imported = other.import_look(&code).unwrap();
        let back = match other.look_document(&imported.id).unwrap() {
            LookDocument::V2(d) => d,
            _ => panic!("expected a v2 document"),
        };

        // The edited colour came through, and the whole document is
        // otherwise identical: every unknown part keeps its content.
        assert_eq!(back, parsed);
        assert_eq!(back.theme.colors.bg, "#eef2f7");
        assert_eq!(
            back.theme.colors.extra.get("border"),
            Some(&json!("#999999"))
        );
        assert_eq!(back.extra.get("futureTop"), Some(&json!({"still": "here"})));
        assert_eq!(
            back.theme.extra.get("futureTheme"),
            Some(&json!({"that": "stays"}))
        );
        let shader = &back.scene.capture.as_ref().unwrap().margin[0];
        assert_eq!(shader.kind, "shader");
        assert!(!shader.on);
        assert_eq!(shader.extra.get("custom"), Some(&json!("keep")));
        let image = &back.scene.all.as_ref().unwrap().backdrop[0];
        assert_eq!(image.config.get("futureKey"), Some(&json!("keep")));
        assert_eq!(back.assets[0].extra.get("future"), Some(&true.into()));
    }

    #[test]
    fn saving_a_v1_look_stores_a_v2_document_that_resolves_identically() {
        let mut db = Db::open_in_memory().unwrap();
        let s = db.save_look(doc(), None).unwrap();
        let stored = match db.look_document(&s.id).unwrap() {
            LookDocument::V2(d) => d,
            _ => panic!("expected the stored copy to be v2"),
        };
        assert_eq!(
            resolve(&s.id, &doc()).unwrap(),
            resolve(&s.id, &stored.v1_view()).unwrap()
        );
    }

    #[test]
    fn editing_an_imported_v2_look_from_the_editor_keeps_its_scene() {
        let mut d = v2doc();
        d.scene.all = Some(scene_with_layers());
        let mut db = Db::open_in_memory().unwrap();
        let imported = db.import_look(&encode_code_v2(&d)).unwrap();

        // The editor only knows the v1 view: it renames and saves back.
        let mut edited = db.look_doc(&imported.id).unwrap();
        edited.name = "Renamed".into();
        db.save_look(edited, Some(&imported.id)).unwrap();

        let back = match db.look_document(&imported.id).unwrap() {
            LookDocument::V2(d) => d,
            _ => panic!("expected v2"),
        };
        assert_eq!(back.name, "Renamed");
        assert_eq!(back.scene.all.as_ref().unwrap().backdrop[0].kind, "image");
    }

    #[test]
    fn updating_a_stored_v1_look_migrates_it_to_v2() {
        let mut db = Db::open_in_memory().unwrap();
        db.conn()
            .execute(
                "INSERT INTO looks (id, doc, created_at, updated_at) VALUES ('user:v1', ?1, 1, 1)",
                [doc().to_json()],
            )
            .unwrap();
        let mut edited = doc();
        edited.name = "Updated".into();
        db.save_look_at(edited, Some("user:v1"), 2).unwrap();
        match db.look_document("user:v1").unwrap() {
            LookDocument::V2(d) => assert_eq!(d.name, "Updated"),
            _ => panic!("expected the save to migrate the row to v2"),
        }
    }

    #[test]
    fn a_stored_v1_row_still_lists_and_resolves_until_its_next_save() {
        let db = Db::open_in_memory().unwrap();
        db.conn()
            .execute(
                "INSERT INTO looks (id, doc, created_at, updated_at) VALUES ('user:v1', ?1, 1, 1)",
                [doc().to_json()],
            )
            .unwrap();
        let looks = db.list_looks().unwrap();
        assert!(looks.iter().any(|l| l.id == "user:v1"));
        assert!(matches!(
            db.look_document("user:v1").unwrap(),
            LookDocument::V1(_)
        ));
        assert_eq!(
            resolve("user:v1", &db.look_doc("user:v1").unwrap()).unwrap(),
            resolve("user:v1", &doc()).unwrap()
        );
    }

    #[test]
    fn importing_a_v1_code_stores_v2_and_a_v2_code_stays_v2() {
        let mut db = Db::open_in_memory().unwrap();
        let from_v1 = db.import_look(&encode_code(&doc())).unwrap();
        assert!(matches!(
            db.look_document(&from_v1.id).unwrap(),
            LookDocument::V2(_)
        ));
        let mut d = v2doc();
        d.scene.all = Some(scene_with_layers());
        let from_v2 = db.import_look(&encode_code_v2(&d)).unwrap();
        match db.look_document(&from_v2.id).unwrap() {
            LookDocument::V2(stored) => assert!(stored.scene.all.is_some()),
            _ => panic!("expected v2"),
        }
        // Both resolve to the same tokens as the v1 original.
        assert_eq!(
            resolve(&from_v1.id, &doc()).unwrap().tokens,
            resolve(&from_v2.id, &v2doc().v1_view()).unwrap().tokens
        );
    }

    #[test]
    fn a_cut_v2_code_reports_incomplete() {
        let mut d = v2doc();
        d.scene.all = Some(scene_with_layers());
        let code = encode_code_v2(&d);
        let cut = &code[..code.len() - 10];
        let err = decode_code(cut).unwrap_err().to_string();
        assert!(err.contains("incomplete"), "got: {err}");
    }

    #[test]
    fn a_document_with_a_v2_scene_survives_a_decode_cycle_verbatim() {
        let raw =
            decode_code(&encode_code_v2(&LookDocV2::from_json(FUTURE_LOOK).unwrap())).unwrap();
        let original = LookDocument::V2(LookDocV2::from_json(FUTURE_LOOK).unwrap());
        assert_eq!(raw, original);
    }
}

// ---- Part A review fixes ----------------------------------------------------

mod part_a_tests {
    use serde_json::Map;

    use super::v2_store_tests::FUTURE_LOOK;
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

// ---- the emoji sticker kind -------------------------------------------------

mod emoji_tests {
    use serde_json::{json, Map, Value};

    use super::v2doc;
    use crate::error::CoreError;
    #[allow(unused_imports)]
    use crate::look::LookDocV2Ext as _;
    use crate::look::{Known, LayerRecord, LookDocV2, WindowScene};

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

    fn with_margin(layer: LayerRecord) -> LookDocV2 {
        let mut d = v2doc();
        d.scene.capture = Some(WindowScene {
            backdrop: Vec::new(),
            margin: vec![layer],
            extra: Map::new(),
        });
        d
    }

    fn assert_invalid(d: &LookDocV2, needle: &str) {
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
        let mut d = v2doc();
        d.scene.all = Some(WindowScene {
            backdrop: vec![sticker("🌸", "top-left")],
            margin: Vec::new(),
            extra: Map::new(),
        });
        assert_invalid(&d, "margin");
    }

    #[test]
    fn seven_stickers_in_one_window_are_refused() {
        let mut d = v2doc();
        d.scene.capture = Some(WindowScene {
            backdrop: Vec::new(),
            margin: (0..7).map(|_| sticker("🌸", "top-left")).collect(),
            extra: Map::new(),
        });
        assert_invalid(&d, "6 stickers");
    }

    #[test]
    fn joined_and_variant_emoji_are_accepted() {
        let mut d = v2doc();
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
        let mut d = v2doc();
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
