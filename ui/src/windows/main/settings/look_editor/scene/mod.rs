//! The editor's scene panel: the backdrop image and up to six stickers
//! for one target window, checked live by the shell before anything is
//! saved. A layer this build does not understand is never shown for
//! editing, and rides through every save untouched. The backdrop and
//! the stickers each live in their own file.

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Key};
use posthaste_model::{SceneEdit, Window, WindowScene};

use crate::ipc::api;
use crate::state::I18n;

mod backdrop;
mod stickers;

/// Which scene entry the panel edits: the shared `all` entry, or one
/// window's own.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
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
fn backdrop_of(entry: &WindowScene) -> Option<posthaste_model::ImageLayer> {
    entry.backdrop.iter().find_map(|layer| match layer.known() {
        Some(posthaste_model::Known::Image(image)) => Some(image),
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

            <backdrop::Section i18n edit target />
            <stickers::Section i18n edit target />
        </div>
    }
}
