//! SceneHost: the active look's scene, drawn around a window's content.
//! Backdrop below, stickers above, everything inert. The interface only
//! renders what storage resolved and never parses a look.

mod emoji;
mod image;

use leptos::prelude::*;
use posthaste_model::{ResolvedLayer, Window};

use crate::state::{fetch_asset, trim_assets, use_asset_urls, use_scene, use_scene_preview};

/// Wraps a window's content in its scene: the backdrop layers are
/// absolutely positioned below the content, the stickers above it in the
/// margin band. Simple looks keeps the scene empty, so this draws
/// nothing while the setting is on.
#[component]
pub fn SceneHost(window: Window, children: Children) -> impl IntoView {
    let scene = use_scene();
    let preview = use_scene_preview();
    let urls = use_asset_urls();
    let layers = Memo::new(move |_| {
        // While the look editor previews a scene, this window draws it.
        preview.with(|p| {
            p.as_ref()
                .map(|scene| scene.window(window).clone())
                .unwrap_or_else(|| scene.with(|s| s.window(window).clone()))
        })
    });
    let has_backdrop = Memo::new(move |_| layers.with(|w| !w.backdrop.is_empty()));

    // Warm the URL cache for every image this window draws, and drop
    // the ones nothing draws any more: the active scene, plus the whole
    // preview when the editor holds one.
    Effect::new(move |_| {
        scene.with(|scene| {
            for window in [
                posthaste_model::Window::Main,
                posthaste_model::Window::Capture,
                posthaste_model::Window::Today,
                posthaste_model::Window::Details,
            ] {
                for layer in &scene.window(window).backdrop {
                    if let ResolvedLayer::Image { asset, .. } = layer {
                        fetch_asset(urls, asset.clone());
                    }
                }
            }
        });
        preview.with(|preview| {
            if let Some(preview) = preview {
                trim_assets(urls, preview);
            }
        });
        scene.with(|scene| trim_assets(urls, scene));
    });

    let backdrops = move || {
        layers.with(|w| {
            w.backdrop
                .iter()
                .cloned()
                .map(|layer| view! { <image::Renderer layer urls /> })
                .collect_view()
        })
    };
    let stickers = move || {
        layers.with(|w| {
            w.margin
                .iter()
                .cloned()
                .map(|layer| view! { <emoji::Renderer layer /> })
                .collect_view()
        })
    };

    view! {
        <div class="scene" class:scene-has-backdrop=has_backdrop>
            {backdrops}
            {children()}
            {stickers}
        </div>
    }
}
