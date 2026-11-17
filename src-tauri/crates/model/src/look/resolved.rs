//! The scene as it reaches the interface: checked, ordered and ready to
//! draw, the way token derivation hands colours over. The UI renders
//! these values and never sees a raw look document.

use serde::{Deserialize, Serialize};

use crate::look::asset::AssetRef;
use crate::look::kinds::emoji::{Anchor, StickerSize};
use crate::look::kinds::image::Fit;
use crate::look::scene::Scene;

/// The four windows a scene can decorate.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Window {
    Main,
    Capture,
    Today,
    Details,
}

/// One drawable layer, with every value decided: this is what the
/// renderer gets instead of a document.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ResolvedLayer {
    Image {
        asset: String,
        fit: Fit,
        /// Backdrop dimming in tenths, from none to fully dark.
        dim_tenths: u8,
    },
    Emoji {
        char: String,
        anchor: Anchor,
        size: StickerSize,
        tilt: i32,
        faded: bool,
    },
}

/// The drawable layers of one window: backdrop below the content,
/// margin above it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct ResolvedWindow {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backdrop: Vec<ResolvedLayer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub margin: Vec<ResolvedLayer>,
}

impl ResolvedWindow {
    /// True when there is nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.backdrop.is_empty() && self.margin.is_empty()
    }
}

/// The scene of the active look, all four windows at once. `skipped`
/// counts the layers this build does not draw: unknown kinds, future
/// versions, layers switched off, and layers that failed their check.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct ResolvedScene {
    pub main: ResolvedWindow,
    pub capture: ResolvedWindow,
    pub today: ResolvedWindow,
    pub details: ResolvedWindow,
    #[serde(default)]
    pub skipped: u32,
}

impl ResolvedScene {
    /// The layers drawn in one window.
    pub fn window(&self, window: Window) -> &ResolvedWindow {
        match window {
            Window::Main => &self.main,
            Window::Capture => &self.capture,
            Window::Today => &self.today,
            Window::Details => &self.details,
        }
    }
}

/// A look's scene and asset list as the editor edits them: the parts of
/// a v2 document that the colour editor does not model.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct SceneEdit {
    #[serde(default)]
    pub scene: Scene,
    #[serde(default)]
    pub assets: Vec<AssetRef>,
}
