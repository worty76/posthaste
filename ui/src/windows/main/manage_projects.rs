//! Manage projects: active projects with counts, inline rename (with merge
//! confirmation), archive, remove, and a collapsed Archived group with
//! unarchive.

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::Project;

use super::Main;
use crate::browser::{self, bind_text};
use crate::components::{
    Button, Dialog, EmptyState, Icon, IconButton, IconName, ProjectDot, Variant,
};
use crate::ipc::api;
use crate::logic::inbox::View;
use crate::logic::look::picker_value;
use crate::logic::project_picker::{rename_decision, RenameDecision};
use crate::state::I18n;

const MAX_PROJECT_CHARS: &str = "40";

/// Custom color picker control with a clear button when a color is set.
#[component]
fn ColorPickerControl(
    i18n: I18n,
    current: Signal<Option<String>>,
    #[prop(into)] on_pick: Callback<Option<String>>,
) -> impl IntoView {
    view! {
        <div
            class="proj-color-ctrl"
            on:click=move |event| event.stop_propagation()
        >
            <input
                type="color"
                class="proj-color-input"
                aria-label=i18n.text(key::INBOX_MANAGE_PROJECTS_COLOR_PICK)
                title=i18n.text(key::INBOX_MANAGE_PROJECTS_COLOR_PICK)
                prop:value=move || {
                    current.with(|c| picker_value(c.as_deref().unwrap_or("#64b5f6")))
                }
                on:input=move |event| on_pick.run(Some(event_target_value(&event)))
            />
            <Show when=move || current.with(Option::is_some)>
                <IconButton
                    icon=IconName::Close
                    label=i18n.label(key::INBOX_MANAGE_PROJECTS_COLOR_CLEAR)
                    on_click=move |()| on_pick.run(None)
                />
            </Show>
        </div>
    }
}

#[component]
pub(super) fn ManageProjectsView(main: Main) -> impl IntoView {
    let i18n = main.i18n;
    let all = RwSignal::new(Vec::<Project>::new());
    let loaded = RwSignal::new(false);
    let selected = RwSignal::new(Option::<String>::None);
    let archived_open = RwSignal::new(false);
    let renaming = RwSignal::new(Option::<String>::None);
    let draft = RwSignal::new(String::new());
    let rename_field = NodeRef::<html::Input>::new();
    let confirm_merge = RwSignal::new(Option::<(String, String, u32)>::None);
    let confirm_remove = RwSignal::new(Option::<(String, u32)>::None);
    let new_name = RwSignal::new(String::new());
    let new_color = RwSignal::new(Option::<String>::None);
    let creating = RwSignal::new(false);
    let container = NodeRef::<html::Div>::new();

    Effect::new(move |_| {
        if let Some(el) = container.get() {
            let _ = el.focus();
        }
    });

    Effect::new(move |_| {
        main.revision.with(|_| ());
        spawn_local(async move {
            match api::projects_list(true).await {
                Ok(list) => {
                    all.set(list);
                    loaded.set(true);
                }
                Err(error) => main.fail(error),
            }
        });
    });

    Effect::new(move |_| {
        if renaming.with(Option::is_some) {
            if let Some(input) = rename_field.get() {
                let _ = input.focus();
                input.select();
            }
        }
    });

    let active = Memo::new(move |_| {
        all.with(|list| {
            list.iter()
                .filter(|p| !p.archived)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let archived = Memo::new(move |_| {
        all.with(|list| {
            list.iter()
                .filter(|p| p.archived)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let visible_names = move || {
        let mut names: Vec<String> =
            active.with_untracked(|list| list.iter().map(|p| p.name.clone()).collect());
        if archived_open.get_untracked() {
            names
                .extend(archived.with_untracked(|list| {
                    list.iter().map(|p| p.name.clone()).collect::<Vec<_>>()
                }));
        }
        names
    };

    let start_rename = move |name: String| {
        draft.set(name.clone());
        renaming.set(Some(name));
    };

    let submit_rename = move || {
        let Some(from) = renaming.get_untracked() else {
            return;
        };
        let to = draft.get_untracked();
        let decision = all.with_untracked(|list| rename_decision(&from, &to, list));
        match decision {
            RenameDecision::NoOp => renaming.set(None),
            RenameDecision::Rename(target) => {
                renaming.set(None);
                spawn_local(async move {
                    match api::project_rename(&from, &target, false).await {
                        Ok(_) => selected.set(Some(target)),
                        Err(error) => main.fail(error),
                    }
                });
            }
            RenameDecision::ConfirmMerge { target, count } => {
                renaming.set(None);
                confirm_merge.set(Some((from, target, count)));
            }
        }
    };

    let set_archived = move |name: String, arch: bool| {
        spawn_local(async move {
            if let Err(error) = api::project_set_archived(&name, arch).await {
                main.fail(error);
            }
        });
    };

    let set_color = move |name: String, color: Option<String>| {
        spawn_local(async move {
            if let Err(error) = api::project_set_color(&name, color.as_deref()).await {
                main.fail(error);
            }
        });
    };

    let create_project = move || {
        let name = new_name.get_untracked();
        if name.trim().is_empty() || creating.get_untracked() {
            return;
        }
        let picked_color = new_color.get_untracked();
        creating.set(true);
        spawn_local(async move {
            match api::project_create(&name).await {
                Ok(created) => {
                    if let Some(color) = picked_color {
                        if let Err(error) =
                            api::project_set_color(&created.name, Some(&color)).await
                        {
                            main.fail(error);
                        }
                    }
                    new_name.set(String::new());
                    new_color.set(None);
                    selected.set(Some(created.name));
                    creating.set(false);
                }
                Err(error) => {
                    creating.set(false);
                    main.fail(error);
                }
            }
        });
    };

    let keys = window_event_listener(leptos::ev::keydown, move |event: KeyboardEvent| {
        if event.is_composing()
            || browser::dialog_is_open()
            || browser::is_typing(&event)
            || event.meta_key()
            || event.ctrl_key()
            || event.alt_key()
        {
            return;
        }
        let key = event.key();
        match key.as_str() {
            "ArrowDown" | "j" => {
                let names = visible_names();
                if names.is_empty() {
                    return;
                }
                event.prevent_default();
                let cur = selected.get_untracked();
                let next = match cur.and_then(|c| names.iter().position(|n| n == &c)) {
                    Some(i) => (i + 1).min(names.len() - 1),
                    None => 0,
                };
                selected.set(names.get(next).cloned());
            }
            "ArrowUp" | "k" => {
                let names = visible_names();
                if names.is_empty() {
                    return;
                }
                event.prevent_default();
                let cur = selected.get_untracked();
                let next = match cur.and_then(|c| names.iter().position(|n| n == &c)) {
                    Some(i) => i.saturating_sub(1),
                    None => names.len() - 1,
                };
                selected.set(names.get(next).cloned());
            }
            "Enter" if !browser::is_within(&event, "button, a, input") => {
                if let Some(name) = selected.get_untracked() {
                    event.prevent_default();
                    main.go(View::Project(name));
                }
            }
            "r" | "R" => {
                if let Some(name) = selected.get_untracked() {
                    event.prevent_default();
                    start_rename(name);
                }
            }
            "a" | "A" => {
                if let Some(name) = selected.get_untracked() {
                    let is_active =
                        active.with_untracked(|list| list.iter().any(|p| p.name == name));
                    if is_active {
                        event.prevent_default();
                        set_archived(name, true);
                    }
                }
            }
            "u" | "U" => {
                if let Some(name) = selected.get_untracked() {
                    let is_archived =
                        archived.with_untracked(|list| list.iter().any(|p| p.name == name));
                    if is_archived {
                        event.prevent_default();
                        set_archived(name, false);
                    }
                }
            }
            "Delete" | "Backspace" => {
                if let Some(name) = selected.get_untracked() {
                    let count = all.with_untracked(|list| {
                        list.iter().find(|p| p.name == name).map_or(0, |p| p.total)
                    });
                    event.prevent_default();
                    confirm_remove.set(Some((name, count)));
                }
            }
            _ => {}
        }
    });
    on_cleanup(move || keys.remove());

    let merge_open = RwSignal::new(false);
    Effect::new(move |_| merge_open.set(confirm_merge.with(Option::is_some)));
    let merge_title = Signal::derive(move || {
        confirm_merge.with(|pending| match pending {
            Some((_, target, count)) => i18n.translator().format(
                key::INBOX_PROJECT_PAGE_MERGE_CONFIRM,
                &[("count", &count.to_string()), ("name", target)],
            ),
            None => String::new(),
        })
    });

    let remove_open = RwSignal::new(false);
    Effect::new(move |_| remove_open.set(confirm_remove.with(Option::is_some)));
    let remove_title = Signal::derive(move || {
        confirm_remove.with(|pending| match pending {
            Some((_, 0)) => i18n
                .translator()
                .text(key::INBOX_MANAGE_PROJECTS_REMOVE_EMPTY_TITLE)
                .to_owned(),
            Some((_, count)) => i18n.translator().format(
                key::INBOX_MANAGE_PROJECTS_REMOVE_TITLE,
                &[("count", &count.to_string())],
            ),
            None => String::new(),
        })
    });

    let render_row = move |project: Project| {
        let name = StoredValue::new(project.name);
        let color = StoredValue::new(project.color);
        let total = project.total;
        let is_archived = project.archived;
        let is_selected = Memo::new(move |_| {
            selected.with(|sel| name.with_value(|n| sel.as_deref() == Some(n.as_str())))
        });
        let is_renaming = Memo::new(move |_| {
            renaming.with(|ren| name.with_value(|n| ren.as_deref() == Some(n.as_str())))
        });
        view! {
            <li
                class="manage-projects__row"
                data-selected=move || is_selected.get().to_string()
                aria-current=move || is_selected.get().then_some("true")
                on:click=move |_| selected.set(Some(name.get_value()))
            >
                {move || {
                    if is_renaming.get() {
                        view! {
                            <div class="manage-projects__edit">
                                <input
                                    node_ref=rename_field
                                    class="field"
                                    type="text"
                                    maxlength=MAX_PROJECT_CHARS
                                    aria-label=i18n.text(key::INBOX_PROJECT_PAGE_RENAME_LABEL)
                                    use:bind_text=Signal::derive(move || draft.get())
                                    on:input=move |event| draft.set(event_target_value(&event))
                                    on:keydown=move |event| {
                                        if event.is_composing() {
                                            return;
                                        }
                                        match event.key().as_str() {
                                            "Enter" => {
                                                event.prevent_default();
                                                event.stop_propagation();
                                                submit_rename();
                                            }
                                            "Escape" => {
                                                event.prevent_default();
                                                event.stop_propagation();
                                                renaming.set(None);
                                            }
                                            _ => {}
                                        }
                                    }
                                />
                                <Button variant=Variant::Primary on_click=move |()| submit_rename()>
                                    {i18n.text(key::COMMON_SAVE)}
                                </Button>
                                <Button variant=Variant::Quiet on_click=move |()| renaming.set(None)>
                                    {i18n.text(key::COMMON_CANCEL)}
                                </Button>
                            </div>
                        }
                        .into_any()
                    } else {
                        view! {
                            <button
                                type="button"
                                class="manage-projects__name"
                                on:click=move |event| {
                                    event.stop_propagation();
                                    selected.set(Some(name.get_value()));
                                }
                                on:dblclick=move |_| main.go(View::Project(name.get_value()))
                            >
                                {color.get_value().map(|c| view! { <ProjectDot color=c /> })}
                                <span class="manage-projects__title">{name.get_value()}</span>
                                <span class="manage-projects__count">{total}</span>
                            </button>
                            <ColorPickerControl
                                i18n=i18n
                                current=Signal::derive(move || color.get_value())
                                on_pick=move |next| set_color(name.get_value(), next)
                            />
                            <div class="manage-projects__actions">
                                <Button
                                    variant=Variant::Quiet
                                    on_click=move |()| start_rename(name.get_value())
                                >
                                    {i18n.text(key::INBOX_MANAGE_PROJECTS_RENAME)}
                                </Button>
                                {if is_archived {
                                    view! {
                                        <Button
                                            variant=Variant::Quiet
                                            on_click=move |()| set_archived(name.get_value(), false)
                                        >
                                            {i18n.text(key::INBOX_MANAGE_PROJECTS_UNARCHIVE)}
                                        </Button>
                                    }
                                    .into_any()
                                } else {
                                    view! {
                                        <Button
                                            variant=Variant::Quiet
                                            on_click=move |()| set_archived(name.get_value(), true)
                                        >
                                            {i18n.text(key::INBOX_MANAGE_PROJECTS_ARCHIVE)}
                                        </Button>
                                    }
                                    .into_any()
                                }}
                                <Button
                                    variant=Variant::Quiet
                                    on_click=move |()| confirm_remove.set(Some((name.get_value(), total)))
                                >
                                    {i18n.text(key::INBOX_MANAGE_PROJECTS_REMOVE)}
                                </Button>
                            </div>
                        }
                        .into_any()
                    }
                }}
            </li>
        }
    };

    let empty_title = Signal::derive(move || {
        i18n.translator()
            .text(key::INBOX_MANAGE_PROJECTS_EMPTY)
            .to_owned()
    });

    view! {
        <div node_ref=container class="manage-projects" tabindex="-1">
            <div class="manage-projects__new">
                <input
                    class="field"
                    type="text"
                    maxlength=MAX_PROJECT_CHARS
                    aria-label=i18n.text(key::INBOX_MANAGE_PROJECTS_NEW_LABEL)
                    placeholder=i18n.text(key::INBOX_MANAGE_PROJECTS_NEW_PLACEHOLDER)
                    use:bind_text=Signal::derive(move || new_name.get())
                    on:input=move |event| new_name.set(event_target_value(&event))
                    on:keydown=move |event| {
                        if event.is_composing() {
                            return;
                        }
                        if event.key() == "Enter" {
                            event.prevent_default();
                            event.stop_propagation();
                            create_project();
                        }
                    }
                />
                <ColorPickerControl
                    i18n=i18n
                    current=Signal::derive(move || new_color.get())
                    on_pick=move |next| new_color.set(next)
                />
                <Button
                    variant=Variant::Primary
                    busy=creating
                    disabled=Signal::derive(move || new_name.with(|n| n.trim().is_empty()))
                    on_click=move |()| create_project()
                >
                    {i18n.text(key::INBOX_MANAGE_PROJECTS_ADD)}
                </Button>
            </div>
            <Show when=move || loaded.get()>
                <Show
                    when=move || all.with(|list| !list.is_empty())
                    fallback=move || view! { <EmptyState title=empty_title>""</EmptyState> }
                >
                    <ul class="manage-projects__list">
                        <For
                            each=move || active.get()
                            key=|p| (p.name.clone(), p.color.clone(), p.total)
                            let:project
                        >
                            {render_row(project)}
                        </For>
                    </ul>

                    <Show when=move || archived.with(|list| !list.is_empty())>
                        <section class="manage-projects__archived">
                            <button
                                type="button"
                                class="proj-section__header"
                                aria-expanded=move || archived_open.get().to_string()
                                on:click=move |_| archived_open.update(|o| *o = !*o)
                            >
                                <span class="proj-section__chevron">
                                    <Icon name=IconName::Next size=12 />
                                </span>
                                <span class="proj-section__name">
                                    {move || {
                                        let count = archived.with(Vec::len).to_string();
                                        i18n.translator().format(
                                            key::INBOX_MANAGE_PROJECTS_ARCHIVED_GROUP,
                                            &[("count", &count)],
                                        )
                                    }}
                                </span>
                            </button>
                            <Show when=move || archived_open.get()>
                                <ul class="manage-projects__list">
                                    <For
                                        each=move || archived.get()
                                        key=|p| (p.name.clone(), p.color.clone(), p.total)
                                        let:project
                                    >
                                        {render_row(project)}
                                    </For>
                                </ul>
                            </Show>
                        </section>
                    </Show>
                </Show>
            </Show>

            <Dialog
                open=merge_open
                title=merge_title
                on_close=move |()| confirm_merge.set(None)
                actions=Box::new(move || {
                    view! {
                        <Button variant=Variant::Quiet on_click=move |()| confirm_merge.set(None)>
                            {i18n.text(key::COMMON_CANCEL)}
                        </Button>
                        <Button
                            variant=Variant::Primary
                            on_click=move |()| {
                                if let Some((from, target, _)) = confirm_merge.get_untracked() {
                                    confirm_merge.set(None);
                                    spawn_local(async move {
                                        match api::project_rename(&from, &target, true).await {
                                            Ok(_) => selected.set(Some(target)),
                                            Err(error) => main.fail(error),
                                        }
                                    });
                                }
                            }
                        >
                            {i18n.text(key::INBOX_PROJECT_PAGE_MERGE_ACTION)}
                        </Button>
                    }
                    .into_any()
                })
            >
                <p>{i18n.text(key::INBOX_MANAGE_PROJECTS_MERGE_TEXT)}</p>
            </Dialog>

            <Dialog
                open=remove_open
                title=remove_title
                on_close=move |()| confirm_remove.set(None)
                actions=Box::new(move || {
                    view! {
                        <Button variant=Variant::Quiet on_click=move |()| confirm_remove.set(None)>
                            {i18n.text(key::COMMON_CANCEL)}
                        </Button>
                        <Button
                            variant=Variant::Danger
                            on_click=move |()| {
                                if let Some((name, _)) = confirm_remove.get_untracked() {
                                    confirm_remove.set(None);
                                    spawn_local(async move {
                                        if let Err(error) = api::project_clear(&name).await {
                                            main.fail(error);
                                        }
                                    });
                                }
                            }
                        >
                            {i18n.text(key::INBOX_MANAGE_PROJECTS_REMOVE_ACTION)}
                        </Button>
                    }
                    .into_any()
                })
            >
                <p>{i18n.text(key::INBOX_MANAGE_PROJECTS_REMOVE_TEXT)}</p>
            </Dialog>
        </div>
    }
}
