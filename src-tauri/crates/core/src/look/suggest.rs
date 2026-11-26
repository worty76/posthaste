//! Palette suggestions, one-accent harmony generation, and one-click WCAG AA
//! contrast repair for the Look Editor. Every generated or curated palette is
//! guaranteed to pass Posthaste's readability checks.

use posthaste_model::{Base, Colors, PaletteMood, PalettePreset};

use super::color::{contrast, hsl_to_rgb, rgb_to_hsl, Rgb};
use super::{parse_color, ACCENT_FILL_MIN, TEXT_CONTRAST_MIN};
use crate::error::Result;

/// Curated, guaranteed-AA starting palettes for the given base.
pub fn presets(base: Base) -> Vec<PalettePreset> {
    let entries: &[(&str, &str, &str, &str, &str, &str)] = match base {
        Base::Dark => &[
            (
                "midnight", "Midnight", "#0f1422", "#171f33", "#e8eeff", "#6ea8ff",
            ),
            (
                "espresso", "Espresso", "#161210", "#211b18", "#f3ebe4", "#e39c64",
            ),
            (
                "forest", "Forest", "#0f1713", "#16231c", "#e7f3ec", "#63c78d",
            ),
            ("amber", "Amber", "#12110e", "#1d1b15", "#f5eedc", "#f0b44c"),
            ("slate", "Slate", "#121417", "#1b1f24", "#eceff4", "#94a3b8"),
            ("rose", "Rose", "#171016", "#231822", "#f6e9f2", "#f48bb6"),
        ],
        Base::Light => &[
            ("paper", "Paper", "#f6f5f2", "#ffffff", "#1d1d1b", "#2d2d2a"),
            ("sage", "Sage", "#f1f5f0", "#ffffff", "#233024", "#4f8a5b"),
            (
                "sandstone",
                "Sandstone",
                "#f7f2ea",
                "#fffdf9",
                "#33291f",
                "#c9743a",
            ),
            ("sky", "Sky", "#eff4fa", "#ffffff", "#1d2939", "#3b82f6"),
            (
                "lavender", "Lavender", "#f4f1fa", "#ffffff", "#2a2338", "#7c65d6",
            ),
            ("peach", "Peach", "#faf2ee", "#fffcfa", "#382420", "#d96b52"),
        ],
    };
    entries
        .iter()
        .map(|&(id, name, bg, surface, text, accent)| PalettePreset {
            id: id.into(),
            name: name.into(),
            base,
            colors: Colors {
                bg: bg.into(),
                surface: surface.into(),
                text: text.into(),
                accent: accent.into(),
                ..Colors::default()
            },
        })
        .collect()
}

/// Derives a harmonious, WCAG-AA four-colour palette from a single accent hex,
/// a light/dark base, and a neutral tinting mood.
pub fn from_accent(accent_hex: &str, base: Base, mood: PaletteMood) -> Result<Colors> {
    let raw_accent = parse_color("accent", accent_hex)?;
    let (h, s, l) = rgb_to_hsl(raw_accent);

    let (neutral_h, bg_s, surf_s, text_s) = match mood {
        PaletteMood::Neutral => (h, 0.02, 0.02, 0.02),
        PaletteMood::Tinted => {
            let tint = (s * 0.24).clamp(0.06, 0.20);
            (h, tint, (tint * 1.15).min(0.24), (tint * 0.6).min(0.14))
        }
        PaletteMood::Warm => (30.0, 0.14, 0.16, 0.10),
    };

    let (bg, surface, mut text, mut accent) = match base {
        Base::Dark => {
            let bg = hsl_to_rgb(neutral_h, bg_s, 0.07);
            let surface = hsl_to_rgb(neutral_h, surf_s, 0.115);
            let text = hsl_to_rgb(neutral_h, text_s, 0.93);
            let accent_l = l.clamp(0.56, 0.78);
            let accent = hsl_to_rgb(h, s, accent_l);
            (bg, surface, text, accent)
        }
        Base::Light => {
            let bg = hsl_to_rgb(neutral_h, bg_s, 0.955);
            let surface = hsl_to_rgb(neutral_h, surf_s * 0.35, 0.995);
            let text = hsl_to_rgb(neutral_h, text_s, 0.14);
            let accent_l = l.clamp(0.32, 0.56);
            let accent = hsl_to_rgb(h, s, accent_l);
            (bg, surface, text, accent)
        }
    };

    // Guarantee generous readability even for extreme hue/saturation inputs.
    text = ensure_text_contrast(text, bg, surface, base, 7.0);
    accent = ensure_accent_contrast(accent, bg, base, 2.2);

    Ok(Colors {
        bg: bg.to_hex(),
        surface: surface.to_hex(),
        text: text.to_hex(),
        accent: accent.to_hex(),
        ..Colors::default()
    })
}

/// Generates six distinct, harmonious, WCAG-AA palettes built around a single
/// user-picked seed colour (`Tinted`, `Neutral`, `Warm`, `Analogous`,
/// `Complementary`, `Vibrant`).
pub fn around_color(seed_hex: &str, base: Base) -> Result<Vec<PalettePreset>> {
    let seed = parse_color("accent", seed_hex)?;
    let (h, s, l) = rgb_to_hsl(seed);

    let make = |id: &str, name: &str, bg_h: f64, bg_s: f64, acc_h: f64, acc_s: f64| {
        let (bg, surface, mut text, mut accent) = match base {
            Base::Dark => {
                let bg = hsl_to_rgb(bg_h, bg_s, 0.075);
                let surface = hsl_to_rgb(bg_h, (bg_s * 1.15).min(0.28), 0.12);
                let text = hsl_to_rgb(bg_h, (bg_s * 0.5).min(0.14), 0.93);
                let accent = hsl_to_rgb(acc_h, acc_s, l.clamp(0.58, 0.78));
                (bg, surface, text, accent)
            }
            Base::Light => {
                let bg = hsl_to_rgb(bg_h, bg_s, 0.955);
                let surface = hsl_to_rgb(bg_h, bg_s * 0.35, 0.995);
                let text = hsl_to_rgb(bg_h, (bg_s * 0.6).min(0.16), 0.14);
                let accent = hsl_to_rgb(acc_h, acc_s, l.clamp(0.32, 0.54));
                (bg, surface, text, accent)
            }
        };
        text = ensure_text_contrast(text, bg, surface, base, 7.0);
        accent = ensure_accent_contrast(accent, bg, base, 2.2);
        PalettePreset {
            id: format!("around:{id}"),
            name: name.into(),
            base,
            colors: Colors {
                bg: bg.to_hex(),
                surface: surface.to_hex(),
                text: text.to_hex(),
                accent: accent.to_hex(),
                ..Colors::default()
            },
        }
    };

    let tint = (s * 0.24).clamp(0.08, 0.20);
    let deep = (s * 0.34).clamp(0.14, 0.26);
    let accent_sat = s.clamp(0.25, 0.88);

    Ok(vec![
        make("tinted", "Tinted", h, tint, h, s),
        make("neutral", "Neutral", h, 0.02, h, s),
        make("warm", "Warm", 30.0, 0.14, h, s),
        make("analogous", "Analogous", (h + 28.0) % 360.0, tint, h, s),
        make(
            "complement",
            "Contrast",
            (h + 180.0) % 360.0,
            tint * 0.85,
            h,
            s,
        ),
        make("deep", "Deep", h, deep, (h + 332.0) % 360.0, accent_sat),
    ])
}

/// Minimally nudges `text` and `accent` lightness so the palette passes
/// Posthaste's WCAG AA contrast checks while keeping the user's chosen hues.
pub fn fix_contrast(colors: &Colors, base: Base) -> Result<Colors> {
    let bg = parse_color("bg", &colors.bg)?;
    let surface = parse_color("surface", &colors.surface)?;
    let text = parse_color("text", &colors.text)?;
    let accent = parse_color("accent", &colors.accent)?;

    let fixed_text = ensure_text_contrast(text, bg, surface, base, TEXT_CONTRAST_MIN + 0.3);
    let fixed_accent = ensure_accent_contrast(accent, bg, base, ACCENT_FILL_MIN + 0.5);

    let mut next = colors.clone();
    next.text = fixed_text.to_hex();
    next.accent = fixed_accent.to_hex();
    Ok(next)
}

fn ensure_text_contrast(text: Rgb, bg: Rgb, surface: Rgb, base: Base, target: f64) -> Rgb {
    if contrast(text, bg) >= target && contrast(text, surface) >= target {
        return text;
    }
    let (h, s, start_l) = rgb_to_hsl(text);
    // Walk lightness away from the base first; if the user put a light bg under
    // Dark base (or vice versa), fall back to the opposite direction.
    let directions: [f64; 2] = match base {
        Base::Dark => [1.0, -1.0],
        Base::Light => [-1.0, 1.0],
    };
    for dir in directions {
        for step in 1..=100 {
            let l = (start_l + dir * (step as f64 / 100.0)).clamp(0.0, 1.0);
            let sat = s * (1.0 - (step as f64 / 120.0)).max(0.0);
            let candidate = hsl_to_rgb(h, sat, l);
            if contrast(candidate, bg) >= target && contrast(candidate, surface) >= target {
                return candidate;
            }
        }
    }
    // Pure white or pure black maximizes contrast against any background.
    let white = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let black = Rgb { r: 0, g: 0, b: 0 };
    let w_min = contrast(white, bg).min(contrast(white, surface));
    let b_min = contrast(black, bg).min(contrast(black, surface));
    if w_min >= b_min {
        white
    } else {
        black
    }
}

fn ensure_accent_contrast(accent: Rgb, bg: Rgb, base: Base, target: f64) -> Rgb {
    if contrast(accent, bg) >= target {
        return accent;
    }
    let (h, s, start_l) = rgb_to_hsl(accent);
    let directions: [f64; 2] = match base {
        Base::Dark => [1.0, -1.0],
        Base::Light => [-1.0, 1.0],
    };
    for dir in directions {
        for step in 1..=100 {
            let l = (start_l + dir * (step as f64 / 100.0)).clamp(0.0, 1.0);
            let candidate = hsl_to_rgb(h, s, l);
            if contrast(candidate, bg) >= target {
                return candidate;
            }
        }
    }
    accent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::check_colors;

    #[test]
    fn every_curated_preset_passes_wcag_aa() {
        for base in [Base::Dark, Base::Light] {
            let list = presets(base);
            assert!(list.len() >= 6);
            for preset in list {
                assert!(
                    check_colors(&preset.colors).is_ok(),
                    "preset {} failed check: {:?}",
                    preset.name,
                    check_colors(&preset.colors)
                );
            }
        }
    }

    #[test]
    fn from_accent_passes_across_all_hues_bases_and_moods() {
        for hue in (0..360).step_by(10) {
            let accent = hsl_to_rgb(hue as f64, 0.72, 0.52).to_hex();
            for base in [Base::Dark, Base::Light] {
                for mood in [PaletteMood::Neutral, PaletteMood::Tinted, PaletteMood::Warm] {
                    let colors = from_accent(&accent, base, mood).unwrap();
                    assert!(
                        check_colors(&colors).is_ok(),
                        "hue {hue} {base:?} {mood:?} failed: {:?}",
                        check_colors(&colors)
                    );
                }
            }
        }
    }

    #[test]
    fn around_color_passes_wcag_aa_across_all_hues_and_bases() {
        for hue in (0..360).step_by(10) {
            let seed = hsl_to_rgb(hue as f64, 0.72, 0.52).to_hex();
            for base in [Base::Dark, Base::Light] {
                let list = around_color(&seed, base).unwrap();
                assert_eq!(list.len(), 6);
                for preset in list {
                    assert!(
                        check_colors(&preset.colors).is_ok(),
                        "hue {hue} {base:?} {} failed: {:?}",
                        preset.name,
                        check_colors(&preset.colors)
                    );
                }
            }
        }
    }

    #[test]
    fn fix_contrast_repairs_unreadable_dark_and_light_palettes() {
        let dark_bad = Colors {
            bg: "#121212".into(),
            surface: "#1a1a1a".into(),
            text: "#222222".into(),
            accent: "#141414".into(),
            ..Colors::default()
        };
        assert!(check_colors(&dark_bad).is_err());
        let dark_fixed = fix_contrast(&dark_bad, Base::Dark).unwrap();
        assert!(check_colors(&dark_fixed).is_ok());

        let light_bad = Colors {
            bg: "#f5f5f5".into(),
            surface: "#ffffff".into(),
            text: "#dcdcdc".into(),
            accent: "#f0f0f0".into(),
            ..Colors::default()
        };
        assert!(check_colors(&light_bad).is_err());
        let light_fixed = fix_contrast(&light_bad, Base::Light).unwrap();
        assert!(check_colors(&light_fixed).is_ok());
    }
}
