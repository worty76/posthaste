//! The active scene and the `data:` URLs its images are drawn from. One
//! load per window page, kept current from the shell's events. Like the
//! rest of `state/`, this only hands checked values to the interface: it
//! never parses a look or reads a file.

use std::cell::RefCell;
use std::collections::HashMap;

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_model::ResolvedScene;

use crate::ipc::{api, events};

thread_local! {
    /// The page's scene, loaded once and refreshed on the shell's events.
    static SCENE: RefCell<Option<RwSignal<ResolvedScene>>> = const { RefCell::new(None) };
    /// `data:` URLs by asset id. An empty string is a fetch in flight or
    /// one that failed: both draw nothing and fetch no more.
    static ASSETS: RefCell<Option<RwSignal<HashMap<String, String>>>> =
        const { RefCell::new(None) };
}

/// The scene of the look the user wears, kept current. Simple looks
/// arrives as an empty scene from storage, so this signal is empty while
/// the setting is on, in every window, live.
pub fn use_scene() -> RwSignal<ResolvedScene> {
    SCENE.with_borrow_mut(|slot| {
        *slot.get_or_insert_with(|| {
            let scene = RwSignal::new(ResolvedScene::default());
            let reload = move || {
                spawn_local(async move {
                    // A rejected lookup leaves the scene empty, the same
                    // way a corrupt look row is skipped.
                    if let Ok(resolved) = api::look_active_scene().await {
                        if let Some(scene) = SCENE.with_borrow(|slot| slot.as_ref().copied()) {
                            scene.set(resolved);
                        }
                    }
                });
            };
            reload();
            events::on::<events::LooksChanged>(move |_| reload());
            events::on::<events::SettingsChanged>(move |_| reload());
            scene
        })
    })
}

/// The page's cache of asset `data:` URLs, shared by every scene layer.
pub fn use_asset_urls() -> RwSignal<HashMap<String, String>> {
    ASSETS.with_borrow_mut(|slot| *slot.get_or_insert_with(|| RwSignal::new(HashMap::new())))
}

/// Fetches one asset's URL into the cache, once. The view redraws when
/// the answer lands.
pub fn fetch_asset(urls: RwSignal<HashMap<String, String>>, id: String) {
    if urls.with(|map| map.contains_key(&id)) {
        return;
    }
    urls.update(|map| {
        map.insert(id.clone(), String::new());
    });
    spawn_local(async move {
        let url = api::look_asset(&id).await.unwrap_or_default();
        urls.update(|map| {
            map.insert(id, url);
        });
    });
}
