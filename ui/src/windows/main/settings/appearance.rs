//! Light or dark, and the gallery of looks: pick one, share one, add one
//! from a code, or make your own.

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::{CommandError, LookPreview, LookSummary, Theme, ThemeEdit};

use super::look_editor::LookEditor;
use super::Context;
use crate::browser::{self, bind_text};
use crate::components::{Button, Dialog, Segmented, Switch, Variant};
use crate::ipc::{self, api, events};
use crate::logic::look::fit_name;
use crate::state::{seed_assets, use_asset_urls, use_scene_preview, Appearance, Message};
use posthaste_model::LookFilePreview;

/// What "Make your own" starts from when the Plain look is selected: the
/// bundled look that matches how the window is drawn.
const START_DARK: &str = "builtin:midnight-grape";
const START_LIGHT: &str = "builtin:strawberry-milk";

/// A look open in the editor.
#[derive(Clone)]
struct Editing {
    theme: ThemeEdit,
    /// The look being changed, or `None` for a new one.
    id: Option<String>,
}

#[component]
pub(super) fn AppearanceSection(context: Context, appearance: Appearance) -> impl IntoView {
    let i18n = context.i18n;
    let toaster = context.toaster;
    let looks = RwSignal::new(Vec::<LookSummary>::new());
    let look_id = context.read(|s| s.look_id.clone());
    let import_open = RwSignal::new(false);
    let import_code = RwSignal::new(String::new());
    let import_error = RwSignal::new(None::<CommandError>);
    let import_busy = RwSignal::new(false);
    let delete_target = RwSignal::new(None::<LookSummary>);
    let delete_busy = RwSignal::new(false);
    let editor = RwSignal::new(None::<Editing>);
    let file_preview = RwSignal::new(None::<LookFilePreview>);
    let file_busy = RwSignal::new(false);
    let file_installed = RwSignal::new(false);
    let scene_preview = use_scene_preview();

    let reload = move || {
        spawn_local(async move {
            // A failed read keeps the gallery as it is.
            if let Ok(found) = api::look_list().await {
                looks.set(found);
            }
        });
    };
    reload();
    events::on::<events::LooksChanged>(move |()| reload());

    let selected = Memo::new(move |_| {
        let id = look_id.get()?;
        looks.with(|looks| looks.iter().find(|look| look.id == id).cloned())
    });
    let editing = Signal::derive(move || editor.with(Option::is_some));

    // A preview belongs to the editor; picking a look ends it.
    let select = move |id: Option<String>| {
        appearance.preview(None);
        context.set(move |s| s.look_id = id);
    };
    // The arrow keys walk the gallery, Plain included, and wrap.
    let on_key = move |event: KeyboardEvent| {
        let forward = match event.key().as_str() {
            "ArrowRight" | "ArrowDown" => true,
            "ArrowLeft" | "ArrowUp" => false,
            _ => return,
        };
        event.prevent_default();
        let mut ids: Vec<Option<String>> =
            looks.with_untracked(|looks| looks.iter().map(|look| Some(look.id.clone())).collect());
        ids.push(None);
        let current = look_id.get_untracked();
        let index = ids.iter().position(|id| *id == current).unwrap_or(0);
        let step = if forward { 1 } else { ids.len() - 1 };
        select(ids.swap_remove((index + step) % ids.len()));
    };

    let share = move |look: LookSummary| {
        spawn_local(async move {
            match api::look_export_code(&look.id).await {
                Ok(code) if browser::copy_text(&code).await => match look.font_family.as_deref() {
                    Some(font) => {
                        toaster.info(Message::with(key::LOOKS_SHARED_WITH_FONT, "font", font))
                    }
                    None => toaster.info(Message::Plain(key::LOOKS_SHARED)),
                },
                Ok(_) => toaster.error(Message::Plain(key::INBOX_TOAST_COPY_FAILED)),
                Err(error) => toaster.fail(error),
            }
        });
    };

    // Opens a look file: every byte is checked in the shell, and the
    // answer carries the whole preview. The windows wear it until the
    // dialog closes, exactly as they wear an edit-in-progress.
    let open_file = move || {
        spawn_local(async move {
            match api::look_file_open().await {
                Ok(Some(preview)) => {
                    seed_assets(use_asset_urls(), &preview.asset_urls);
                    appearance.preview(Some(preview.tokens.clone()));
                    scene_preview.set(Some(preview.scene.clone()));
                    file_installed.set(false);
                    file_preview.set(Some(preview));
                }
                Ok(None) => {}
                Err(error) => toaster.fail(error),
            }
        });
    };

    let close_file = move |()| {
        if file_preview.with(Option::is_some) && !file_installed.get_untracked() {
            spawn_local(async {
                let _ = api::look_file_discard().await;
            });
        }
        file_preview.set(None);
        appearance.preview(None);
        scene_preview.set(None);
    };

    let install_file = move |()| {
        file_busy.set(true);
        spawn_local(async move {
            match api::look_file_install().await {
                Ok(saved) => {
                    file_installed.set(true);
                    file_preview.set(None);
                    appearance.preview(None);
                    scene_preview.set(None);
                    toaster.info(Message::with(key::LOOKS_SAVED, "name", saved.name));
                    context.set(move |s| s.look_id = Some(saved.id));
                }
                Err(error) => {
                    file_busy.set(false);
                    toaster.fail(error);
                }
            }
        });
    };

    let export_file = move |look: LookSummary| {
        spawn_local(async move {
            match api::look_file_export(&look.id).await {
                Ok(Some(_)) => toaster.info(Message::Plain(key::LOOKS_EXPORTED)),
                Ok(None) => {}
                Err(error) => toaster.fail(error),
            }
        });
    };

    let close_import = move |()| {
        import_open.set(false);
        import_code.set(String::new());
        import_error.set(None);
    };
    let run_import = move |()| {
        import_busy.set(true);
        spawn_local(async move {
            match api::look_import(&import_code.get_untracked()).await {
                Ok(added) => {
                    close_import(());
                    toaster.info(Message::with(key::LOOKS_ADDED, "name", added.name));
                    context.set(move |s| s.look_id = Some(added.id));
                }
                // The dialog stays open with the code, to fix or try again.
                Err(error) => import_error.set(Some(error)),
            }
            import_busy.set(false);
        });
    };

    let run_delete = move |()| {
        let Some(target) = delete_target.get_untracked() else {
            return;
        };
        delete_busy.set(true);
        spawn_local(async move {
            match api::look_delete(&target.id).await {
                Ok(()) => delete_target.set(None),
                Err(error) => toaster.fail(error),
            }
            delete_busy.set(false);
        });
    };

    // Opens the editor on `look`: its own document to edit, or a copy when
    // it is bundled. With no look, a new one from a bundled starting point.
    let open_editor = move |look: Option<LookSummary>| {
        spawn_local(async move {
            let start = match &look {
                Some(look) => look.id.clone(),
                None if appearance.is_dark() => START_DARK.to_owned(),
                None => START_LIGHT.to_owned(),
            };
            match api::look_theme(&start).await {
                Ok(mut theme) => {
                    let copying = look.as_ref().is_some_and(|look| look.builtin);
                    if copying {
                        let copy = i18n
                            .translator()
                            .format(key::LOOKS_COPY_NAME, &[("name", &theme.name)]);
                        theme.name = fit_name(&copy);
                    }
                    let id = look.filter(|look| !look.builtin).map(|look| look.id);
                    editor.set(Some(Editing { theme, id }));
                }
                Err(error) => toaster.fail(error),
            }
        });
    };

    let look_button =
        move |id: Option<String>, name: Signal<String>, colors: Option<LookPreview>| {
            let checked = {
                let id = id.clone();
                move || look_id.with(|current| *current == id)
            };
            let is_checked = checked.clone();
            view! {
                <button
                    type="button"
                    role="radio"
                    class="look"
                    aria-checked=move || checked().to_string()
                    tabindex=move || if is_checked() { "0" } else { "-1" }
                    on:click=move |_| select(id.clone())
                >
                    <Swatch colors=colors />
                    <span>{move || name.get()}</span>
                </button>
            }
        };

    view! {
        <div class="row">
            <div class="row__text">
                <div class="row__label">{i18n.text(key::LOOKS_THEME)}</div>
                <div class="row__help">{i18n.text(key::LOOKS_THEME_HELP)}</div>
            </div>
            <div class="row__control">
                <Segmented
                    i18n=i18n
                    label=key::LOOKS_THEME
                    value=context.read(|s| s.theme)
                    options=vec![
                        (Theme::System, key::LOOKS_SYSTEM),
                        (Theme::Light, key::LOOKS_LIGHT),
                        (Theme::Dark, key::LOOKS_DARK),
                    ]
                    on_change=move |theme| context.set(move |s| s.theme = theme)
                />
            </div>
        </div>
        <div class="row">
            <div class="row__text">
                <div class="row__label">{i18n.text(key::LOOKS_SIMPLE)}</div>
                <div class="row__help">{i18n.text(key::LOOKS_SIMPLE_HELP)}</div>
            </div>
            <div class="row__control">
                <Switch
                    label=i18n.label(key::LOOKS_SIMPLE)
                    checked=context.read(|s| s.simple_looks)
                    on_change=move |on| context.set(move |s| s.simple_looks = on)
                />
            </div>
        </div>
        <div class="row row--stack">
            <div class="looks" role="radiogroup" aria-label=i18n.text(key::LOOKS_GALLERY) on:keydown=on_key>
                <For each=move || looks.get() key=|look| (look.id.clone(), look.name.clone(), look.preview.clone()) let:look>
                    {look_button(Some(look.id), Signal::stored(look.name), Some(look.preview))}
                </For>
                // No look: plain light or dark, following the setting above.
                // Ink, first in the list, is what every install starts with.
                {look_button(None, i18n.label(key::LOOKS_PLAIN), None)}
            </div>
            <div class="looks__actions">
                <Button on_click=open_file>
                    {i18n.text(key::LOOKS_OPEN_FILE)}
                </Button>
                {move || selected.get().map(|look| {
                    let builtin = look.builtin;
                    let (to_share, to_edit, to_delete) = (look.clone(), look.clone(), look);
                    view! {
                        <Button on_click=move |()| share(to_share.clone())>
                            {i18n.text(key::LOOKS_SHARE)}
                        </Button>
                        {if builtin {
                            view! {
                                <Button disabled=editing on_click=move |()| open_editor(Some(to_edit.clone()))>
                                    {i18n.text(key::LOOKS_COPY)}
                                </Button>
                            }
                            .into_any()
                        } else {
                            view! {
                                <Button disabled=editing on_click=move |()| open_editor(Some(to_edit.clone()))>
                                    {i18n.text(key::LOOKS_EDIT)}
                                </Button>
                                <Button
                                    variant=Variant::Danger
                                    disabled=editing
                                    on_click=move |()| delete_target.set(Some(to_delete.clone()))
                                >
                                    {i18n.text(key::LOOKS_DELETE)}
                                </Button>
                            }
                            .into_any()
                        }}
                    }
                })}
                {move || selected.get().filter(|look| !look.builtin).map(|look| {
                    let to_export = look.clone();
                    view! {
                        <Button on_click=move |()| export_file(to_export.clone())>
                            {i18n.text(key::LOOKS_EXPORT_FILE)}
                        </Button>
                    }
                })}
                <Button on_click=move |()| import_open.set(true)>{i18n.text(key::LOOKS_ADD)}</Button>
                <Button disabled=editing on_click=move |()| open_editor(selected.get_untracked())>
                    {i18n.text(key::LOOKS_MAKE)}
                </Button>
            </div>
        </div>

        <Dialog
            open=import_open
            title=i18n.label(key::LOOKS_IMPORT_TITLE)
            on_close=close_import
            actions=Box::new(move || view! {
                <Button on_click=close_import>{i18n.text(key::COMMON_CANCEL)}</Button>
                <Button
                    variant=Variant::Primary
                    busy=import_busy
                    disabled=Signal::derive(move || import_code.with(|code| code.trim().is_empty()))
                    on_click=run_import
                >
                    {i18n.text(key::LOOKS_IMPORT_ACTION)}
                </Button>
            }.into_any())
        >
            <p class="dialog__text">{i18n.text(key::LOOKS_IMPORT_TEXT)}</p>
            <textarea
                class="textarea"
                aria-label=i18n.text(key::LOOKS_IMPORT_FIELD)
                rows="4"
                spellcheck="false"
                use:bind_text=Signal::derive(move || import_code.get())
                on:input=move |event| {
                    import_code.set(event_target_value(&event));
                    import_error.set(None);
                }
            ></textarea>
            {move || import_error.get().map(|error| view! {
                <p class="dialog__alert" role="alert">{ipc::message(&error, i18n.translator())}</p>
            })}
        </Dialog>

        <Dialog
            open=Signal::derive(move || delete_target.with(Option::is_some))
            title=Signal::derive(move || {
                let name = delete_target.with(|target| target.as_ref().map(|t| t.name.clone()));
                i18n.translator()
                    .format(key::LOOKS_REMOVE_TITLE, &[("name", &name.unwrap_or_default())])
            })
            on_close=move |()| delete_target.set(None)
            actions=Box::new(move || view! {
                <Button on_click=move |()| delete_target.set(None)>{i18n.text(key::COMMON_CANCEL)}</Button>
                <Button variant=Variant::Danger busy=delete_busy on_click=run_delete>
                    {i18n.text(key::LOOKS_DELETE)}
                </Button>
            }.into_any())
        >
            <p class="dialog__text">{i18n.text(key::LOOKS_REMOVE_TEXT)}</p>
        </Dialog>

        <Dialog
            open=Signal::derive(move || file_preview.with(Option::is_some))
            title=i18n.label(key::LOOKS_FILE_TITLE)
            on_close=close_file
            actions=Box::new(move || view! {
                <Button on_click=close_file>{i18n.text(key::COMMON_CANCEL)}</Button>
                <Button
                    variant=Variant::Primary
                    busy=file_busy
                    on_click=install_file
                >
                    {i18n.text(key::LOOKS_FILE_INSTALL)}
                </Button>
            }.into_any())
        >
            <p class="dialog__text">{i18n.text(key::LOOKS_FILE_TEXT)}</p>
        </Dialog>

        {move || editor.get().map(|editing| view! {
            <LookEditor
                i18n=i18n
                appearance=appearance
                initial=editing.theme
                simple_looks=context.read(|s| s.simple_looks)
                editing_id=editing.id
                on_close=move |()| editor.set(None)
                on_saved=move |saved: LookSummary| {
                    toaster.info(Message::with(key::LOOKS_SAVED, "name", saved.name));
                    context.set(move |s| s.look_id = Some(saved.id));
                }
            />
        })}
    }
}

/// The small colour card in the gallery. A look's colours travel only as
/// SVG fill attributes, never as styles. The Plain card has none and is
/// painted by the stylesheet from the live tokens.
#[component]
fn Swatch(colors: Option<LookPreview>) -> impl IntoView {
    match colors {
        Some(colors) => view! {
            <svg class="swatch" viewBox="0 0 48 32" aria-hidden="true">
                <rect x="0" y="0" width="48" height="32" rx="6" fill=colors.bg />
                <rect x="7" y="9" width="34" height="14" rx="4" fill=colors.surface />
                <circle cx="37" cy="10" r="5" fill=colors.accent />
            </svg>
        }
        .into_any(),
        None => view! {
            <svg class="swatch" viewBox="0 0 48 32" aria-hidden="true">
                <rect class="swatch__default-bg" x="0" y="0" width="48" height="32" rx="6" />
                <rect class="swatch__default-surface" x="7" y="9" width="34" height="14" rx="4" />
                <circle class="swatch__default-accent" cx="37" cy="10" r="5" />
            </svg>
        }
        .into_any(),
    }
}
