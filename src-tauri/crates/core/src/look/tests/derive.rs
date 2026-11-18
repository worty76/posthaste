use super::*;

mod derive_tests {
    use serde_json::Map;

    use super::doc;
    use crate::look::color::contrast;
    use crate::look::derive::{resolve, ResolvedLook};
    #[allow(unused_imports)]
    use crate::look::LookDocExt as _;
    use crate::look::{Base, Colors, FontFace, LookDoc, LookTheme, Shape, TEXT_CONTRAST_MIN};
    use crate::CoreError;

    fn midnight() -> LookDoc {
        LookDoc {
            format: "posthaste-look".into(),
            version: 2,
            name: "Midnight Grape".into(),
            author: Some("Posthaste".into()),
            theme: LookTheme {
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
                    extra: Map::new(),
                },
                shape: Shape::Round,
                font: FontFace::Rounded,
                font_family: None,
                extra: Map::new(),
            },
            scene: Default::default(),
            assets: Vec::new(),
            extra: Default::default(),
        }
    }

    fn tokens(d: &LookDoc) -> std::collections::BTreeMap<String, String> {
        resolve("test", &d.theme_edit()).unwrap().tokens
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
        let r: ResolvedLook = resolve("builtin:ink", &doc().theme_edit()).unwrap();
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
        d.theme.colors.danger = Some("#ff0000".into());
        d.theme.colors.ok = Some("#00ff00".into());
        d.theme.colors.bg_sunken = Some("#123456".into());
        let t = tokens(&d);
        assert_eq!(t.get("--danger").unwrap(), "#ff0000");
        assert_eq!(t.get("--danger-soft").unwrap(), "rgba(255, 0, 0, 0.12)");
        assert_eq!(t.get("--ok").unwrap(), "#00ff00");
        assert_eq!(t.get("--bg-sunken").unwrap(), "#123456");
    }

    #[test]
    fn colours_are_normalised_to_lowercase_six_digit_hex() {
        let mut d = doc();
        d.theme.colors.bg = "#FFF".into();
        d.theme.colors.accent = "#FF8FB1".into();
        let t = tokens(&d);
        assert_eq!(t.get("--bg").unwrap(), "#ffffff");
        assert_eq!(t.get("--accent").unwrap(), "#ff8fb1");
    }

    #[test]
    fn an_invalid_edit_is_rejected_without_tokens() {
        let mut d = doc();
        d.theme.colors.text = d.theme.colors.bg.clone();
        let err = resolve("test", &d.theme_edit()).unwrap_err();
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
                version: 2,
                name: "Property".into(),
                author: None,
                theme: LookTheme {
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
                        extra: Map::new(),
                    },
                    shape: Shape::Soft,
                    font: FontFace::System,
                    font_family: None,
                    extra: Map::new(),
                },
                scene: Default::default(),
                assets: Vec::new(),
                extra: Default::default(),
            };
            let Ok(r) = resolve("test", &d.theme_edit()) else {
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
            let again = resolve("test", &d.theme_edit()).unwrap();
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

mod font_tests {
    use super::doc;
    use crate::look::code::{decode_code, encode_code};
    use crate::look::derive::resolve;
    use crate::look::{validate_theme, FontFace};

    fn tokens_with(name: Option<&str>) -> std::collections::BTreeMap<String, String> {
        let mut edit = doc().theme_edit();
        edit.font = FontFace::Mono;
        edit.font_family = name.map(String::from);
        resolve("test", &edit).unwrap().tokens
    }

    #[test]
    fn a_custom_font_becomes_the_font_token() {
        let t = tokens_with(Some("JetBrains Mono"));
        assert_eq!(
            t.get("--font").unwrap(),
            "\"JetBrains Mono\", var(--font-mono-ui)"
        );
    }

    #[test]
    fn no_custom_font_no_font_token() {
        assert!(!tokens_with(None).contains_key("--font"));
    }

    #[test]
    fn a_custom_font_round_trips_in_a_share_code() {
        let mut d = doc();
        d.theme.font_family = Some("JetBrains Mono".into());
        let code = encode_code(&d);
        assert_eq!(decode_code(&code).unwrap(), d);
    }

    #[test]
    fn font_names_are_checked() {
        for name in [
            "Fira Code",
            "Noto Sans CJK JP",
            "Hiragino Kaku Gothic ProN",
            "游ゴシック",
        ] {
            let mut edit = doc().theme_edit();
            edit.font_family = Some(name.into());
            assert!(
                validate_theme(&edit).is_ok(),
                "{name} is a font name: {:?}",
                validate_theme(&edit)
            );
        }
        let long = "x".repeat(65);
        for (name, needle) in [
            ("", "can't be empty, or start or end with a space"),
            (" Inter", "can't be empty, or start or end with a space"),
            ("Inter ", "can't be empty, or start or end with a space"),
            (long.as_str(), "at most 64 characters"),
            (
                "Inter\"",
                "letters, numbers, spaces, hyphens, underscores and dots",
            ),
            (
                "Inter;",
                "letters, numbers, spaces, hyphens, underscores and dots",
            ),
            (
                "a\\b",
                "letters, numbers, spaces, hyphens, underscores and dots",
            ),
            (
                "x(y)",
                "letters, numbers, spaces, hyphens, underscores and dots",
            ),
            (
                "{x}",
                "letters, numbers, spaces, hyphens, underscores and dots",
            ),
            (
                "a,b",
                "letters, numbers, spaces, hyphens, underscores and dots",
            ),
            (
                "url(x)",
                "letters, numbers, spaces, hyphens, underscores and dots",
            ),
        ] {
            let mut edit = doc().theme_edit();
            edit.font_family = Some(name.into());
            let err = validate_theme(&edit).unwrap_err().to_string();
            assert!(err.contains(needle), "{name:?}: got {err}");
        }
    }
}
