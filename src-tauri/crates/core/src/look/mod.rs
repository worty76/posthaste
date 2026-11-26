//! Looks: a colour document with a scene of layers, validated and turned
//! into CSS tokens here in Rust; the UI only applies the result.
//! Validation lives here and nowhere else, so a look can never reach the
//! DOM unchecked. Unknown fields are kept, never drawn.

pub mod assets;
pub mod bundled;
pub mod code;
pub mod color;
pub mod derive;
pub mod package;
pub mod resolve;
pub mod scene;
pub mod store;
pub mod suggest;

#[cfg(test)]
mod tests;

use crate::error::{CoreError, Result};
use color::{contrast, Rgb};

/// The look a fresh install wears until the user picks one.
pub use posthaste_model::DEFAULT_LOOK_ID;
pub use posthaste_model::LOOK_FORMAT;
pub use posthaste_model::{
    Anchor, AssetRef, Base, Colors, EmojiLayer, Fit, FontFace, ImageLayer, Known, LayerRecord,
    LookDoc, LookPreview, LookTheme, PaletteMood, PalettePreset, Read, Region, ResolvedLayer,
    ResolvedScene, ResolvedWindow, Scene, SceneEdit, Shape, StickerSize, ThemeEdit, Window,
    WindowScene,
};
pub use resolve::{resolve_live, resolve_parts, resolve_scene};
pub const LOOK_VERSION: u32 = 2;
pub const MAX_NAME_CHARS: usize = 40;
/// Longest font name a look can name.
pub const MAX_FONT_FAMILY_CHARS: usize = 64;
pub const MAX_DOC_BYTES: usize = 16 * 1024;
pub const MAX_DEPTH: usize = 16;
pub const MAX_LAYERS: usize = 64;
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
    /// Parses and validates. Size and nesting depth are bounded before any
    /// of the document is processed.
    fn from_json(json: &str) -> Result<Self>;
    /// Compact JSON, the form stored and shared. The `extra` maps ride
    /// along, so unknown fields come back out.
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
            return Err(newer_version_error(self.version));
        }
        if self.version != LOOK_VERSION {
            return Err(not_a_look_error());
        }
        check_identity(&self.name, self.author.as_deref())?;
        check_colors(&self.theme.colors)?;
        check_font_family(self.theme.font_family.as_deref())?;
        scene::check(&self.scene, &self.assets)
    }

    fn from_json(json: &str) -> Result<LookDoc> {
        parse(json, false)
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn preview(&self) -> LookPreview {
        preview_of(&self.theme.colors)
    }
}

/// The identity checks, shared by the document and the editor edit.
pub fn check_identity(name: &str, author: Option<&str>) -> Result<()> {
    if name.trim().is_empty() {
        return Err(CoreError::InvalidLook("Give the look a name.".into()));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(CoreError::InvalidLook(format!(
            "The name can be at most {MAX_NAME_CHARS} characters."
        )));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(CoreError::InvalidLook(
            "The name can't contain control characters.".into(),
        ));
    }
    if let Some(author) = author {
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
    Ok(())
}

/// The font-name checks, shared by the document and the editor edit. A
/// font is only a name, drawn after the preset stack as `"<name>",
/// var(<stack>)`: the character rule is what keeps rule text out of it.
pub fn check_font_family(name: Option<&str>) -> Result<()> {
    let Some(name) = name else {
        return Ok(());
    };
    if name.trim().is_empty() || name.trim() != name {
        return Err(CoreError::InvalidLook(
            "The font name can't be empty, or start or end with a space.".into(),
        ));
    }
    if name
        .chars()
        .any(|c| !(c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.')))
    {
        return Err(CoreError::InvalidLook(
            "The font name can only use letters, numbers, spaces, hyphens, underscores and dots."
                .into(),
        ));
    }
    if name.chars().count() > MAX_FONT_FAMILY_CHARS {
        return Err(CoreError::InvalidLook(format!(
            "The font name can be at most {MAX_FONT_FAMILY_CHARS} characters."
        )));
    }
    Ok(())
}

/// The colour checks, shared by the document and the editor edit.
fn check_colors(colors: &Colors) -> Result<()> {
    let bg = parse_color("bg", &colors.bg)?;
    let surface = parse_color("surface", &colors.surface)?;
    let text = parse_color("text", &colors.text)?;
    let accent = parse_color("accent", &colors.accent)?;
    for (field, value) in [
        ("bg_sunken", &colors.bg_sunken),
        ("surface_raised", &colors.surface_raised),
        ("danger", &colors.danger),
        ("ok", &colors.ok),
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

/// Checks what the look editor edits: identity, colours, font name.
pub fn validate_theme(edit: &ThemeEdit) -> Result<()> {
    check_identity(&edit.name, edit.author.as_deref())?;
    check_colors(&edit.colors)?;
    check_font_family(edit.font_family.as_deref())
}

/// Normalized hex of the four main colours, for gallery cards.
fn preview_of(colors: &Colors) -> LookPreview {
    let hex = |s: &str| Rgb::parse(s).map(Rgb::to_hex).unwrap_or_default();
    LookPreview {
        bg: hex(&colors.bg),
        surface: hex(&colors.surface),
        text: hex(&colors.text),
        accent: hex(&colors.accent),
    }
}

/// Checks a scene and its asset list without a whole document: what the
/// editor's live preview asks for. The same checks `validate` runs.
pub fn validate_scene_parts(scene: &Scene, assets: &[AssetRef]) -> Result<()> {
    scene::check(scene, assets)
}

/// The message for a payload that arrived broken: the code changed when it
/// was copied.
pub(crate) fn incomplete_code() -> CoreError {
    CoreError::InvalidLook(
        "That code looks incomplete, or it changed when it was copied. Ask for it again.".into(),
    )
}

/// Parses a look document with the shared gates: size first, then shape,
/// then depth, then every check. `from_code` reports a payload that
/// cannot be parsed as a code that changed in transit; the file path
/// reports it as an invalid look.
pub(crate) fn parse(json: &str, from_code: bool) -> Result<LookDoc> {
    if json.len() > MAX_DOC_BYTES {
        return Err(CoreError::InvalidLook(format!(
            "That look file is too large. The limit is {MAX_DOC_BYTES} bytes."
        )));
    }
    let mangled = |e| {
        if from_code {
            incomplete_code()
        } else {
            CoreError::InvalidLook(format!("That isn't a valid look: {e}"))
        }
    };
    let value: serde_json::Value = serde_json::from_str(json).map_err(mangled)?;
    // The version decides the message before the shape does: a newer
    // version's document may not even parse as this build's shape, and
    // "missing field theme" would hide the real answer.
    if value["format"].as_str() == Some(LOOK_FORMAT) {
        let version = value["version"].as_u64().unwrap_or(0);
        if version > u64::from(LOOK_VERSION) {
            return Err(newer_version_error(version as u32));
        }
        if version != u64::from(LOOK_VERSION) {
            return Err(not_a_look_error());
        }
    }
    let depth = json_depth(&value);
    if depth > MAX_DEPTH {
        return Err(CoreError::InvalidLook(format!(
            "That look nests {depth} levels deep. The limit is {MAX_DEPTH}."
        )));
    }
    let doc: LookDoc = serde_json::from_value(value).map_err(mangled)?;
    doc.validate()?;
    Ok(doc)
}

/// Parses a stored row, shape only: the size gates ran when it was
/// saved. A version this build does not know is refused here; the store
/// decides what a refused row means for each caller.
pub fn parse_stored(json: &str) -> Result<LookDoc> {
    let invalid = |e| CoreError::InvalidLook(format!("That isn't a valid look: {e}"));
    let value: serde_json::Value = serde_json::from_str(json).map_err(invalid)?;
    let version = value["version"].as_u64().unwrap_or(0);
    if value["format"] != serde_json::Value::String(LOOK_FORMAT.into())
        || version != u64::from(LOOK_VERSION)
    {
        return Err(not_a_look_error());
    }
    serde_json::from_value(value).map_err(invalid)
}

fn newer_version_error(version: u32) -> CoreError {
    CoreError::InvalidLook(format!(
        "This look was made with a newer version of Posthaste (format {version}). \
         Update the app to use it."
    ))
}

fn not_a_look_error() -> CoreError {
    CoreError::InvalidLook("This isn't a Posthaste look file.".into())
}

/// The deepest container nesting in a JSON value; a scalar counts as one.
/// The recursion is bounded: serde_json refuses to parse deeper than 128.
fn json_depth(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Array(items) => 1 + items.iter().map(json_depth).max().unwrap_or(0),
        serde_json::Value::Object(map) => 1 + map.values().map(json_depth).max().unwrap_or(0),
        _ => 1,
    }
}
