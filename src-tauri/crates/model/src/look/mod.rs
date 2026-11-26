//! Looks: the document a look is written as, the shape the editor edits,
//! and the shapes the shell sends about it. Checking a document and
//! deriving its tokens is storage's job (`posthaste-core`); these are only
//! the shapes.

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

/// The font presets a look picks between. The bundled faces name fonts
/// the app ships (see `ui/styles/fonts/`); the first four name system
/// stacks. Wire values are kebab-case.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "kebab-case")]
pub enum FontFace {
    #[default]
    System,
    Rounded,
    Serif,
    Mono,
    BeVietnamPro,
    Nunito,
    Quicksand,
}

/// The look's colours. Raw hex only; anything else is rejected by
/// storage before it is used. Colour keys this build does not model are
/// kept in `extra`, never drawn.
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

/// A look document: the theme plus a scene of layers and an asset list,
/// with every level keeping fields it does not understand. There is one
/// format; adding a field or a layer kind never bumps the version — only
/// a change that would make an old reader draw something wrong does.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct LookDoc {
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

/// The theme part of a look document. Named `LookTheme` because `Theme`
/// is already the interface's dark-or-light setting.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct LookTheme {
    pub base: Base,
    pub colors: Colors,
    #[serde(default)]
    pub shape: Shape,
    #[serde(default)]
    pub font: FontFace,
    /// The name of a font installed on this computer, drawn in front of
    /// the preset stack above. `None` means the preset alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// Fields a newer Posthaste may have added: kept, never drawn.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// What the look editor edits: identity and theme, without the
/// document's envelope. The scene and the assets travel separately, as
/// `SceneEdit`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct ThemeEdit {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub base: Base,
    pub colors: Colors,
    #[serde(default)]
    pub shape: Shape,
    #[serde(default)]
    pub font: FontFace,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
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
    /// The look's custom font name, when it names one.
    pub font_family: Option<String>,
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
    /// Minimum text contrast ratio (tenths, e.g. 148 = 14.8:1) across
    /// background and surface. `0` when omitted by older payloads.
    #[serde(default)]
    pub contrast_x10: u16,
}

/// How strongly a generated palette tints its neutral background and surface
/// from the chosen accent hue.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PaletteMood {
    /// Clean grayscale neutrals with the accent providing the sole colour pop.
    Neutral,
    /// Subtle tint of the accent hue woven into background, surface and text.
    #[default]
    Tinted,
    /// Warm paper / espresso neutrals paired with the accent.
    Warm,
}

/// A curated four-colour starting palette shown in the Look Editor.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct PalettePreset {
    pub id: String,
    pub name: String,
    pub base: Base,
    pub colors: Colors,
}

pub mod asset;
pub mod kinds;
pub mod layer;
pub mod resolved;
pub mod scene;

/// The format string every look document carries.
pub const LOOK_FORMAT: &str = "posthaste-look";
/// The current document version.
pub const LOOK_VERSION: u32 = 2;

pub use asset::AssetRef;
pub use kinds::emoji::{Anchor, EmojiLayer, StickerSize};
pub use kinds::image::{Fit, ImageLayer};
pub use layer::{Known, LayerRecord, Read};
pub use resolved::{ResolvedLayer, ResolvedScene, ResolvedWindow, SceneEdit, Window};
pub use scene::{Region, Scene, WindowScene};

impl LookDoc {
    /// The theme part as the editor edits it. v2-only parts are not in
    /// the view.
    pub fn theme_edit(&self) -> ThemeEdit {
        ThemeEdit {
            name: self.name.clone(),
            author: self.author.clone(),
            base: self.theme.base,
            colors: self.theme.colors.clone(),
            shape: self.theme.shape,
            font: self.theme.font,
            font_family: self.theme.font_family.clone(),
        }
    }

    /// Replaces what the editor edits (identity and theme) from an edit,
    /// keeping the scene, the assets and every field this build does not
    /// model — in the theme and inside `colors`. Editing the parsed value
    /// is what keeps those parts; rebuilding from typed fields would drop
    /// them.
    pub fn set_theme_from(&mut self, edit: &ThemeEdit) {
        self.name = edit.name.clone();
        self.author = edit.author.clone();
        let extra = std::mem::take(&mut self.theme.extra);
        let colors_extra = std::mem::take(&mut self.theme.colors.extra);
        self.theme = LookTheme {
            base: edit.base,
            colors: Colors {
                bg: edit.colors.bg.clone(),
                surface: edit.colors.surface.clone(),
                text: edit.colors.text.clone(),
                accent: edit.colors.accent.clone(),
                bg_sunken: edit.colors.bg_sunken.clone(),
                surface_raised: edit.colors.surface_raised.clone(),
                danger: edit.colors.danger.clone(),
                ok: edit.colors.ok.clone(),
                extra: colors_extra,
            },
            shape: edit.shape,
            font: edit.font,
            font_family: edit.font_family.clone(),
            extra,
        };
    }
}
