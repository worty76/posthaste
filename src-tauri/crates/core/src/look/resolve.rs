//! Resolves a look's scene into the drawable layers the interface gets,
//! the way `derive.rs` resolves colours into tokens. It never fails: a
//! layer this build does not draw is skipped and counted (invariant 7).

use posthaste_model::{AssetRef, LookDocV2, Scene, WindowScene};
use posthaste_model::{
    Known, Read, ResolvedLayer, ResolvedScene, ResolvedWindow, SceneEdit, Window,
};

/// Resolves the scene of a validated v2 document.
pub fn resolve_scene(doc: &LookDocV2) -> ResolvedScene {
    resolve_parts(&doc.scene, &doc.assets)
}

/// Resolves a scene plus its asset list — what the editor's live preview
/// sends instead of a whole document.
pub fn resolve_parts(scene: &Scene, assets: &[AssetRef]) -> ResolvedScene {
    let mut skipped = 0u32;
    let mut build = |target: &mut ResolvedWindow, own: Option<&WindowScene>| {
        if let Some(all) = &scene.all {
            push_entry(target, all, assets, &mut skipped);
        }
        if let Some(own) = own {
            push_entry(target, own, assets, &mut skipped);
        }
    };
    let mut out = ResolvedScene::default();
    build(&mut out.main, scene.main.as_ref());
    build(&mut out.capture, scene.capture.as_ref());
    build(&mut out.today, scene.today.as_ref());
    build(&mut out.details, scene.details.as_ref());
    out.skipped = skipped;
    out
}

/// One scene entry into a window's drawable layers.
fn push_entry(
    out: &mut ResolvedWindow,
    entry: &WindowScene,
    assets: &[AssetRef],
    skipped: &mut u32,
) {
    for (region, layer) in entry.regions() {
        // An off layer draws nowhere, in no window.
        if !layer.on {
            *skipped += 1;
            continue;
        }
        match layer.read() {
            Read::Known(Known::Image(image)) => {
                if !assets.iter().any(|a| a.id == image.asset) {
                    *skipped += 1;
                    continue;
                }
                let dim_tenths = image
                    .dim
                    .map(|d| ((d * 10.0).round().clamp(0.0, 10.0)) as u8)
                    .unwrap_or(0);
                out.backdrop.push(ResolvedLayer::Image {
                    asset: image.asset.clone(),
                    fit: image.fit.unwrap_or_default(),
                    dim_tenths,
                });
            }
            Read::Known(Known::Emoji(emoji)) => {
                let _ = region; // the kind decides which list it draws in
                out.margin.push(ResolvedLayer::Emoji {
                    char: emoji.char.clone(),
                    anchor: emoji.anchor,
                    size: emoji.size,
                    tilt: emoji.tilt,
                    faded: emoji.faded,
                });
            }
            Read::Future | Read::Malformed => *skipped += 1,
        }
    }
}

/// The window entries in document order, for `SceneEdit` round trips.
pub fn scene_windows(scene: &Scene) -> [Option<&WindowScene>; 4] {
    [
        scene.main.as_ref(),
        scene.capture.as_ref(),
        scene.today.as_ref(),
        scene.details.as_ref(),
    ]
}

/// The names of the four windows, in `ResolvedScene` field order.
pub const WINDOWS: [Window; 4] = [
    Window::Main,
    Window::Capture,
    Window::Today,
    Window::Details,
];

/// The edit for a look with no scene: what a v1 look answers.
pub fn empty_edit() -> SceneEdit {
    SceneEdit::default()
}
