//! A single project's page: quick-add at the top, its notes across every
//! status grouped by day (open before finished within each day), and inline
//! rename triggered from the header's project menu (`M`).

use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::Note;

use super::notes::{day_label, row_key, NoteCard};
use super::Main;
use crate::browser::bind_text;
use crate::components::{Button, Icon, IconName, Variant};
use crate::ipc::api;
use crate::logic::inbox::{group_by_day, DayLabel, View};
use crate::time;

const MAX_PROJECT_CHARS: &str = "40";

fn format_day_label(label: DayLabel, locale: &str, t: posthaste_i18n::Translator) -> String {
    match label {
        DayLabel::Today => t.text(key::COMMON_DAY_TODAY).to_owned(),
        DayLabel::Yesterday => t.text(key::COMMON_DAY_YESTERDAY).to_owned(),
        DayLabel::Earlier(ts) => day_label(time::day_of(ts), time::today(), locale, t),
    }
}

fn day_key(label: &DayLabel) -> (u8, i64) {
    match *label {
        DayLabel::Today => (0, 0),
        DayLabel::Yesterday => (1, 0),
        DayLabel::Earlier(ts) => (2, ts),
    }
}

#[component]
pub(super) fn ProjectPage(main: Main, name: String) -> impl IntoView {
    let i18n = main.i18n;
    let project = StoredValue::new(name);
    let draft_name = RwSignal::new(project.get_value());
    let rename_field = NodeRef::<html::Input>::new();
    let add_text = RwSignal::new(String::new());
    let adding = RwSignal::new(false);

    Effect::new(move |_| {
        if main.project_renaming.get() {
            draft_name.set(project.get_value());
            if let Some(input) = rename_field.get() {
                let _ = input.focus();
                input.select();
            }
        }
    });

    let groups = Memo::new(move |_| {
        let limit = main.limit.get();
        let today = time::bounds(time::today());
        main.notes.with(|notes| {
            let slice = &notes[..notes.len().min(limit)];
            group_by_day(slice, today)
                .into_iter()
                .map(|(label, list)| (label, list.into_iter().cloned().collect::<Vec<Note>>()))
                .collect::<Vec<_>>()
        })
    });
    let has_more = move || main.notes.with(|notes| notes.len() > main.limit.get());
    let labels = move || {
        groups.with(|groups| {
            groups
                .iter()
                .map(|(label, _)| *label)
                .collect::<Vec<DayLabel>>()
        })
    };
    let notes_of = move |label: DayLabel| {
        groups.with(|groups| {
            groups
                .iter()
                .find(|(day, _)| *day == label)
                .map(|(_, notes)| notes.clone())
                .unwrap_or_default()
        })
    };

    let submit_rename = move || {
        let from = project.get_value();
        let to = draft_name.get_untracked();
        let to = to.trim().trim_start_matches('#').trim().to_owned();
        if to.is_empty() || to == from {
            main.project_renaming.set(false);
            return;
        }
        spawn_local(async move {
            match api::project_rename(&from, &to, false).await {
                Ok(_) => {
                    main.project_renaming.set(false);
                    main.go(View::Project(to));
                }
                Err(error) => main.fail(error),
            }
        });
    };

    let submit_add = move || {
        let text = add_text.get_untracked();
        if text.trim().is_empty() || adding.get_untracked() {
            return;
        }
        adding.set(true);
        let category = project.get_value();
        spawn_local(async move {
            match api::capture_save(&text, Some(&category), false).await {
                Ok(()) => {
                    add_text.set(String::new());
                    adding.set(false);
                }
                Err(error) => {
                    adding.set(false);
                    main.fail(error);
                }
            }
        });
    };

    let empty_title = Signal::derive(move || {
        i18n.translator()
            .text(key::INBOX_PROJECT_PAGE_EMPTY_TITLE)
            .to_owned()
    });
    let add_label = Signal::derive(move || {
        project.with_value(|name| {
            i18n.translator()
                .format(key::INBOX_PROJECT_PAGE_ADD_PLACEHOLDER, &[("name", name)])
        })
    });

    view! {
        <div class="proj-page">
            <Show when=move || main.project_renaming.get()>
                <div class="proj-page__rename">
                    <input
                        node_ref=rename_field
                        class="field"
                        type="text"
                        maxlength=MAX_PROJECT_CHARS
                        aria-label=i18n.text(key::INBOX_PROJECT_PAGE_RENAME_LABEL)
                        use:bind_text=Signal::derive(move || draft_name.get())
                        on:input=move |event| draft_name.set(event_target_value(&event))
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
                                    main.project_renaming.set(false);
                                }
                                _ => {}
                            }
                        }
                    />
                    <Button variant=Variant::Primary on_click=move |()| submit_rename()>
                        {i18n.text(key::COMMON_SAVE)}
                    </Button>
                    <Button variant=Variant::Quiet on_click=move |()| main.project_renaming.set(false)>
                        {i18n.text(key::COMMON_CANCEL)}
                    </Button>
                </div>
            </Show>

            <div class="proj-page__add">
                <span class="proj-page__add-icon" aria-hidden="true">
                    <Icon name=IconName::Plus size=14 />
                </span>
                <input
                    class="field"
                    type="text"
                    aria-label=add_label
                    placeholder=add_label
                    use:bind_text=Signal::derive(move || add_text.get())
                    on:input=move |event| add_text.set(event_target_value(&event))
                    on:keydown=move |event| {
                        if event.is_composing() {
                            return;
                        }
                        if event.key() == "Enter" {
                            event.prevent_default();
                            event.stop_propagation();
                            submit_add();
                        }
                    }
                />
            </div>

            <Show when=move || main.loaded.get()>
                <Show
                    when=move || groups.with(|g| !g.is_empty())
                    fallback=move || view! {
                        <div class="empty proj-page__empty">
                            <div class="empty__title">{empty_title}</div>
                        </div>
                    }
                >
                    <For each=labels key=day_key let:label>
                        {
                            let heading = move || {
                                format_day_label(label, &main.locale.get(), i18n.translator())
                            };
                            view! {
                                <section class="day" aria-label=heading>
                                    <h2 class="day__label">
                                        {heading}
                                        <span class="day__count">{move || notes_of(label).len()}</span>
                                    </h2>
                                    <ul class="notes">
                                        <For each=move || notes_of(label) key=row_key let:note>
                                            <NoteCard main=main note=note />
                                        </For>
                                    </ul>
                                </section>
                            }
                        }
                    </For>
                    <Show when=has_more>
                        <Button class="more" on_click=move |()| main.limit.update(|limit| *limit += super::PAGE)>
                            {i18n.text(key::INBOX_MORE)}
                        </Button>
                    </Show>
                </Show>
            </Show>
        </div>
    }
}
