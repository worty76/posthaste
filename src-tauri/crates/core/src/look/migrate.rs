//! Moves a v1 document into the v2 shape: its fields become the theme,
//! and the scene and asset list start empty. Pure: no I/O, and it does
//! not validate — callers validate the result like any other document.
//! This is the only v1 → v2 converter.

use serde_json::Map;

use crate::look::{LookDoc, LookDocV2, LookTheme, Scene, LOOK_FORMAT, LOOK_VERSION_V2};

pub fn v1_to_v2(v1: &LookDoc) -> LookDocV2 {
    LookDocV2 {
        format: LOOK_FORMAT.into(),
        version: LOOK_VERSION_V2,
        name: v1.name.clone(),
        author: v1.author.clone(),
        theme: LookTheme {
            base: v1.base,
            colors: v1.colors.clone(),
            shape: v1.shape,
            font: v1.font,
            extra: Map::new(),
        },
        scene: Scene::default(),
        assets: Vec::new(),
        extra: Map::new(),
    }
}
