//! The editor's scene panel: the backdrop image and up to six stickers
//! for one target window, checked live by the shell before anything is
//! saved. A layer this build does not understand is never shown for
//! editing, and rides through every save untouched.

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Key};
use posthaste_model::{
    Anchor, EmojiLayer, Fit, ImageLayer, Known, LayerRecord, SceneEdit, StickerSize, Window,
    WindowScene,
};

use crate::browser::bind_text;
use crate::components::{Button, Segmented, Switch};
use crate::ipc::api;
use crate::state::I18n;

/// The default sticker a new row starts with: valid on its own, easy to
/// change in place.
const NEW_STICKER: &str = "🌸";

/// Which scene entry the panel edits: the shared `all` entry, or one
/// window's own.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    All,
    Main,
    Capture,
    Today,
    Details,
}

impl Target {
    fn label(self) -> Key {
        match self {
            Target::All => key::LOOKS_EDITOR_TARGET_ALL,
            Target::Main => key::LOOKS_EDITOR_TARGET_MAIN,
            Target::Capture => key::LOOKS_EDITOR_TARGET_CAPTURE,
            Target::Today => key::LOOKS_EDITOR_TARGET_TODAY,
            Target::Details => key::LOOKS_EDITOR_TARGET_DETAILS,
        }
    }

    fn window(self) -> Option<Window> {
        match self {
            Target::All => None,
            Target::Main => Some(Window::Main),
            Target::Capture => Some(Window::Capture),
            Target::Today => Some(Window::Today),
            Target::Details => Some(Window::Details),
        }
    }

    const CHOICES: [Target; 5] = [
        Target::All,
        Target::Main,
        Target::Capture,
        Target::Today,
        Target::Details,
    ];
}

/// The editor's scene and everything the panel says about it.
#[derive(Clone, Copy)]
pub(super) struct SceneState {
    pub edit: RwSignal<SceneEdit>,
    /// Sticker and image counts the shell could not draw in the last
    /// preview.
    pub skipped: RwSignal<u32>,
    /// True once the look's real scene has loaded, or failed to. Before
    /// that the editor does not know the scene, so it sends none and
    /// Save is off: a save without the load would wipe the stored one.
    pub loaded: RwSignal<bool>,
}

impl SceneState {
    /// An empty scene for a new look; an existing look's real one loads
    /// right after. A failed load keeps the empty scene and the save
    /// then sends none, which leaves the stored scene as it is.
    pub fn new(editing_id: Option<String>) -> Self {
        let state = Self {
            edit: RwSignal::new(SceneEdit::default()),
            skipped: RwSignal::new(0),
            loaded: RwSignal::new(editing_id.is_none()),
        };
        if let Some(id) = editing_id {
            let edit = state.edit;
            let loaded = state.loaded;
            spawn_local(async move {
                if let Ok(loaded_scene) = api::look_scene(&id).await {
                    edit.set(loaded_scene);
                }
                // Either way the load is over: with it failed, sending
                // none is exactly what keeps the stored scene.
                loaded.set(true);
            });
        }
        state
    }
}

/// Runs `go` on the scene entry `target` points at.
fn with_entry<R>(
    edit: &mut SceneEdit,
    target: Target,
    go: impl FnOnce(&mut WindowScene) -> R,
) -> R {
    match target.window() {
        Some(window) => go(edit.scene.window_mut(window)),
        None => go(edit.scene.all.get_or_insert_with(WindowScene::default)),
    }
}

/// The image layer of an entry, if it has one.
fn backdrop_of(entry: &WindowScene) -> Option<ImageLayer> {
    entry.backdrop.iter().find_map(|layer| match layer.known() {
        Some(Known::Image(image)) => Some(image),
        _ => None,
    })
}

/// Drops every listed asset that no layer, in any entry, mentions any
/// more.
fn prune_assets(edit: &mut SceneEdit) {
    let used = |id: &str| edit.scene.layers().any(|(_, layer)| layer.mentions(id));
    edit.assets.retain(|asset| used(&asset.id));
}

/// The scene panel, below the colour form.
#[component]
pub(super) fn ScenePanel(i18n: I18n, state: SceneState) -> impl IntoView {
    let edit = state.edit;
    let target = RwSignal::new(Target::All);

    // The backdrop and the stickers of the chosen target, as typed
    // layers. Unknown layers are not in these lists and stay untouched.
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
    let stickers = Memo::new(move |_| {
        target.with(|target| {
            edit.with(|edit| {
                let entry = match *target {
                    Target::All => edit.scene.all.as_ref(),
                    other => other.window().and_then(|window| edit.scene.window(window)),
                };
                let Some(entry) = entry else {
                    return Vec::new();
                };
                entry
                    .margin
                    .iter()
                    .enumerate()
                    .filter_map(|(index, layer)| {
                        layer.known().map(|known| match known {
                            Known::Emoji(_) => Some((index, layer)),
                            Known::Image(_) => None,
                        })?
                    })
                    .map(|(index, layer)| (index, layer.clone()))
                    .collect()
            })
        })
    });
    let full = move || stickers.with(|list| list.len() >= EmojiLayer::MAX_PER_WINDOW);

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

    let add_sticker = move |()| {
        edit.update(|edit| {
            with_entry(edit, target.get_untracked(), |entry| {
                entry
                    .margin
                    .push(LayerRecord::from_known(&Known::Emoji(EmojiLayer {
                        char: NEW_STICKER.into(),
                        ..EmojiLayer::default()
                    })));
            });
        });
    };

    view! {
        <div class="look-editor__scene">
            <div class="look-editor__scene-title">
                {i18n.text(key::LOOKS_EDITOR_SCENE)}
            </div>
            <div class="look-editor__row">
                <label class="look-editor__label">
                    {i18n.text(key::LOOKS_EDITOR_SCENE_TARGET)}
                </label>
                <select
                    class="field select"
                    aria-label=i18n.text(key::LOOKS_EDITOR_SCENE_TARGET)
                    on:change=move |event| {
                        let value = event_target_value(&event);
                        target.set(match value.as_str() {
                            "main" => Target::Main,
                            "capture" => Target::Capture,
                            "today" => Target::Today,
                            "details" => Target::Details,
                            _ => Target::All,
                        });
                    }
                >
                    {Target::CHOICES
                        .into_iter()
                        .map(|choice| {
                            let value = match choice {
                                Target::All => "all",
                                Target::Main => "main",
                                Target::Capture => "capture",
                                Target::Today => "today",
                                Target::Details => "details",
                            };
                            let selected = move || target.get() == choice;
                            view! {
                                <option value=value selected=selected>
                                    {i18n.text(choice.label())}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </div>

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

            <div class="look-editor__row">
                <label class="look-editor__label">
                    {i18n.text(key::LOOKS_EDITOR_STICKERS)}
                </label>
                <div class="look-editor__stack">
                    {move || stickers.get().into_iter().map(|(index, layer)| {
                        let (remove, row) =
                            sticker_row(i18n, edit, target.get_untracked(), index, layer);
                        view! {
                            <div class="look-editor__sticker">
                                {row}
                                <Button
                                    variant=crate::components::Variant::Danger
                                    on_click=move |()| remove.run(())
                                >
                                    {i18n.text(key::LOOKS_EDITOR_REMOVE_LAYER)}
                                </Button>
                            </div>
                        }
                    }).collect_view()}
                    <Button disabled=Signal::derive(full) on_click=move |()| add_sticker(())>
                        {i18n.text(key::LOOKS_EDITOR_ADD_STICKER)}
                    </Button>
                </div>
            </div>
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

/// One sticker's controls. `index` is the layer's position in the
/// target's margin, which is what removing needs.
fn sticker_row(
    i18n: I18n,
    edit: RwSignal<SceneEdit>,
    target: Target,
    index: usize,
    sticker: posthaste_model::LayerRecord,
) -> (Callback<()>, impl IntoView) {
    let remove = move |()| {
        edit.update(|edit| {
            with_entry(edit, target, |entry| {
                if index < entry.margin.len() {
                    entry.margin.remove(index);
                }
            });
        });
    };
    let remove = Callback::new(remove);

    let Known::Emoji(sticker) = sticker.known().unwrap_or_else(|| {
        Known::Emoji(EmojiLayer {
            char: "🌸".into(),
            ..EmojiLayer::default()
        })
    }) else {
        panic!("only stickers are listed");
    };

    let char = RwSignal::new(sticker.char.clone());
    let set_char = move |event| {
        let value = event_target_value(&event);
        char.set(value.clone());
        edit.update(|edit| {
            with_entry(edit, target, |entry| {
                edit_layer(entry, index, |emoji| emoji.char = value.clone());
            });
        });
    };

    let faded = RwSignal::new(sticker.faded);
    let set_faded = move |on: bool| {
        faded.set(on);
        edit.update(|edit| {
            with_entry(edit, target, |entry| {
                edit_layer(entry, index, |emoji| emoji.faded = on);
            });
        });
    };

    let t = i18n.translator();
    let row = view! {
        <div class="look-editor__sticker-row">
            <input
                class="field"
                aria-label=t.text(key::LOOKS_EDITOR_STICKER_CHAR)
                maxlength=EmojiLayer::MAX_BYTES.to_string()
                spellcheck="false"
                use:bind_text=Signal::derive(move || char.get())
                on:input=set_char
            />
            <label class="look-editor__label">
                {i18n.text(key::LOOKS_EDITOR_STICKER_ANCHOR)}
            </label>
            {select_control(
                t.text(key::LOOKS_EDITOR_STICKER_ANCHOR).to_owned(),
                ANCHORS.into_iter().map(|(value, name)| (value, t.text(name).to_owned())).collect(),
                anchor_value,
                sticker.anchor,
                move |anchor| edit.update(|edit| {
                    with_entry(edit, target, |entry| {
                        edit_layer(entry, index, |emoji| emoji.anchor = anchor);
                    });
                }),
            )}
            <label class="look-editor__label">
                {i18n.text(key::LOOKS_EDITOR_STICKER_SIZE)}
            </label>
            {select_control(
                t.text(key::LOOKS_EDITOR_STICKER_SIZE).to_owned(),
                SIZES.into_iter().map(|(value, name)| (value, t.text(name).to_owned())).collect(),
                size_value,
                sticker.size,
                move |size| edit.update(|edit| {
                    with_entry(edit, target, |entry| {
                        edit_layer(entry, index, |emoji| emoji.size = size);
                    });
                }),
            )}
            <label class="look-editor__label">
                {i18n.text(key::LOOKS_EDITOR_STICKER_TILT)}
            </label>
            {tilt_control(
                t.text(key::LOOKS_EDITOR_STICKER_TILT).to_owned(),
                sticker.tilt,
                move |tilt| edit.update(|edit| {
                with_entry(edit, target, |entry| {
                    edit_layer(entry, index, |emoji| emoji.tilt = tilt);
                });
            }))}
            <label class="look-editor__label">
                {i18n.text(key::LOOKS_EDITOR_STICKER_FADED)}
            </label>
            <Switch
                label=i18n.label(key::LOOKS_EDITOR_STICKER_FADED)
                checked=faded
                on_change=set_faded
            />
        </div>
    };
    (remove, row)
}

/// Edits the emoji layer at `index`, keeping any config keys this build
/// does not model.
fn edit_layer(entry: &mut WindowScene, index: usize, go: impl FnOnce(&mut EmojiLayer)) {
    let Some(layer) = entry.margin.get_mut(index) else {
        return;
    };
    let mut typed = layer.known();
    if let Some(Known::Emoji(emoji)) = typed.as_mut() {
        go(emoji);
        layer.set_known(&Known::Emoji(emoji.clone()));
    }
}

const ANCHORS: [(Anchor, Key); 6] = [
    (Anchor::TopLeft, key::LOOKS_EDITOR_ANCHOR_TOP_LEFT),
    (Anchor::TopRight, key::LOOKS_EDITOR_ANCHOR_TOP_RIGHT),
    (Anchor::BottomLeft, key::LOOKS_EDITOR_ANCHOR_BOTTOM_LEFT),
    (Anchor::BottomRight, key::LOOKS_EDITOR_ANCHOR_BOTTOM_RIGHT),
    (Anchor::LeftMiddle, key::LOOKS_EDITOR_ANCHOR_LEFT_MIDDLE),
    (Anchor::RightMiddle, key::LOOKS_EDITOR_ANCHOR_RIGHT_MIDDLE),
];

const SIZES: [(StickerSize, Key); 4] = [
    (StickerSize::S, key::LOOKS_EDITOR_SIZE_S),
    (StickerSize::M, key::LOOKS_EDITOR_SIZE_M),
    (StickerSize::L, key::LOOKS_EDITOR_SIZE_L),
    (StickerSize::Xl, key::LOOKS_EDITOR_SIZE_XL),
];

fn anchor_value(anchor: Anchor) -> String {
    match anchor {
        Anchor::TopLeft => "top-left".into(),
        Anchor::TopRight => "top-right".into(),
        Anchor::BottomLeft => "bottom-left".into(),
        Anchor::BottomRight => "bottom-right".into(),
        Anchor::LeftMiddle => "left-middle".into(),
        Anchor::RightMiddle => "right-middle".into(),
    }
}

fn size_value(size: StickerSize) -> String {
    match size {
        StickerSize::S => "s".into(),
        StickerSize::M => "m".into(),
        StickerSize::L => "l".into(),
        StickerSize::Xl => "xl".into(),
    }
}

/// A plain select for the value sets that are too long for a segmented
/// control. `to_wire` names each value the way the document spells it.
fn select_control<T: Copy + PartialEq + Send + Sync + 'static>(
    label: String,
    options: Vec<(T, String)>,
    to_wire: impl Fn(T) -> String + Copy + 'static,
    current: T,
    on_change: impl Fn(T) + 'static,
) -> impl IntoView {
    let options = StoredValue::new(options);
    view! {
        <select
            class="field select"
            aria-label=label
            on:change=move |event| {
                let value = event_target_value(&event);
                for (option, _) in options.get_value().iter() {
                    if to_wire(*option) == value {
                        on_change(*option);
                        break;
                    }
                }
            }
        >
            {options
                .get_value()
                .into_iter()
                .map(|(value, name)| {
                    let wire = to_wire(value);
                    let is_selected = move || value == current;
                    view! {
                        <option value=wire selected=is_selected>{name}</option>
                    }
                })
                .collect_view()}
        </select>
    }
}

fn tilt_control(label: String, current: i32, on_change: impl Fn(i32) + 'static) -> impl IntoView {
    let steps: [i32; 7] = [-45, -30, -15, 0, 15, 30, 45];
    view! {
        <select
            class="field select"
            aria-label=label
            on:change=move |event| {
                if let Ok(tilt) = event_target_value(&event).parse() {
                    on_change(tilt);
                }
            }
        >
            {steps
                .into_iter()
                .map(|step| {
                    let is_selected = move || step == current;
                    view! {
                        <option value=step.to_string() selected=is_selected>
                            {format!("{step}°")}
                        </option>
                    }
                })
                .collect_view()}
        </select>
    }
}
