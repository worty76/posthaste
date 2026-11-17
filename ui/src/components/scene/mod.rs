//! SceneHost: the active look's scene, drawn around a window's content.
//! Backdrop below, stickers above, everything inert. The interface only
//! renders what storage resolved and never parses a look.

mod emoji;
mod image;

use leptos::prelude::*;
use posthaste_model::{ResolvedLayer, Window};

use crate::state::{fetch_asset, use_asset_urls, use_scene};

/// Wraps a window's content in its scene: the backdrop layers are
/// absolutely positioned below the content, the stickers above it in the
/// margin band. Simple looks keeps the scene empty, so this draws
/// nothing while the setting is on.
#[component]
pub fn SceneHost(window: Window, children: Children) -> impl IntoView {
    let scene = use_scene();
    let urls = use_asset_urls();
    let layers = Memo::new(move |_| scene.with(|s| s.window(window).clone()));
    let has_backdrop = Memo::new(move |_| layers.with(|w| !w.backdrop.is_empty()));

    // Warm the URL cache for every image this window draws.
    Effect::new(move |_| {
        layers.with(|w| {
            for layer in &w.backdrop {
                if let ResolvedLayer::Image { asset, .. } = layer {
                    fetch_asset(urls, asset.clone());
                }
            }
        });
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
