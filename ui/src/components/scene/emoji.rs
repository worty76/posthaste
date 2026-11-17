//! The sticker renderer: one classed `<span>` in the margin band. The
//! stylesheet positions it by anchor, sizes and tilts it in steps, and
//! clamps it into the band, so a sticker can never reach the text.

use leptos::prelude::*;
use posthaste_model::{Anchor, ResolvedLayer, StickerSize};

/// The anchor names the stylesheet positions by.
fn anchor_name(anchor: Anchor) -> &'static str {
    match anchor {
        Anchor::TopLeft => "top-left",
        Anchor::TopRight => "top-right",
        Anchor::BottomLeft => "bottom-left",
        Anchor::BottomRight => "bottom-right",
        Anchor::LeftMiddle => "left-middle",
        Anchor::RightMiddle => "right-middle",
    }
}

/// The size classes.
fn size_name(size: StickerSize) -> &'static str {
    match size {
        StickerSize::S => "s",
        StickerSize::M => "m",
        StickerSize::L => "l",
        StickerSize::Xl => "xl",
    }
}

/// The tilt classes: n marks negative degrees.
fn tilt_name(tilt: i32) -> String {
    if tilt < 0 {
        format!("n{}", -tilt)
    } else {
        format!("{tilt}")
    }
}

/// One sticker.
#[component]
pub fn Renderer(layer: ResolvedLayer) -> impl IntoView {
    let ResolvedLayer::Emoji {
        char,
        anchor,
        size,
        tilt,
        faded,
    } = layer
    else {
        return None;
    };
    let class = format!(
        "scene-sticker scene-sticker--{} scene-sticker--size-{} scene-sticker--tilt-{}{}",
        anchor_name(anchor),
        size_name(size),
        tilt_name(tilt),
        if faded { " scene-sticker--faded" } else { "" },
    );
    Some(view! {
        <span class=class aria-hidden="true">{char}</span>
    })
}
