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

/// A well-formed asset id for the fixtures: `sha256:` plus 64 hex digits.
fn asset_id() -> String {
    format!("sha256:{}", "a".repeat(64))
}

/// The document's claim for an asset the fixtures only reference by id.
fn listed_asset(id: &str) -> crate::look::AssetRef {
    crate::look::AssetRef {
        id: id.into(),
        asset_type: "image/png".into(),
        bytes: 33,
        width: 2,
        height: 3,
        extra: Map::new(),
    }
}

// The tests, grouped by topic: colour maths and validation, derivation,
// share codes and bundled looks, storage, the v2 format layer, emoji,
// image assets and resolved scenes.
mod assets;
mod code;
mod color;
mod derive;
mod emoji;
mod format;
mod scene;
mod store;
