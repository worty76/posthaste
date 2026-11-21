//! The list of notes in a section or a search, grouped by the day they
//! were captured.

use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Translator};
use posthaste_model::{Note, Status};

use super::Main;
use crate::browser::{self, bind_text};
use crate::components::{caps, Button, EmptyState, Icon, IconButton, IconName, Kbd, Variant};
use crate::logic::dates::Day;
use crate::logic::inbox::{empty_reason, is_long, Empty};
use crate::logic::sections;
use crate::time::{self, DateStyle, Width};
use crate::windows::note_badges::NoteBadges;

/// Longest category a note can have.
const MAX_CATEGORY_CHARS: &str = "40";

/// "Today", "Yesterday", or the date written out.
pub(super) fn day_label(day: Day, today: Day, locale: &str, t: Translator) -> String {
    match day.days_until(today) {
        0 => t.text(key::COMMON_DAY_TODAY).to_owned(),
        1 => t.text(key::COMMON_DAY_YESTERDAY).to_owned(),
        _ => time::format_day(
            day,
            locale,
            DateStyle {
                weekday: Some(Width::Long),
                month: Some(Width::Long),
                day: true,
                year: day.year != today.year,
            },
        ),
    }
}

#[component]
pub(super) fn NoteList(main: Main, capture_keys: Memo<Vec<String>>) -> impl IntoView {
    let i18n = main.i18n;
    let visible = Memo::new(move |_| {
        let limit = main.limit.get();
        main.notes
            .with(|notes| notes.iter().take(limit).cloned().collect::<Vec<_>>())
    });
    let grouped = Memo::new(move |_| {
        visible.with(|notes| {
            sections::group(notes)
                .into_iter()
                .map(|section| {
                    (
                        section.project.map(str::to_owned),
                        section.notes.into_iter().cloned().collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    let has_more = move || main.notes.with(|notes| notes.len() > main.limit.get());
    let keys = move || {
        grouped.with(|groups| {
            groups
                .iter()
                .map(|(project, _)| project.clone())
                .collect::<Vec<_>>()
        })
    };
    let notes_of = move |project: Option<&str>| {
        grouped.with(|groups| {
            groups
                .iter()
                .find(|(name, _)| name.as_deref() == project)
                .map(|(_, notes)| notes.clone())
                .unwrap_or_default()
        })
    };

    view! {
        <Show when=move || main.loaded.get()>
            <Show
                when=move || visible.with(|notes| !notes.is_empty())
                fallback=move || view! { <Nothing main=main capture_keys=capture_keys /> }
            >
                <Show
                    when=move || main.searching()
                    fallback=move || view! {
                        <For each=keys key=|project| project.clone() let:project>
                            {match project {
                                Some(name) => {
                                    let project = StoredValue::new(name);
                                    let expanded = move || {
                                        project.with_value(|name| !main.is_section_collapsed(name))
                                    };
                                    view! {
                                        <section class="proj-section" aria-label=project.get_value()>
                                            <button
                                                type="button"
                                                class="proj-section__header"
                                                aria-expanded=move || expanded().to_string()
                                                on:click=move |_| {
                                                    project.with_value(|name| main.toggle_section(name));
                                                }
                                                on:keydown=move |event| {
                                                    if event.is_composing() {
                                                        return;
                                                    }
                                                    match event.key().as_str() {
                                                        "ArrowLeft" => {
                                                            event.prevent_default();
                                                            event.stop_propagation();
                                                            project.with_value(|name| {
                                                                main.set_section_collapsed(name, true);
                                                            });
                                                        }
                                                        "ArrowRight" => {
                                                            event.prevent_default();
                                                            event.stop_propagation();
                                                            project.with_value(|name| {
                                                                main.set_section_collapsed(name, false);
                                                            });
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            >
                                                <span class="proj-section__chevron">
                                                    <Icon name=IconName::Next size=12 />
                                                </span>
                                                {move || project.with_value(|name| main.project_color(name)).map(|c| view! {
                                                    <span class="proj-dot" data-color=c aria-hidden="true"></span>
                                                })}
                                                <span class="proj-section__name">{project.get_value()}</span>
                                                <span class="proj-section__count">
                                                    {move || project.with_value(|name| notes_of(Some(name)).len())}
                                                </span>
                                            </button>
                                            <Show when=expanded>
                                                <ul class="notes">
                                                    <For
                                                        each=move || project.with_value(|name| notes_of(Some(name)))
                                                        key=row_key
                                                        let:note
                                                    >
                                                        <NoteCard main=main note=note />
                                                    </For>
                                                </ul>
                                            </Show>
                                        </section>
                                    }
                                    .into_any()
                                }
                                None => view! {
                                    <section class="proj-section proj-section--plain">
                                        <ul class="notes">
                                            <For each=move || notes_of(None) key=row_key let:note>
                                                <NoteCard main=main note=note />
                                            </For>
                                        </ul>
                                    </section>
                                }
                                .into_any(),
                            }}
                        </For>
                    }
                >
                    <ul class="notes">
                        <For each=move || visible.get() key=row_key let:note>
                            <NoteCard main=main note=note />
                        </For>
                    </ul>
                </Show>
                <Show when=has_more>
                    <Button class="more" on_click=move |()| main.limit.update(|limit| *limit += super::PAGE)>
                        {i18n.text(key::INBOX_MORE)}
                    </Button>
                </Show>
            </Show>
        </Show>
    }
}

/// Everything about a note that changes how its card is drawn. A card is
/// redrawn only when this changes, so focus and scroll survive a refresh.
pub(super) fn row_key(note: &Note) -> (String, i64, Status, Option<i64>, bool, u32) {
    (
        note.id.clone(),
        note.updated_at,
        note.status,
        note.later_at,
        note.has_details,
        note.attachment_count,
    )
}

/// Why the list is empty, and what to do about it.
#[component]
fn Nothing(main: Main, capture_keys: Memo<Vec<String>>) -> impl IntoView {
    let i18n = main.i18n;
    let reason = Memo::new(move |_| {
        main.view
            .with(|view| main.filters.with(|filters| empty_reason(view, filters)))
    });
    let title = Signal::derive(move || {
        let key = match reason.get() {
            Empty::NoMatch { .. } | Empty::NothingThatDay => key::INBOX_EMPTY_SEARCH_TITLE,
            Empty::Project => key::INBOX_PROJECT_PAGE_EMPTY_TITLE,
            Empty::Today => key::INBOX_EMPTY_TODAY_TITLE,
            Empty::Later => key::INBOX_EMPTY_LATER_TITLE,
            Empty::Archive => key::INBOX_EMPTY_ARCHIVE_TITLE,
            Empty::Inbox => key::INBOX_EMPTY_INBOX_TITLE,
        };
        i18n.translator().text(key).to_owned()
    });
    view! {
        <EmptyState title=title>
            {move || {
                let t = i18n.translator();
                match reason.get() {
                    Empty::NoMatch { filtered } => {
                        let key = if filtered {
                            key::INBOX_EMPTY_SEARCH_TEXT_FILTERED
                        } else {
                            key::INBOX_EMPTY_SEARCH_TEXT
                        };
                        let text = main.filters.with(|filters| filters.text.trim().to_owned());
                        t.format(key, &[("text", &text)]).into_any()
                    }
                    Empty::Project => ().into_any(),
                    Empty::NothingThatDay => t.text(key::INBOX_EMPTY_SEARCH_DATE).into_any(),
                    Empty::Today => t.text(key::INBOX_EMPTY_TODAY_TEXT).into_any(),
                    Empty::Archive => t.text(key::INBOX_EMPTY_ARCHIVE_TEXT).into_any(),
                    Empty::Later => {
                        let (before, after) = t.around(key::INBOX_EMPTY_LATER_TEXT, "key");
                        view! { {before} <Kbd keys=caps(&["L"]) /> {after} }.into_any()
                    }
                    Empty::Inbox if capture_keys.with(Vec::is_empty) => {
                        t.text(key::INBOX_EMPTY_INBOX_TEXT_PLAIN).into_any()
                    }
                    Empty::Inbox => {
                        let (before, after) = t.around(key::INBOX_EMPTY_INBOX_TEXT_SHORTCUT, "keys");
                        view! { {before} <Kbd keys=capture_keys /> {after} }.into_any()
                    }
                }
            }}
        </EmptyState>
    }
}

#[component]
pub(super) fn NoteCard(main: Main, note: Note) -> impl IntoView {
    let i18n = main.i18n;
    let item = NodeRef::<html::Li>::new();
    let expanded = RwSignal::new(false);
    let long = is_long(&note.content);
    let done = note.status == Status::Completed;
    let status = note.status;
    let note = StoredValue::new(note);
    let id = note.with_value(|note| note.id.clone());
    let is_this = move |selected: &Option<String>| selected.as_deref() == Some(id.as_str());
    let selected = Memo::new({
        let is_this = is_this.clone();
        move |_| main.selected.with(&is_this)
    });
    let editing = Memo::new(move |_| main.editing.with(&is_this));

    // Moving with the keyboard must never leave the selection off screen.
    Effect::new(move |_| {
        if let (true, Some(item)) = (selected.get(), item.get()) {
            let options = web_sys::ScrollIntoViewOptions::new();
            options.set_block(web_sys::ScrollLogicalPosition::Nearest);
            item.scroll_into_view_with_scroll_into_view_options(&options);
        }
    });

    let check_label = move || {
        i18n.translator().text(if done {
            key::INBOX_NOTE_MARK_NOT_COMPLETED
        } else {
            key::INBOX_NOTE_MARK_COMPLETED
        })
    };

    view! {
        <li
            node_ref=item
            class="note"
            data-selected=move || selected.get().to_string()
            data-status=status.as_str()
            aria-current=move || selected.get().then_some("true")
            on:click=move |_| note.with_value(|note| main.selected.set(Some(note.id.clone())))
        >
            <button
                type="button"
                role="checkbox"
                class="check"
                aria-checked=done.to_string()
                aria-label=check_label
                title=check_label
                on:click=move |event| {
                    event.stop_propagation();
                    note.with_value(|note| main.toggle_complete(note));
                }
            >
                <Icon name=IconName::Check size=12 />
            </button>
            {move || {
                if editing.get() {
                    view! { <NoteEditor main=main note=note.get_value() /> }.into_any()
                } else {
                    view! { <NoteBody main=main note=note expanded=expanded long=long /> }.into_any()
                }
            }}
        </li>
    }
}

/// A note as it is read: its text, what is known about it, and the actions.
#[component]
fn NoteBody(
    main: Main,
    note: StoredValue<Note>,
    expanded: RwSignal<bool>,
    long: bool,
) -> impl IntoView {
    let i18n = main.i18n;
    let (created_at, status, parked) =
        note.with_value(|note| (note.created_at, note.status, note.later_at.is_some()));
    let in_archive = status != Status::Inbox;
    let tag = move |key| view! { <span>{i18n.text(key)}</span> };
    let action = move |icon, label, run: fn(Main, &Note)| {
        view! {
            <IconButton
                icon=icon
                label=i18n.label(label)
                on_click=move |()| note.with_value(|note| run(main, note))
            />
        }
    };

    view! {
        <div class="note__body">
            <p
                class="note__content selectable"
                data-clamped=move || (long && !expanded.get()).to_string()
                on:dblclick=move |_| note.with_value(|note| main.start_edit(&note.id))
            >
                {note.with_value(|note| note.content.clone())}
            </p>
            {long.then(|| view! {
                <button
                    type="button"
                    class="note__more"
                    aria-expanded=move || expanded.get().to_string()
                    on:click=move |event| {
                        event.stop_propagation();
                        expanded.update(|open| *open = !*open);
                    }
                >
                    {move || i18n.translator().text(if expanded.get() {
                        key::INBOX_NOTE_SHOW_LESS
                    } else {
                        key::INBOX_NOTE_SHOW_MORE
                    })}
                </button>
            })}
            <div class="note__meta">
                <time datetime=time::iso(created_at)>
                    {move || time::format_time(created_at, &main.locale.get())}
                </time>
                {note.with_value(|note| note.category.clone()).map(|category| {
                    let pick = category.clone();
                    let named = category.clone();
                    view! {
                        <button
                            type="button"
                            class="note__category"
                            title=move || {
                                i18n.translator()
                                    .format(key::INBOX_NOTE_CATEGORY_TIP, &[("name", &named)])
                            }
                            // Its own control: it opens the project page, not the row.
                            on:click=move |event| {
                                event.stop_propagation();
                                main.go(crate::logic::inbox::View::Project(pick.clone()));
                            }
                        >
                            {category}
                        </button>
                    }
                })}
                {note.with_value(|note| note.source.clone()).map(|source| view! {
                    <span>
                        {move || i18n.translator().format(key::INBOX_NOTE_FROM, &[("source", &source)])}
                    </span>
                })}
                {parked.then(|| tag(key::INBOX_NOTE_LATER))}
                {(status == Status::Archived).then(|| tag(key::INBOX_NOTE_ARCHIVED))}
                {(status == Status::Completed).then(|| tag(key::INBOX_NOTE_COMPLETED))}
                <NoteBadges note=note.get_value() i18n=i18n />
            </div>
        </div>
        <div class="note__actions" on:click=move |event| event.stop_propagation()>
            {action(IconName::Edit, key::INBOX_NOTE_EDIT, |main, note| main.start_edit(&note.id))}
            {action(IconName::Open, key::INBOX_NOTE_OPEN_DETAILS, |main, note| main.open_details(note))}
            {action(IconName::Copy, key::INBOX_NOTE_COPY, |main, note| main.copy(note))}
            {if in_archive {
                action(IconName::Restore, key::INBOX_NOTE_RESTORE, |main, note| main.restore(note)).into_any()
            } else {
                let (icon, label) = if parked {
                    (IconName::Inbox, key::INBOX_NOTE_TO_INBOX)
                } else {
                    (IconName::Later, key::INBOX_NOTE_TO_LATER)
                };
                view! {
                    {action(icon, label, |main, note| main.toggle_later(note))}
                    {action(IconName::Archive, key::INBOX_NOTE_ARCHIVE, |main, note| main.archive(note))}
                }
                .into_any()
            }}
            <IconButton
                icon=IconName::Trash
                label=i18n.label(key::INBOX_NOTE_DELETE)
                danger=true
                on_click=move |()| note.with_value(|note| main.remove(note))
            />
        </div>
    }
}

/// A note being edited in place: Enter saves, Esc cancels.
#[component]
fn NoteEditor(main: Main, note: Note) -> impl IntoView {
    let i18n = main.i18n;
    let content = RwSignal::new(note.content.clone());
    let category = RwSignal::new(note.category.clone().unwrap_or_default());
    let saving = RwSignal::new(false);
    let field = NodeRef::<html::Textarea>::new();
    let id = StoredValue::new(note.id);

    Effect::new(move |_| {
        if let Some(field) = field.get() {
            browser::focus_end(&field);
        }
    });

    let empty = move || content.with(|content| content.trim().is_empty());
    let save = move || {
        if empty() || saving.get_untracked() {
            return;
        }
        saving.set(true);
        spawn_local(async move {
            let content = content.get_untracked();
            let category = category.get_untracked();
            let category = category.trim();
            let stored = main
                .save_edit(
                    &id.get_value(),
                    &content,
                    (!category.is_empty()).then_some(category),
                )
                .await;
            // On success the editor is gone, and its signals with it.
            if !stored {
                saving.set(false);
            }
        });
    };
    // Tall enough for the text, within reason.
    let rows = move || content.with(|content| content.split('\n').count().clamp(2, 12));

    view! {
        <div
            class="note__edit"
            on:click=move |event| event.stop_propagation()
            // Stopped here, so the window's own key handler never sees these.
            on:keydown=move |event| {
                if event.is_composing() {
                    return;
                }
                if event.key() == "Escape" {
                    event.prevent_default();
                    event.stop_propagation();
                    main.editing.set(None);
                } else if event.key() == "Enter" && !event.shift_key() {
                    event.prevent_default();
                    save();
                }
            }
        >
            <textarea
                node_ref=field
                class="textarea"
                aria-label=i18n.text(key::INBOX_EDIT_LABEL)
                rows=rows
                use:bind_text=Signal::derive(move || content.get())
                on:input=move |event| content.set(event_target_value(&event))
            ></textarea>
            <div class="note__edit-row">
                <input
                    class="field"
                    aria-label=i18n.text(key::INBOX_EDIT_CATEGORY_LABEL)
                    placeholder=i18n.text(key::INBOX_EDIT_CATEGORY_PLACEHOLDER)
                    maxlength=MAX_CATEGORY_CHARS
                    use:bind_text=Signal::derive(move || category.get())
                    on:input=move |event| category.set(event_target_value(&event))
                />
                <span class="note__edit-hint">{i18n.text(key::INBOX_EDIT_HINT)}</span>
                <Button variant=Variant::Quiet on_click=move |()| main.editing.set(None)>
                    {i18n.text(key::COMMON_CANCEL)}
                </Button>
                <Button
                    variant=Variant::Primary
                    busy=saving
                    disabled=Signal::derive(empty)
                    on_click=move |()| save()
                >
                    {i18n.text(key::COMMON_SAVE)}
                </Button>
            </div>
        </div>
    }
}
