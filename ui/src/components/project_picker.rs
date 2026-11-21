//! Shared project picker used by the main window (`P` and `#`), Today (`P`),
//! and Note details (the project chip). Receives its list of projects as a
//! signal so it never touches IPC.

use leptos::html;
use leptos::prelude::*;
use posthaste_i18n::key;
use posthaste_model::Project;

use crate::browser::bind_text;
use crate::logic::project_picker::{rows, PickerMode, PickerRow};
use crate::state::I18n;

/// Stable key for a picker row in `<For>`.
fn row_key(row: &PickerRow) -> (u8, String) {
    match row {
        PickerRow::NoProject => (0, String::new()),
        PickerRow::Project { name, .. } => (1, name.clone()),
        PickerRow::Create(name) => (2, name.clone()),
        PickerRow::Unarchive(name) => (3, name.clone()),
    }
}

#[component]
pub fn ProjectPicker(
    i18n: I18n,
    mode: PickerMode,
    projects: Signal<Vec<Project>>,
    #[prop(into)] on_pick: Callback<PickerRow>,
    #[prop(into)] on_close: Callback<()>,
) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let highlighted = RwSignal::new(0usize);
    let field = NodeRef::<html::Input>::new();

    Effect::new(move |_| {
        if let Some(field) = field.get() {
            let _ = field.focus();
        }
    });

    let shown = Memo::new(move |_| {
        let typed = query.get();
        projects.with(|list| rows(list, &typed, mode))
    });
    let highlighted_row = Memo::new(move |_| {
        shown.with(|list| {
            let at = highlighted.get().min(list.len().saturating_sub(1));
            list.get(at).cloned()
        })
    });
    let none_active = Memo::new(move |_| projects.with(|list| list.iter().all(|p| p.archived)));

    let title_key = match mode {
        PickerMode::SetNote => key::COMMON_PROJECT_PICKER_TITLE_SET,
        PickerMode::Jump => key::COMMON_PROJECT_PICKER_TITLE_JUMP,
    };
    let placeholder_key = match mode {
        PickerMode::SetNote => key::COMMON_PROJECT_PICKER_PLACEHOLDER,
        PickerMode::Jump => key::COMMON_PROJECT_PICKER_PLACEHOLDER_JUMP,
    };

    view! {
        <div
            class="proj-picker"
            role="dialog"
            aria-label=i18n.text(title_key)
            on:click=move |event| event.stop_propagation()
        >
            <input
                node_ref=field
                class="field"
                type="text"
                aria-label=i18n.text(placeholder_key)
                placeholder=i18n.text(placeholder_key)
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
                            if let Some(row) = highlighted_row.get_untracked() {
                                on_pick.run(row);
                            }
                        }
                        "Escape" => {
                            event.prevent_default();
                            event.stop_propagation();
                            on_close.run(());
                        }
                        _ => {}
                    }
                }
            />
            <Show
                when=move || shown.with(|s| !s.is_empty())
                fallback=move || view! {
                    <p class="proj-picker__empty">
                        {move || {
                            let t = i18n.translator();
                            if none_active.get() {
                                t.text(key::COMMON_PROJECT_PICKER_EMPTY_HINT).to_owned()
                            } else {
                                t.text(key::COMMON_PROJECT_PICKER_NONE).to_owned()
                            }
                        }}
                    </p>
                }
            >
                <ul class="proj-picker__list">
                    <For each=move || shown.get() key=row_key let:row>
                        {
                            let row_click = row.clone();
                            let row_hover = row.clone();
                            let row_sel = row.clone();
                            let label = match &row {
                                PickerRow::NoProject => {
                                    i18n.translator().text(key::COMMON_PROJECT_PICKER_NO_PROJECT).to_owned()
                                }
                                PickerRow::Project { name, .. } => name.clone(),
                                PickerRow::Create(name) => i18n
                                    .translator()
                                    .format(key::COMMON_PROJECT_PICKER_CREATE, &[("name", name)]),
                                PickerRow::Unarchive(name) => i18n
                                    .translator()
                                    .format(key::COMMON_PROJECT_PICKER_UNARCHIVE, &[("name", name)]),
                            };
                            let count = match &row {
                                PickerRow::Project { count, .. } => Some(
                                    i18n.translator().plural(
                                        key::COMMON_PROJECT_PICKER_NOTES,
                                        u64::from(*count),
                                        &[],
                                    ),
                                ),
                                _ => None,
                            };
                            view! {
                                <li>
                                    <button
                                        type="button"
                                        class="proj-picker__item"
                                        data-selected=move || highlighted_row
                                            .with(|h| h.as_ref() == Some(&row_sel))
                                            .to_string()
                                        on:click=move |_| on_pick.run(row_click.clone())
                                        on:mouseenter=move |_| {
                                            if let Some(at) = shown
                                                .with_untracked(|s| s.iter().position(|r| r == &row_hover))
                                            {
                                                highlighted.set(at);
                                            }
                                        }
                                    >
                                        <span class="proj-picker__name">{label}</span>
                                        {count.map(|c| view! {
                                            <span class="proj-picker__count">{c}</span>
                                        })}
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
