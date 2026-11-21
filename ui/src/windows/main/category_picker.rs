//! The category picker, opened with `#`: pick a category to filter the
//! list by, from the ones in use on live notes.

use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::CategoryCount;

use super::Main;
use crate::browser::bind_text;
use crate::ipc::api;

#[component]
pub(super) fn CategoryPicker(main: Main) -> impl IntoView {
    let i18n = main.i18n;
    let categories = RwSignal::new(Vec::<CategoryCount>::new());
    let query = RwSignal::new(String::new());
    let highlighted = RwSignal::new(0usize);
    let field = NodeRef::<html::Input>::new();

    let load = move || {
        spawn_local(async move {
            // A refresh keeps the highlighted category when it still exists.
            let wanted = categories
                .with_untracked(|all| all.get(highlighted.get_untracked()).map(|c| c.name.clone()));
            match api::categories_counts().await {
                Ok(found) => {
                    let keep = wanted.and_then(|name| found.iter().position(|c| c.name == name));
                    highlighted.set(keep.unwrap_or(0).min(found.len().saturating_sub(1)));
                    categories.set(found);
                }
                Err(error) => main.fail(error),
            }
        });
    };
    load();
    // Notes changed while the picker is open: refresh without losing the place.
    Effect::new(move |_| {
        main.revision.with(|_| ());
        if main.picker_open.get_untracked() {
            load();
        }
    });
    Effect::new(move |_| {
        if let Some(field) = field.get() {
            let _ = field.focus();
        }
    });

    // The typed prefix narrows the list, case-insensitive, in the given order.
    let shown = Memo::new(move |_| {
        let prefix = query.with(|q| q.trim().to_lowercase());
        categories.with(|all| {
            all.iter()
                .filter(|c| prefix.is_empty() || c.name.to_lowercase().starts_with(&prefix))
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let highlighted_name = Memo::new(move |_| {
        shown.with(|shown| {
            let at = highlighted
                .get_untracked()
                .min(shown.len().saturating_sub(1));
            shown.get(at).map(|c| c.name.clone())
        })
    });
    let close = move || main.picker_open.set(false);
    let pick = move |name: String| main.set_category(name);
    let count_label = move |count: u32| {
        i18n.translator()
            .plural(key::INBOX_CAT_PICKER_NOTES, u64::from(count), &[])
    };
    let none_in_use = Memo::new(move |_| categories.with(Vec::is_empty));

    view! {
        <div
            class="cat-picker"
            role="dialog"
            aria-label=i18n.text(key::INBOX_CAT_PICKER_TITLE)
            on:click=move |event| event.stop_propagation()
        >
            <input
                node_ref=field
                class="field"
                type="text"
                aria-label=i18n.text(key::INBOX_CAT_PICKER_PLACEHOLDER)
                placeholder=i18n.text(key::INBOX_CAT_PICKER_PLACEHOLDER)
                spellcheck="false"
                use:bind_text=Signal::derive(move || query.get())
                on:input=move |event| {
                    query.set(event_target_value(&event));
                    highlighted.set(0);
                }
                on:keydown=move |event| {
                    if event.is_composing() {
                        return;
                    }
                    match event.key().as_str() {
                        "ArrowDown" => {
                            event.prevent_default();
                            highlighted.update(|i| {
                                let last = shown.with_untracked(|s| s.len().saturating_sub(1));
                                *i = (*i + 1).min(last);
                            });
                        }
                        "ArrowUp" => {
                            event.prevent_default();
                            highlighted.update(|i| *i = i.saturating_sub(1));
                        }
                        "Enter" => {
                            event.prevent_default();
                            event.stop_propagation();
                            if let Some(name) = highlighted_name.get_untracked() {
                                pick(name);
                            }
                        }
                        "Escape" => {
                            event.prevent_default();
                            event.stop_propagation();
                            close();
                        }
                        _ => {}
                    }
                }
            />
            <Show
                when=move || shown.with(|s| !s.is_empty())
                fallback=move || view! {
                    <p class="cat-picker__empty">
                        {move || {
                            let t = i18n.translator();
                            if none_in_use.get() {
                                t.text(key::INBOX_CAT_PICKER_EMPTY_HINT).to_owned()
                            } else {
                                t.text(key::INBOX_CAT_PICKER_NONE).to_owned()
                            }
                        }}
                    </p>
                }
            >
                <ul class="cat-picker__list">
                    <For each=move || shown.get() key=|c| c.name.clone() let:c>
                        {
                            let name = c.name.clone();
                            let count = c.count;
                            let hover = name.clone();
                            let named = name.clone();
                            let shown_name = name.clone();
                            view! {
                                <li>
                                    <button
                                        type="button"
                                        class="cat-picker__item"
                                        data-selected=move || highlighted_name
                                            .with(|h| h.as_deref() == Some(named.as_str()))
                                            .to_string()
                                        on:click=move |_| pick(name.clone())
                                        on:mouseenter=move |_| {
                                            if let Some(at) = shown
                                                .with_untracked(|s| s.iter().position(|c| c.name == hover))
                                            {
                                                highlighted.set(at);
                                            }
                                        }
                                    >
                                        <span class="cat-picker__name">{shown_name}</span>
                                        <span class="cat-picker__count">{count_label(count)}</span>
                                    </button>
                                </li>
                            }
                        }
                    </For>
                </ul>
            </Show>
        </div>
    }
}
