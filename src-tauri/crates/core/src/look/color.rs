//! Colour parsing and the small maths a look needs: WCAG contrast and
//! per-channel mixing. No other file in this crate touches colour values.

/// An sRGB colour. Only ever comes from `#rgb`/`#rrggbb` text via [`Rgb::parse`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Accepts `#rgb` or `#rrggbb`, any case. Nothing else parses, so a look
    /// file can never smuggle in a name, function or URL.
    pub fn parse(s: &str) -> Option<Rgb> {
        let hex = s.strip_prefix('#')?;
        if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let d: Vec<u8> = hex
            .bytes()
            .map(|b| (b as char).to_digit(16).unwrap_or(0) as u8)
            .collect();
        match d.len() {
            3 => Some(Rgb {
                r: d[0] * 16 + d[0],
                g: d[1] * 16 + d[1],
                b: d[2] * 16 + d[2],
            }),
            6 => Some(Rgb {
                r: d[0] * 16 + d[1],
                g: d[2] * 16 + d[3],
                b: d[4] * 16 + d[5],
            }),
            _ => None,
        }
    }

    /// Lowercase `#rrggbb`, the form every derived token and preview uses.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// The colour as an `rgba()` string, for tokens that blend toward the text.
    pub fn to_rgba(self, alpha: f64) -> String {
        format!("rgba({}, {}, {}, {})", self.r, self.g, self.b, alpha)
    }
}

fn channel_luminance(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    // WCAG 2.x: linearise sRGB below the 0.03928 threshold, gamma above it.
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG 2.x relative luminance.
pub fn luminance(c: Rgb) -> f64 {
    0.2126 * channel_luminance(c.r)
        + 0.7152 * channel_luminance(c.g)
        + 0.0722 * channel_luminance(c.b)
}

/// WCAG contrast ratio, 1.0 to 21.0, symmetric in its arguments.
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (hi, lo) = if luminance(a) >= luminance(b) {
        (a, b)
    } else {
        (b, a)
    };
    (luminance(hi) + 0.05) / (luminance(lo) + 0.05)
}

/// Per-channel linear interpolation, rounded half up. `t` 0 gives `a`, 1 gives `b`.
pub fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let blend = |x: u8, y: u8| ((f64::from(x) + (f64::from(y) - f64::from(x)) * t).round()) as u8;
    Rgb {
        r: blend(a.r, b.r),
        g: blend(a.g, b.g),
        b: blend(a.b, b.b),
    }
}

/// Converts sRGB to `(hue 0..360, saturation 0..1, lightness 0..1)`.
pub fn rgb_to_hsl(c: Rgb) -> (f64, f64, f64) {
    let r = f64::from(c.r) / 255.0;
    let g = f64::from(c.g) / 255.0;
    let b = f64::from(c.b) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;
    let d = max - min;
    if d.abs() < 1e-9 {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < 1e-9 {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) * 60.0
    } else if (max - g).abs() < 1e-9 {
        ((b - r) / d + 2.0) * 60.0
    } else {
        ((r - g) / d + 4.0) * 60.0
    };
    (h.rem_euclid(360.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0))
}

/// Converts `(hue 0..360, saturation 0..1, lightness 0..1)` to sRGB.
pub fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgb {
    let h = h.rem_euclid(360.0);
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c * 0.5;
    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to_u8 = |v: f64| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgb {
        r: to_u8(r),
        g: to_u8(g),
        b: to_u8(b),
    }
}
