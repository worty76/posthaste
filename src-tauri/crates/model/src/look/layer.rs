//! The layer envelope: one shape that parses whatever the kind is, and a
//! separate step that turns it into a typed layer.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::look::kinds::image::ImageLayer;

/// The envelope every scene layer is written in. It never fails to parse
/// because of a new kind: unknown kinds and unknown fields land in
/// `config` and `extra` verbatim, and a known kind with a config this
/// build cannot read is kept untouched through [`LayerRecord::known`]
/// returning `None`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct LayerRecord {
    pub kind: String,
    pub v: u32,
    /// Off layers are kept, not dropped, so a toggle survives a downgrade.
    #[serde(default = "yes")]
    pub on: bool,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub config: Value,
    /// Fields a newer Posthaste may have added: kept, never drawn.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The layer's config parsed into a type this build knows.
#[derive(Clone, PartialEq, Debug)]
pub enum Known {
    Image(ImageLayer),
}

/// The envelope's default for `on`.
fn yes() -> bool {
    true
}

impl LayerRecord {
    /// `None` = a kind or version this build does not know: keep, never
    /// draw. Adding a kind means a new `Known` variant and an arm here,
    /// which the compiler then requires in every check and renderer.
    pub fn known(&self) -> Option<Known> {
        if self.kind == ImageLayer::KIND && self.v <= ImageLayer::VERSION {
            serde_json::from_value(self.config.clone())
                .ok()
                .map(Known::Image)
        } else {
            None
        }
    }
}
