//! The scene: which layers decorate which window, in which region.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::look::layer::LayerRecord;

/// The two bands of a window a scene can decorate. `backdrop` sits behind
/// all content; `margin` holds decorations around the edges, where they
/// can never cover the text.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    Backdrop,
    Margin,
}

/// The layers of one window. There is no z-index: a window renders
/// `all` first, then its own layers, in list order.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct WindowScene {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backdrop: Vec<LayerRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub margin: Vec<LayerRecord>,
    /// Region names a newer Posthaste may have added: kept, never drawn.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl WindowScene {
    /// Every layer with the region it sits in.
    pub fn regions(&self) -> impl Iterator<Item = (Region, &LayerRecord)> {
        self.backdrop
            .iter()
            .map(|l| (Region::Backdrop, l))
            .chain(self.margin.iter().map(|l| (Region::Margin, l)))
    }
}

/// A look's scene: an `all` entry plus one per window. A window not
/// listed here renders `all` only; a future window would be one more key.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Scene {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<WindowScene>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<WindowScene>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<WindowScene>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub today: Option<WindowScene>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<WindowScene>,
    /// Window names a newer Posthaste may have added: kept, never drawn.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Scene {
    /// True when there is nothing to draw and nothing to keep.
    pub fn is_empty(&self) -> bool {
        self.all.is_none()
            && self.main.is_none()
            && self.capture.is_none()
            && self.today.is_none()
            && self.details.is_none()
            && self.extra.is_empty()
    }

    /// Every layer of the look, `all` first, with the region it sits in.
    pub fn layers(&self) -> impl Iterator<Item = (Region, &LayerRecord)> {
        self.all
            .iter()
            .chain(self.main.iter())
            .chain(self.capture.iter())
            .chain(self.today.iter())
            .chain(self.details.iter())
            .flat_map(|w| w.regions())
    }

    /// The layer count the document limit is checked against.
    pub fn layer_count(&self) -> usize {
        self.layers().count()
    }
}
