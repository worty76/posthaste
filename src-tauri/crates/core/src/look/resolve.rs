//! Resolves a look's scene into the drawable layers the interface gets,
//! the way `derive.rs` resolves colours into tokens. It never fails: a
//! layer this build does not draw is skipped and counted (invariant 7).

use posthaste_model::{AssetRef, EmojiLayer, LookDoc, Scene, WindowScene};
use posthaste_model::{Known, Read, ResolvedLayer, ResolvedScene, SceneEdit, Window};

/// Resolves the scene of a validated v2 document as this build draws
/// it: stickers are not shipped yet, so emoji layers are left out
/// quietly. They stay in the document and storage still checks them.
pub fn resolve_scene(doc: &LookDoc) -> ResolvedScene {
    resolve_live(&doc.scene, &doc.assets)
}

/// Resolves a scene plus its asset list the way the app will draw it.
pub fn resolve_live(scene: &Scene, assets: &[AssetRef]) -> ResolvedScene {
    resolve_parts(&without_unshipped_stickers(scene), assets)
}

/// A copy of `scene` without emoji layers, while stickers are unshipped.
/// Silently left out, not counted as skipped: this build knows the kind,
/// it just does not draw it yet.
fn without_unshipped_stickers(scene: &Scene) -> Scene {
    if EmojiLayer::SHIPPED {
        return scene.clone();
    }
    let mut scene = scene.clone();
    for entry in [
        &mut scene.all,
        &mut scene.main,
        &mut scene.capture,
        &mut scene.today,
        &mut scene.details,
    ]
    .into_iter()
    .flatten()
    {
        entry
            .margin
            .retain(|layer| !matches!(layer.known(), Some(Known::Emoji(_))));
    }
    scene
}

/// Resolves a scene plus its asset list — what the editor's live preview
/// sends instead of a whole document.
pub fn resolve_parts(scene: &Scene, assets: &[AssetRef]) -> ResolvedScene {
    let mut skipped = 0u32;
    let mut out = ResolvedScene::default();
    let windows = [
        (&mut out.main, scene.main.as_ref()),
        (&mut out.capture, scene.capture.as_ref()),
        (&mut out.today, scene.today.as_ref()),
        (&mut out.details, scene.details.as_ref()),
    ];
    for (target, own) in windows {
        // A window draws `all` first, then its own layers. Stickers are
        // capped across the merge, and the window's own ones count
        // first: they were chosen for this window.
        let shared = scene
            .all
            .as_ref()
            .map(|entry| resolve_entry(entry, assets, &mut skipped))
            .unwrap_or_default();
        let own = own
            .map(|entry| resolve_entry(entry, assets, &mut skipped))
            .unwrap_or_default();
        let shared_keep = EmojiLayer::MAX_PER_WINDOW
            .saturating_sub(own.stickers.len())
            .min(shared.stickers.len());
        skipped += u32::try_from(shared.stickers.len() - shared_keep).unwrap_or(0);
        skipped += u32::try_from(
            own.stickers
                .len()
                .saturating_sub(EmojiLayer::MAX_PER_WINDOW),
        )
        .unwrap_or(0);
        target.backdrop = [shared.backdrop, own.backdrop].concat();
        target.margin = [shared.stickers[..shared_keep].to_vec(), own.stickers].concat();
    }
    out.skipped = skipped;
    out
}

/// One scene entry resolved into its drawable layers. Backdrop images
/// collect in `backdrop`, stickers in `stickers`; anything this build
/// does not draw is counted.
#[derive(Default)]
struct Entry {
    backdrop: Vec<ResolvedLayer>,
    stickers: Vec<ResolvedLayer>,
}

fn resolve_entry(entry: &WindowScene, assets: &[AssetRef], skipped: &mut u32) -> Entry {
    let mut out = Entry::default();
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
                out.stickers.push(ResolvedLayer::Emoji {
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
    out
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
