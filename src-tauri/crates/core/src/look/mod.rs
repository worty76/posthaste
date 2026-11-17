//! Looks: a small colour document, validated and turned into CSS tokens here
//! in Rust; the UI only applies the result. Validation lives here and nowhere
//! else, so a look can never reach the DOM unchecked. Format v2 adds a scene
//! and an asset list and keeps unknown fields; v1 documents migrate to it on
//! the next save.

pub mod assets;
pub mod bundled;
pub mod code;
pub mod color;
pub mod derive;
pub mod migrate;
pub mod resolve;
pub mod scene;
pub mod store;

#[cfg(test)]
mod tests;

use crate::error::{CoreError, Result};
use color::{contrast, Rgb};

/// The look a fresh install wears until the user picks one.
pub use posthaste_model::DEFAULT_LOOK_ID;
pub use posthaste_model::LOOK_FORMAT;
pub use posthaste_model::{
    Anchor, AssetRef, Base, Colors, EmojiLayer, Fit, FontFace, ImageLayer, Known, LayerRecord,
    LookDoc, LookDocV2, LookPreview, LookTheme, Read, Region, ResolvedLayer, ResolvedScene,
    ResolvedWindow, Scene, SceneEdit, Shape, StickerSize, Window, WindowScene, LOOK_VERSION_V2,
};
pub use resolve::{resolve_parts, resolve_scene};
pub const LOOK_VERSION: u32 = 1;
pub const MAX_NAME_CHARS: usize = 40;
pub const MAX_DOC_BYTES: usize = 4096;
/// The v2 limits: a roomier document, but bounded nesting and layer count.
pub const MAX_DOC_BYTES_V2: usize = 16 * 1024;
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
        // Version 1 knows no extra colour keys: name the first one so the
        // look's author can remove it.
        if let Some(unknown) = self.colors.extra.keys().next() {
            return Err(CoreError::InvalidLook(format!(
                "That isn't a valid look: unknown field `{unknown}`."
            )));
        }
        check_identity(&self.name, self.author.as_deref())?;
        check_colors(&self.colors)
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
        preview_of(&self.colors)
    }
}

/// The identity checks, shared by both format versions.
fn check_identity(name: &str, author: Option<&str>) -> Result<()> {
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

/// The colour checks, shared by both format versions.
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

/// What storage does with a v2 look document. Same gate order as v1:
/// size first, then parse, then every check.
pub trait LookDocV2Ext: Sized {
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

impl LookDocV2Ext for LookDocV2 {
    fn validate(&self) -> Result<()> {
        if self.format != LOOK_FORMAT {
            return Err(CoreError::InvalidLook(
                "This isn't a Posthaste look file.".into(),
            ));
        }
        if self.version > LOOK_VERSION_V2 {
            return Err(newer_version_error(self.version));
        }
        if self.version != LOOK_VERSION_V2 {
            return Err(not_a_look_error());
        }
        check_identity(&self.name, self.author.as_deref())?;
        check_colors(&self.theme.colors)?;
        scene::check(&self.scene, &self.assets)
    }

    fn from_json(json: &str) -> Result<LookDocV2> {
        v2_parse(json, false)
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn preview(&self) -> LookPreview {
        preview_of(&self.theme.colors)
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

/// Parses a v2 document with the shared gates: size first, then shape,
/// then depth, then every check. `from_code` reports a payload that
/// cannot be parsed as a code that changed in transit; the file path
/// reports it as an invalid look.
pub(crate) fn v2_parse(json: &str, from_code: bool) -> Result<LookDocV2> {
    if json.len() > MAX_DOC_BYTES_V2 {
        return Err(CoreError::InvalidLook(format!(
            "That look file is too large. The limit is {MAX_DOC_BYTES_V2} bytes."
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
    let depth = json_depth(&value);
    if depth > MAX_DEPTH {
        return Err(CoreError::InvalidLook(format!(
            "That look nests {depth} levels deep. The limit is {MAX_DEPTH}."
        )));
    }
    let doc: LookDocV2 = serde_json::from_value(value).map_err(mangled)?;
    doc.validate()?;
    Ok(doc)
}

/// A look document of either format. Reading dispatches on the version; a
/// v1 document migrates to v2 when it is saved, and a stored v2 document
/// keeps the parts this build does not model.
#[allow(clippy::large_enum_variant)] // a short-lived dispatch value; the
// size gap between the variants costs nothing here
#[derive(Clone, PartialEq, Debug)]
pub enum LookDocument {
    V1(LookDoc),
    V2(LookDocV2),
}

#[derive(serde::Deserialize)]
struct VersionProbe {
    version: u32,
}

fn probe_version(json: &str) -> Result<u32> {
    serde_json::from_str::<VersionProbe>(json)
        .map(|p| p.version)
        .map_err(|e| CoreError::InvalidLook(format!("That isn't a valid look: {e}")))
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

impl LookDocument {
    /// Parses and validates a document from outside the database: pasted
    /// JSON or a decoded share code. Each version keeps its own size gate.
    pub fn from_json(json: &str) -> Result<Self> {
        match probe_version(json)? {
            1 => Ok(Self::V1(LookDoc::from_json(json)?)),
            2 => Ok(Self::V2(LookDocV2::from_json(json)?)),
            v if v > LOOK_VERSION_V2 => Err(newer_version_error(v)),
            _ => Err(not_a_look_error()),
        }
    }

    /// Parses a stored row. The size gates ran when it was saved, so only
    /// the shape is checked here; the caller validates before trusting it.
    pub fn parse_stored(json: &str) -> Result<Self> {
        let invalid = |e| CoreError::InvalidLook(format!("That isn't a valid look: {e}"));
        match probe_version(json)? {
            1 => Ok(Self::V1(serde_json::from_str(json).map_err(invalid)?)),
            2 => Ok(Self::V2(serde_json::from_str(json).map_err(invalid)?)),
            v if v > LOOK_VERSION_V2 => Err(newer_version_error(v)),
            _ => Err(not_a_look_error()),
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::V1(d) => d.validate(),
            Self::V2(d) => d.validate(),
        }
    }

    pub fn to_json(&self) -> String {
        match self {
            Self::V1(d) => d.to_json(),
            Self::V2(d) => d.to_json(),
        }
    }

    /// The theme part as a v1 document: what token derivation reads and
    /// what the look editor shows. v2-only parts are not in the view.
    pub fn v1_view(&self) -> LookDoc {
        match self {
            Self::V1(d) => d.clone(),
            Self::V2(d) => d.v1_view(),
        }
    }

    pub fn preview(&self) -> LookPreview {
        match self {
            Self::V1(d) => d.preview(),
            Self::V2(d) => d.preview(),
        }
    }
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
