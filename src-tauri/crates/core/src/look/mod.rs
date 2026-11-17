//! Looks: a small colour document, validated and turned into CSS tokens here
//! in Rust; the UI only applies the result. Validation lives here and nowhere
//! else, so a look can never reach the DOM unchecked.

pub mod bundled;
pub mod code;
pub mod color;
pub mod derive;
pub mod store;

#[cfg(test)]
mod tests;

use crate::error::{CoreError, Result};
use color::{contrast, Rgb};

pub const LOOK_FORMAT: &str = "posthaste-look";
/// The look a fresh install wears until the user picks one.
pub use posthaste_model::DEFAULT_LOOK_ID;
pub use posthaste_model::{Base, Colors, FontFace, LookDoc, LookPreview, Shape};
pub const LOOK_VERSION: u32 = 1;
pub const MAX_NAME_CHARS: usize = 40;
pub const MAX_DOC_BYTES: usize = 4096;
pub const MAX_USER_LOOKS: usize = 50;
/// Text on the background or surface must stay readable (WCAG AA).
pub const TEXT_CONTRAST_MIN: f64 = 4.5;
/// The accent only has to stand out as a fill, not be read as text.
pub const ACCENT_FILL_MIN: f64 = 1.3;

fn parse_color(field: &str, value: &str) -> Result<Rgb> {
    Rgb::parse(value).ok_or_else(|| {
        let shown: String = value.chars().take(20).collect();
        CoreError::InvalidLook(format!(
            "Color \"{field}\" must look like #rrggbb or #rgb, not \"{shown}\"."
        ))
    })
}

/// What storage does with a look document. The document's shape lives in
/// `posthaste-model`, shared with the UI; checking it lives only here.
pub trait LookDocExt: Sized {
    /// All checks in order, first failure wins; nothing is partly applied.
    fn validate(&self) -> Result<()>;
    /// Parses and validates. The size limit comes first: a huge blob is
    /// refused before any of it is processed.
    fn from_json(json: &str) -> Result<Self>;
    /// Compact JSON, the form stored and shared.
    fn to_json(&self) -> String;
    fn preview(&self) -> LookPreview;
}

impl LookDocExt for LookDoc {
    fn validate(&self) -> Result<()> {
        if self.format != LOOK_FORMAT {
            return Err(CoreError::InvalidLook(
                "This isn't a Posthaste look file.".into(),
            ));
        }
        if self.version > LOOK_VERSION {
            return Err(CoreError::InvalidLook(format!(
                "This look was made with a newer version of Posthaste (format {v}). \
                 Update the app to use it.",
                v = self.version
            )));
        }
        if self.version < LOOK_VERSION {
            return Err(CoreError::InvalidLook(
                "This isn't a Posthaste look file.".into(),
            ));
        }

        if self.name.trim().is_empty() {
            return Err(CoreError::InvalidLook("Give the look a name.".into()));
        }
        if self.name.chars().count() > MAX_NAME_CHARS {
            return Err(CoreError::InvalidLook(format!(
                "The name can be at most {MAX_NAME_CHARS} characters."
            )));
        }
        if self.name.chars().any(|c| c.is_control()) {
            return Err(CoreError::InvalidLook(
                "The name can't contain control characters.".into(),
            ));
        }
        if let Some(author) = &self.author {
            if author.chars().count() > MAX_NAME_CHARS {
                return Err(CoreError::InvalidLook(format!(
                    "The author name can be at most {MAX_NAME_CHARS} characters."
                )));
            }
            if author.chars().any(|c| c.is_control()) {
                return Err(CoreError::InvalidLook(
                    "The author name can't contain control characters.".into(),
                ));
            }
        }

        let bg = parse_color("bg", &self.colors.bg)?;
        let surface = parse_color("surface", &self.colors.surface)?;
        let text = parse_color("text", &self.colors.text)?;
        let accent = parse_color("accent", &self.colors.accent)?;
        for (field, value) in [
            ("bg_sunken", &self.colors.bg_sunken),
            ("surface_raised", &self.colors.surface_raised),
            ("danger", &self.colors.danger),
            ("ok", &self.colors.ok),
        ] {
            if let Some(v) = value {
                parse_color(field, v)?;
            }
        }

        if contrast(text, bg) < TEXT_CONTRAST_MIN {
            return Err(CoreError::InvalidLook(format!(
                "Text and background are too similar to read comfortably ({:.1}:1, \
                 needs {:.1}:1).",
                contrast(text, bg),
                TEXT_CONTRAST_MIN
            )));
        }
        if contrast(text, surface) < TEXT_CONTRAST_MIN {
            return Err(CoreError::InvalidLook(format!(
                "Text and surface are too similar to read comfortably ({:.1}:1, \
                 needs {:.1}:1).",
                contrast(text, surface),
                TEXT_CONTRAST_MIN
            )));
        }
        if contrast(accent, bg) < ACCENT_FILL_MIN {
            return Err(CoreError::InvalidLook(
                "The accent is almost invisible on the background. \
                 Pick a color that stands out more."
                    .into(),
            ));
        }
        Ok(())
    }

    fn from_json(json: &str) -> Result<LookDoc> {
        if json.len() > MAX_DOC_BYTES {
            return Err(CoreError::InvalidLook(format!(
                "That look file is too large. The limit is {MAX_DOC_BYTES} bytes."
            )));
        }
        let doc: LookDoc = serde_json::from_str(json)
            .map_err(|e| CoreError::InvalidLook(format!("That isn't a valid look: {e}")))?;
        doc.validate()?;
        Ok(doc)
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn preview(&self) -> LookPreview {
        let hex = |s: &str| Rgb::parse(s).map(Rgb::to_hex).unwrap_or_default();
        LookPreview {
            bg: hex(&self.colors.bg),
            surface: hex(&self.colors.surface),
            text: hex(&self.colors.text),
            accent: hex(&self.colors.accent),
        }
    }
}
