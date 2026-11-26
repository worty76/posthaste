//! Command Palette (`⌘K` / `Ctrl+K`): jump to views, projects, looks, or actions
//! in a few keystrokes.

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;
use posthaste_i18n::{key, Key};

use crate::browser::bind_text;
use crate::components::{Icon, IconName, Kbd, ProjectDot};
use crate::logic::palette::{filter_items, move_index, PaletteGroup, PaletteItem};
use crate::state::I18n;

fn group_label(group: PaletteGroup) -> Key {
    match group {
        PaletteGroup::Navigate => key::INBOX_PALETTE_GROUP_JUMP,
        PaletteGroup::Project => key::INBOX_NAV_PROJECTS,
        PaletteGroup::Action => key::INBOX_PALETTE_GROUP_ACTIONS,
        PaletteGroup::Look => key::LOOKS_GALLERY_HEADING,
    }
}

#[component]
pub fn CommandPalette(
    open: RwSignal<bool>,
    i18n: I18n,
    #[prop(into)] items: Signal<Vec<PaletteItem>>,
    #[prop(into)] on_select: Callback<String>,
) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let active = RwSignal::new(0usize);
    let input_ref = NodeRef::<html::Input>::new();

    let filtered = Memo::new(move |_| {
        let q = query.get();
        items.with(|all| filter_items(all, &q))
    });

    Effect::new(move |_| {
        if open.get() {
            query.set(String::new());
            active.set(0);
            request_animation_frame(move || {
                if let Some(el) = input_ref.get_untracked() {
                    let _ = el.focus();
                }
            });
        }
    });

    Effect::new(move |_| {
        let len = filtered.with(Vec::len);
        if active.get_untracked() >= len {
            active.set(0);
        }
    });

    let close = move || open.set(false);

    let run_selected = move || {
        let chosen = filtered.with_untracked(|list| {
            let idx = active.get_untracked();
            list.get(idx).map(|item| item.id.clone())
        });
        if let Some(id) = chosen {
            open.set(false);
            on_select.run(id);
        }
    };

    let on_keydown = move |event: KeyboardEvent| {
        if event.is_composing() {
            return;
        }
        match event.key().as_str() {
            "Escape" => {
                event.prevent_default();
                event.stop_propagation();
                close();
            }
            "ArrowDown" => {
                event.prevent_default();
                let len = filtered.with_untracked(Vec::len);
                active.update(|i| *i = move_index(len, *i, 1));
            }
            "ArrowUp" => {
                event.prevent_default();
                let len = filtered.with_untracked(Vec::len);
                active.update(|i| *i = move_index(len, *i, -1));
            }
            "Enter" => {
                event.prevent_default();
                run_selected();
            }
            _ => {}
        }
    };

    view! {
        <Show when=move || open.get()>
            <div
                class="palette__backdrop"
                on:mousedown=move |event| {
                    if event.target() == event.current_target() {
                        close();
                    }
                }
            >
                <div
                    class="palette"
                    role="dialog"
                    aria-modal="true"
                    aria-label=i18n.text(key::INBOX_PALETTE_TITLE)
                    on:keydown=on_keydown
                >
                    <div class="palette__search">
                        <span class="palette__search-icon" aria-hidden="true">
                            <Icon name=IconName::Search size=15 />
                        </span>
                        <input
                            node_ref=input_ref
                            type="text"
                            class="palette__input"
                            aria-label=i18n.text(key::INBOX_PALETTE_PLACEHOLDER)
                            placeholder=i18n.text(key::INBOX_PALETTE_PLACEHOLDER)
                            use:bind_text=Signal::derive(move || query.get())
                            on:input=move |event| {
                                query.set(event_target_value(&event));
                                active.set(0);
                            }
                        />
                        <Kbd keys=vec!["Esc".to_owned()] />
                    </div>

                    <div class="palette__list" role="listbox">
                        {move || {
                            let list = filtered.get();
                            if list.is_empty() {
                                return view! {
                                    <p class="palette__empty">
                                        {i18n.text(key::INBOX_PALETTE_EMPTY)}
                                    </p>
                                }
                                .into_any();
                            }
                            let mut last_group: Option<PaletteGroup> = None;
                            list.into_iter()
                                .enumerate()
                                .map(|(idx, item)| {
                                    let show_heading = last_group != Some(item.group);
                                    last_group = Some(item.group);
                                    let id = item.id.clone();
                                    let is_active = move || active.get() == idx;
                                    view! {
                                        {show_heading.then(|| view! {
                                            <div class="palette__group" aria-hidden="true">
                                                {i18n.text(group_label(item.group))}
                                            </div>
                                        })}
                                        <button
                                            type="button"
                                            role="option"
                                            class="palette__item"
                                            aria-selected=move || is_active().to_string()
                                            data-active=move || is_active().to_string()
                                            on:mouseenter=move |_| active.set(idx)
                                            on:click=move |_| {
                                                open.set(false);
                                                on_select.run(id.clone());
                                            }
                                        >
                                            {item.color.map(|c| view! { <ProjectDot color=c /> })}
                                            <span class="palette__title">{item.title}</span>
                                            {item.subtitle.map(|sub| view! {
                                                <span class="palette__subtitle">{sub}</span>
                                            })}
                                            {(!item.shortcut.is_empty()).then(|| view! {
                                                <Kbd keys=item.shortcut />
                                            })}
                                        </button>
                                    }
                                })
                                .collect_view()
                                .into_any()
                        }}
                    </div>
                </div>
            </div>
        </Show>
    }
}
