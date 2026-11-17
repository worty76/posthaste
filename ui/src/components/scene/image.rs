//! The backdrop image renderer: one `<img>` under a dimming veil, both
//! inert. An asset that is missing or still loading draws nothing.

use std::collections::HashMap;

use leptos::prelude::*;
use posthaste_model::{Fit, ResolvedLayer};

use crate::state::fetch_asset;

/// The fit names the stylesheet selects on.
fn fit_name(fit: Fit) -> &'static str {
    match fit {
        Fit::Cover => "cover",
        Fit::Contain => "contain",
        Fit::Fill => "fill",
    }
}

/// One backdrop layer: a classed `<div>` holding the image, and the dim
/// as a tenth-step class the stylesheet turns into a veil opacity.
#[component]
pub fn Renderer(layer: ResolvedLayer, urls: RwSignal<HashMap<String, String>>) -> impl IntoView {
    let ResolvedLayer::Image {
        asset,
        fit,
        dim_tenths,
    } = layer
    else {
        return None;
    };
    fetch_asset(urls, asset.clone());
    let src = Memo::new(move |_| {
        urls.with(|map| map.get(&asset).cloned())
            .filter(|url| !url.is_empty())
    });
    let class = move || {
        format!(
            "scene-backdrop scene-backdrop--fit-{} scene-dim--{dim_tenths}",
            fit_name(fit)
        )
    };
    Some(view! {
        <div class=class aria-hidden="true">
            <Show when=move || src.with(|url| url.is_some())>
                <img src=src />
            </Show>
        </div>
    })
}
