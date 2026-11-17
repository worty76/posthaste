//! Scene checks for a v2 document: one layer count for the whole look,
//! then every scene entry's layers against their kind's limits and
//! allowed regions. A layer of a kind or version this build does not know
//! is kept untouched: it is never drawn. A layer whose kind and version
//! this build knows, whose config it cannot read, is refused. When a kind
//! is added, its arm is written in the `match` below — no wildcard arm
//! may hide it.

use crate::error::{CoreError, Result};
use crate::look::assets::{MAX_ASSETS_PER_LOOK, MAX_ASSET_SIDE, MAX_ASSET_TOTAL_BYTES};
use crate::look::{
    AssetRef, EmojiLayer, ImageLayer, Known, Read, Region, Scene, WindowScene, MAX_LAYERS,
};

/// All scene checks, first failure wins.
pub(crate) fn check(scene: &Scene, assets: &[AssetRef]) -> Result<()> {
    check_assets(assets)?;
    let count = scene.layer_count();
    if count > MAX_LAYERS {
        return Err(CoreError::InvalidLook(format!(
            "A look can have at most {MAX_LAYERS} layers, not {count}."
        )));
    }
    let entries = scene
        .all
        .iter()
        .chain(scene.main.iter())
        .chain(scene.capture.iter())
        .chain(scene.today.iter())
        .chain(scene.details.iter());
    for entry in entries {
        check_entry(entry, assets)?;
    }
    Ok(())
}

/// The checks on the look's asset list itself: count, total size, and
/// well-formed ids.
fn check_assets(assets: &[AssetRef]) -> Result<()> {
    if assets.len() > MAX_ASSETS_PER_LOOK {
        return Err(CoreError::InvalidLook(format!(
            "A look can list at most {MAX_ASSETS_PER_LOOK} images, not {}.",
            assets.len()
        )));
    }
    let total: u64 = assets.iter().map(|a| a.bytes).sum();
    if total > MAX_ASSET_TOTAL_BYTES as u64 {
        return Err(CoreError::InvalidLook(
            "That look's images are more than 10 MB in total.".into(),
        ));
    }
    for asset in assets {
        let well_formed = match asset.id.strip_prefix("sha256:") {
            Some(hex) => {
                hex.len() == 64
                    && hex
                        .chars()
                        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
            }
            None => false,
        };
        if !well_formed {
            return Err(CoreError::InvalidLook(
                "That look has a malformed image id.".into(),
            ));
        }
        if asset.width > MAX_ASSET_SIDE || asset.height > MAX_ASSET_SIDE {
            return Err(CoreError::InvalidLook(format!(
                "That look has an image bigger than {MAX_ASSET_SIDE}×{MAX_ASSET_SIDE}."
            )));
        }
    }
    Ok(())
}

/// The checks for one scene entry (the `all` entry, or one window's own).
fn check_entry(entry: &WindowScene, assets: &[AssetRef]) -> Result<()> {
    let mut stickers = 0usize;
    for (region, layer) in entry.regions() {
        if layer.read() == Read::Malformed {
            return Err(CoreError::InvalidLook(
                "A layer of a kind this build knows has a config it cannot read.".into(),
            ));
        }
        match layer.known() {
            Some(Known::Image(image)) => check_image(region, image, assets)?,
            Some(Known::Emoji(emoji)) => {
                stickers += 1;
                check_emoji(region, emoji)?;
            }
            None => {}
        }
    }
    if stickers > EmojiLayer::MAX_PER_WINDOW {
        return Err(CoreError::InvalidLook(format!(
            "A window can have at most {} stickers, not {stickers}.",
            EmojiLayer::MAX_PER_WINDOW
        )));
    }
    Ok(())
}

fn check_image(region: Region, image: ImageLayer, assets: &[AssetRef]) -> Result<()> {
    if !ImageLayer::REGIONS.contains(&region) {
        return Err(CoreError::InvalidLook(
            "Image layers can only sit in a backdrop region.".into(),
        ));
    }
    if let Some(dim) = image.dim {
        if !(ImageLayer::MIN_DIM..=ImageLayer::MAX_DIM).contains(&dim) {
            return Err(CoreError::InvalidLook(format!(
                "An image's dim must be between {} and {}, not {dim}.",
                ImageLayer::MIN_DIM,
                ImageLayer::MAX_DIM
            )));
        }
    }
    if !assets.iter().any(|a| a.id == image.asset) {
        return Err(CoreError::InvalidLook(
            "That look's backdrop uses an image that is not in its assets list.".into(),
        ));
    }
    Ok(())
}

fn check_emoji(region: Region, emoji: EmojiLayer) -> Result<()> {
    if !EmojiLayer::REGIONS.contains(&region) {
        return Err(CoreError::InvalidLook(
            "Stickers can only sit in a margin region.".into(),
        ));
    }
    let bytes = emoji.char.len();
    if bytes == 0 || bytes > EmojiLayer::MAX_BYTES {
        return Err(CoreError::InvalidLook(format!(
            "A sticker is one emoji, up to {} bytes of text, not {bytes}.",
            EmojiLayer::MAX_BYTES
        )));
    }
    if !emoji.char.chars().all(EmojiLayer::char_is_sticker) {
        return Err(CoreError::InvalidLook(
            "A sticker must be an emoji, not a letter, digit or punctuation.".into(),
        ));
    }
    if emoji.tilt % EmojiLayer::TILT_STEP != 0 || emoji.tilt.abs() > EmojiLayer::MAX_TILT {
        return Err(CoreError::InvalidLook(format!(
            "A sticker's tilt is a multiple of {} up to {}, not {}.",
            EmojiLayer::TILT_STEP,
            EmojiLayer::MAX_TILT,
            emoji.tilt
        )));
    }
    Ok(())
}
