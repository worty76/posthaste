//! The backdrop section: one image behind the target window's content,
//! with its fit and dimming. Choosing an image lists it in the look's
//! assets; removing it drops the listing once nothing mentions it.

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::{Fit, ImageLayer, Known, LayerRecord, SceneEdit};

use crate::components::{Button, Segmented};
use crate::ipc::api;
use crate::state::I18n;

use super::{backdrop_of, prune_assets, with_entry, Target};

/// The backdrop row of the target's scene entry.
#[component]
pub(super) fn Section(
    i18n: I18n,
    edit: RwSignal<SceneEdit>,
    target: RwSignal<Target>,
) -> impl IntoView {
    // The backdrop of the chosen target, as a typed layer. Unknown
    // layers are not shown and stay untouched.
    let backdrop = Memo::new(move |_| {
        target.with(|target| {
            edit.with(|edit| match target {
                Target::All => edit.scene.all.as_ref().and_then(backdrop_of),
                _ => target
                    .window()
                    .and_then(|window| edit.scene.window(window))
                    .and_then(backdrop_of),
            })
        })
    });

    let choose_image = move |()| {
        spawn_local(async move {
            // Cancelled or refused: nothing changes.
            let Some(asset) = api::look_asset_pick().await.ok().flatten() else {
                return;
            };
            edit.update(|edit| {
                let layer = LayerRecord::from_known(&Known::Image(ImageLayer {
                    asset: asset.id.clone(),
                    fit: Some(Fit::default()),
                    dim: Some(0.0),
                    ..ImageLayer::default()
                }));
                with_entry(edit, target.get_untracked(), |entry| {
                    entry.backdrop = vec![layer];
                });
                if !edit.assets.iter().any(|listed| listed.id == asset.id) {
                    edit.assets.push(asset);
                }
            });
        });
    };

    let remove_image = move |()| {
        edit.update(|edit| {
            with_entry(edit, target.get_untracked(), |entry| {
                entry.backdrop.clear();
            });
            prune_assets(edit);
        });
    };

    view! {
        <div class="look-editor__row">
            <label class="look-editor__label">
                {i18n.text(key::LOOKS_EDITOR_BACKDROP)}
            </label>
            {move || match backdrop.get() {
                Some(image) => {
                    let (fit_layer, dim_layer) = (image.clone(), image.clone());
                    view! {
                        <div class="look-editor__stack">
                            <Segmented
                                i18n=i18n
                                label=key::LOOKS_EDITOR_FIT
                                value=Signal::derive(move || fit_layer.fit.unwrap_or_default())
                                options=vec![
                                    (Fit::Cover, key::LOOKS_EDITOR_FIT_COVER),
                                    (Fit::Contain, key::LOOKS_EDITOR_FIT_CONTAIN),
                                    (Fit::Fill, key::LOOKS_EDITOR_FIT_FILL),
                                ]
                                on_change=move |fit| edit.update(|edit| {
                                    with_entry(edit, target.get_untracked(), |entry| {
                                        for layer in &mut entry.backdrop {
                                            if layer.kind == "image" {
                                                let mut typed = layer.known();
                                                if let Some(Known::Image(mut image)) = typed.take() {
                                                    image.fit = Some(fit);
                                                    layer.set_known(&Known::Image(image));
                                                }
                                            }
                                        }
                                    });
                                })
                            />
                            <label class="look-editor__label">
                                {i18n.text(key::LOOKS_EDITOR_DIM)}
                            </label>
                            {dim_control(i18n, dim_layer, edit, target.get_untracked())}
                            <Button on_click=remove_image>
                                {i18n.text(key::LOOKS_EDITOR_REMOVE_IMAGE)}
                            </Button>
                        </div>
                    }
                    .into_any()
                }
                None => view! {
                    <Button on_click=move |()| choose_image(())>
                        {i18n.text(key::LOOKS_EDITOR_CHOOSE_IMAGE)}
                    </Button>
                }
                .into_any(),
            }}
        </div>
    }
}

/// The dimming select, in tenths from none to fully dark.
fn dim_control(
    i18n: I18n,
    image: ImageLayer,
    edit: RwSignal<SceneEdit>,
    target: Target,
) -> impl IntoView {
    let current = image
        .dim
        .map(|dim| (dim * 10.0).round() as i32)
        .unwrap_or(0);
    let label = i18n.text(key::LOOKS_EDITOR_DIM);
    view! {
        <select
            class="field select"
            aria-label=label
            on:change=move |event| {
                let tenths: i32 = event_target_value(&event).parse().unwrap_or(0);
                edit.update(|edit| {
                    with_entry(edit, target, |entry| {
                        for layer in &mut entry.backdrop {
                            if layer.kind == "image" {
                                let mut typed = layer.known();
                                if let Some(Known::Image(mut image)) = typed.take() {
                                    image.dim = Some(f64::from(tenths) / 10.0);
                                    layer.set_known(&Known::Image(image));
                                }
                            }
                        }
                    });
                });
            }
        >
            {(0..=10).map(|tenths| {
                let selected = move || tenths == current;
                view! {
                    <option value=tenths.to_string() selected=selected>
                        {format!("{tenths}/10")}
                    </option>
                }
            }).collect_view()}
        </select>
    }
}
