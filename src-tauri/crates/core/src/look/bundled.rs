//! The eight looks that ship with the app, in gallery order. They are
//! data: parsed here, validated by the tests, never edited at runtime.
//! Ink, the factory default, comes first. The six colour looks are v1
//! documents; Sakura night and Paper are v2, with emoji stickers.

use crate::look::LookDocument;

const SOURCES: [(&str, &str); 8] = [
    ("ink", include_str!("../../looks/ink.json")),
    (
        "strawberry-milk",
        include_str!("../../looks/strawberry-milk.json"),
    ),
    ("matcha", include_str!("../../looks/matcha.json")),
    (
        "lavender-haze",
        include_str!("../../looks/lavender-haze.json"),
    ),
    (
        "midnight-grape",
        include_str!("../../looks/midnight-grape.json"),
    ),
    ("blueberry", include_str!("../../looks/blueberry.json")),
    (
        "sakura-night",
        include_str!("../../looks/sakura-night.json"),
    ),
    ("paper", include_str!("../../looks/paper.json")),
];

/// `(id, document)` pairs with ids `builtin:<slug>`, bundled looks first
/// in gallery order. A file that no longer parses is skipped and logged
/// rather than breaking the app; the bundled-look tests are what catch
/// that.
pub fn bundled() -> Vec<(String, LookDocument)> {
    SOURCES
        .iter()
        .filter_map(|(slug, json)| match LookDocument::from_json(json) {
            Ok(doc) => Some((format!("builtin:{slug}"), doc)),
            Err(e) => {
                log::error!("bundled look {slug} does not parse: {e}");
                None
            }
        })
        .collect()
}
