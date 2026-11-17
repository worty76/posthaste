//! The backdrop image layer: its config shape, its limits and the one
//! region it may sit in.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::look::scene::Region;

/// The layer kind name in a v2 document.
pub const KIND: &str = "image";
/// The config version; a higher `v` means a reader this build predates.
pub const VERSION: u32 = 1;
/// Images are backdrop layers only; margin images are a later request.
pub const REGIONS: &[Region] = &[Region::Backdrop];
/// How strongly the backdrop dims the image, from not at all to fully.
pub const MIN_DIM: f64 = 0.0;
pub const MAX_DIM: f64 = 1.0;

/// How the image fills the window, as CSS `object-fit` spellings.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    #[default]
    Cover,
    Contain,
    Fill,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct ImageLayer {
    /// The image to draw, as an id in the look's `assets` list.
    pub asset: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<Fit>,
    /// 0 keeps the image at full strength; 1 dims it out completely.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim: Option<f64>,
    /// Fields a newer Posthaste may have added: kept, never drawn.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
