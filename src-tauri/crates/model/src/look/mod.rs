//! Looks: the colour document a look is written as, and the shapes the
//! shell sends about it. Checking a document and deriving its tokens is
//! storage's job (`posthaste-core`); these are only the shapes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Which built-in token set the look sits on: everything it does not set
/// comes from the light or the dark block in `tokens.css`.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Base {
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Sharp,
    #[default]
    Soft,
    Round,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum FontFace {
    #[default]
    System,
    Rounded,
    Serif,
    Mono,
}

/// The look's colours. Raw hex only; anything else is rejected by
/// storage before it is used. A v2 document keeps colour keys it does
/// not model in `extra`; a v1 document refuses them in validation.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Colors {
    pub bg: String,
    pub surface: String,
    pub text: String,
    pub accent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg_sunken: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_raised: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub danger: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ok: Option<String>,
    /// Colour keys a newer Posthaste may have added: kept, never drawn.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// A look document, exactly what version 1 of the format allows. Unknown
/// fields anywhere reject the whole look.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct LookDoc {
    pub format: String,
    pub version: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub base: Base,
    pub colors: Colors,
    #[serde(default)]
    pub shape: Shape,
    #[serde(default)]
    pub font: FontFace,
}

/// Normalized hex of the four main colours, for gallery cards.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
pub struct LookPreview {
    pub bg: String,
    pub surface: String,
    pub text: String,
    pub accent: String,
}

/// A look as the gallery shows it: identity plus the four preview colours.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct LookSummary {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub builtin: bool,
    pub base: Base,
    pub preview: LookPreview,
}

/// A checked look, ready to apply: its CSS custom properties by name.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct ResolvedLook {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub base: Base,
    pub shape: Shape,
    pub font: FontFace,
    pub tokens: BTreeMap<String, String>,
}

// ---- format v2 -------------------------------------------------------------
//
// Version 2 adds a scene of layers and an asset list, and keeps every field
// it does not understand in an `extra` map, so a look written by a newer
// Posthaste survives an edit in an older one. The v1 shapes above are
// unchanged: v1 documents parse into them and migrate into `LookDocV2` in
// storage. Adding a field or a layer kind never bumps the version; only a
// change that would make an old reader draw something wrong does.

pub mod asset;
pub mod kinds;
pub mod layer;
pub mod scene;

/// The format string every look document carries, v1 and v2 alike.
pub const LOOK_FORMAT: &str = "posthaste-look";
/// The current document version.
pub const LOOK_VERSION_V2: u32 = 2;

pub use asset::AssetRef;
pub use kinds::image::{Fit, ImageLayer};
pub use layer::{Known, LayerRecord};
pub use scene::{Region, Scene, WindowScene};

/// The v1 theme fields, unchanged, nested in a v2 document. Named
/// `LookTheme` because `Theme` is already the interface's dark-or-light
/// setting.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct LookTheme {
    pub base: Base,
    pub colors: Colors,
    #[serde(default)]
    pub shape: Shape,
    #[serde(default)]
    pub font: FontFace,
    /// Fields a newer Posthaste may have added: kept, never drawn.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// A look document, version 2: the v1 theme plus a scene and an asset
/// list, with every level keeping fields it does not understand.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct LookDocV2 {
    pub format: String,
    pub version: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub theme: LookTheme,
    #[serde(default, skip_serializing_if = "Scene::is_empty")]
    pub scene: Scene,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<AssetRef>,
    /// Fields a newer Posthaste may have added: kept, never drawn.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl LookDocV2 {
    /// Replaces what the editor edits (identity and theme) from a v1
    /// document, keeping the scene, the assets and every field this build
    /// does not model. Editing the parsed value is what keeps those parts;
    /// rebuilding from typed fields would drop them.
    pub fn set_theme_from(&mut self, v1: &LookDoc) {
        self.name = v1.name.clone();
        self.author = v1.author.clone();
        let extra = std::mem::take(&mut self.theme.extra);
        self.theme = LookTheme {
            base: v1.base,
            colors: v1.colors.clone(),
            shape: v1.shape,
            font: v1.font,
            extra,
        };
    }

    /// The theme part as a v1 document: what token derivation reads and
    /// what the look editor shows. v2-only parts are not in the view.
    pub fn v1_view(&self) -> LookDoc {
        LookDoc {
            format: LOOK_FORMAT.into(),
            version: 1,
            name: self.name.clone(),
            author: self.author.clone(),
            base: self.theme.base,
            colors: self.theme.colors.clone(),
            shape: self.theme.shape,
            font: self.theme.font,
        }
    }
}
