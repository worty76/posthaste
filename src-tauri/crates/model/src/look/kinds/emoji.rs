//! The margin emoji sticker: its config shape, its limits and the one
//! region it may sit in.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::look::scene::Region;

/// How a sticker sits in the margin band.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    LeftMiddle,
    RightMiddle,
}

/// Sticker sizes are classes, not pixels: each maps to a CSS class, so
/// finer steps can be added later without a format bump.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum StickerSize {
    S,
    #[default]
    M,
    L,
    Xl,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct EmojiLayer {
    /// The sticker to draw: one emoji, at most 16 bytes of UTF-8.
    pub char: String,
    /// Where in the margin band it sits.
    pub anchor: Anchor,
    #[serde(default)]
    pub size: StickerSize,
    /// Degrees clockwise, a multiple of 15 from -45 to 45.
    #[serde(default)]
    pub tilt: i32,
    /// Drawn at reduced strength, for stickers that should sit back.
    #[serde(default)]
    pub faded: bool,
    /// Fields a newer Posthaste may have added: kept, never drawn.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl EmojiLayer {
    /// The layer kind name in a v2 document.
    pub const KIND: &str = "emoji";
    /// Stickers are not shipped yet: documents keep them untouched and
    /// storage still checks them, but nothing draws them and the editor
    /// does not offer them. Flip to true to ship.
    pub const SHIPPED: bool = false;
    /// The config version; a higher `v` means a reader this build predates.
    pub const VERSION: u32 = 1;
    /// Stickers are margin layers only; they never sit behind the text.
    pub const REGIONS: &[Region] = &[Region::Margin];
    /// The longest `char` is one emoji with its joiners and selectors.
    pub const MAX_BYTES: usize = 16;
    /// Tilts are steps, so each maps to one CSS class.
    pub const TILT_STEP: i32 = 15;
    pub const MAX_TILT: i32 = 45;
    /// The most stickers one scene entry may hold.
    pub const MAX_PER_WINDOW: usize = 6;

    /// True for characters a sticker may use: emoji, their joiners and
    /// variation selectors, and the symbol blocks they come from. This
    /// rules out letters, digits, whitespace, control and markup
    /// characters.
    pub fn char_is_sticker(c: char) -> bool {
        matches!(
            c,
            '\u{00A9}'
                | '\u{00AE}'
                | '\u{200D}'
                | '\u{20E3}'
                | '\u{2100}'..='\u{2BFF}'
                | '\u{3030}'
                | '\u{303D}'
                | '\u{3297}'
                | '\u{3299}'
                | '\u{FE0F}'
                | '\u{1F000}'..='\u{1FAFF}'
                | '\u{E0020}'..='\u{E007F}'
        )
    }
}
