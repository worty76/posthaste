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
/// storage before it is used.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
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
