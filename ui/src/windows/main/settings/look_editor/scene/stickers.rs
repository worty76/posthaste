//! The stickers section: up to six emoji in the target window's margin
//! band, each with anchor, size, tilt and a faded switch. Every edit
//! writes back through the typed layer, keeping config keys this build
//! does not model.

use leptos::prelude::*;
use posthaste_i18n::{key, Key};
use posthaste_model::{
    Anchor, EmojiLayer, Known, LayerRecord, SceneEdit, StickerSize, WindowScene,
};

use crate::browser::bind_text;
use crate::components::{Button, Switch, Variant};
use crate::state::I18n;

use super::{with_entry, Target};

/// The default sticker a new row starts with: valid on its own, easy to
/// change in place.
const NEW_STICKER: &str = "🌸";

/// The stickers of the target's scene entry, with the add button.
#[component]
pub(super) fn Section(
    i18n: I18n,
    edit: RwSignal<SceneEdit>,
    target: RwSignal<Target>,
) -> impl IntoView {
    // The stickers of the chosen target, as typed layers. Unknown layers
    // are not in this list and stay untouched.
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
                                variant=Variant::Danger
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
    }
}

/// One sticker's controls. `index` is the layer's position in the
/// target's margin, which is what removing needs.
fn sticker_row(
    i18n: I18n,
    edit: RwSignal<SceneEdit>,
    target: Target,
    index: usize,
    sticker: LayerRecord,
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
