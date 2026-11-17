//! Scene checks for a v2 document: one layer count for the whole look,
//! then every layer against its kind's limits and allowed regions. A
//! layer of a kind or version this build does not know is kept untouched:
//! it is never drawn. When a kind is added, its arm is written in the
//! `match` below — no wildcard arm may hide it.

use crate::error::{CoreError, Result};
use crate::look::{ImageLayer, Known, Region, Scene, MAX_LAYERS};

/// All scene checks, first failure wins.
pub(crate) fn check(scene: &Scene) -> Result<()> {
    let count = scene.layer_count();
    if count > MAX_LAYERS {
        return Err(CoreError::InvalidLook(format!(
            "A look can have at most {MAX_LAYERS} layers, not {count}."
        )));
    }
    for (region, layer) in scene.layers() {
        match layer.known() {
            Some(Known::Image(image)) => check_image(region, image)?,
            None => {}
        }
    }
    Ok(())
}

fn check_image(region: Region, image: ImageLayer) -> Result<()> {
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
    Ok(())
}
