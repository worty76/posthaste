//! Tests for the look module: colour maths, validation, derivation, share
//! codes, bundled looks, storage, the document format, emoji, image
//! assets and resolved scenes.

use serde_json::Map;

use crate::look::{Base, Colors, FontFace, LookDoc, LookTheme, Shape, ThemeEdit};

/// The old version number, used only by the tests that prove this build
/// refuses it. Interpolated into fixtures, never written as text, so no
/// v1 wire value lives in the source (`check-rules.sh` looks for one).
pub(crate) const OLD_VERSION: u32 = 1;

/// The Strawberry Milk look as a full document (no scene, no assets).
fn doc() -> LookDoc {
    LookDoc {
        format: "posthaste-look".into(),
        version: 2,
        name: "Strawberry Milk".into(),
        author: Some("Posthaste".into()),
        theme: LookTheme {
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
            font_family: None,
            extra: Map::new(),
        },
        scene: Default::default(),
        assets: Vec::new(),
        extra: Default::default(),
    }
}

/// The same look as the editor edits it.
fn theme() -> ThemeEdit {
    doc().theme_edit()
}

/// A minimal valid PNG header the sniffer accepts, for asset fixtures.
pub(crate) fn png(width: u32, height: u32) -> Vec<u8> {
    let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    b.extend_from_slice(&13u32.to_be_bytes());
    b.extend_from_slice(b"IHDR");
    b.extend_from_slice(&width.to_be_bytes());
    b.extend_from_slice(&height.to_be_bytes());
    b.extend_from_slice(&[8, 2, 0, 0, 0]);
    b.extend_from_slice(&[0; 4]); // CRC, which the sniffer never reads
    b
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
// share codes and bundled looks, storage, the document format, emoji,
// image assets and resolved scenes.
mod assets;
mod code;
mod color;
mod derive;
mod emoji;
mod format;
mod scene;
mod store;
