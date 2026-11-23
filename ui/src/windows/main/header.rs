//! Content header (title, project action menu, search field) and filter bar.

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::Os;

use super::date_filter;
use super::sidebar::view_title;
use super::Main;
use crate::browser::bind_text;
use crate::components::{Icon, IconButton, IconName, ProjectDot, Segmented};
use crate::ipc::api;
use crate::logic::inbox::{StatusFilter, View};
use crate::state::Message;

#[component]
pub(super) fn ContentHeader(main: Main) -> impl IntoView {
    let i18n = main.i18n;
    let os = main.os;
    let in_settings = move || main.view.get() == View::Settings;
    let title = move || {
        let t = i18n.translator();
        if main.searching() {
            t.text(key::INBOX_SEARCH_TITLE).to_owned()
        } else {
            match main.view.get() {
                View::Project(name) => name,
                view => t.text(view_title(&view)).to_owned(),
            }
        }
    };

    view! {
        <header class="content__header" data-tauri-drag-region=(os == Os::Macos).then_some("")>
            <div class="content__heading">
                {move || {
                    if main.searching() {
                        return None;
                    }
                    if let View::Project(name) = main.view.get() {
                        main.project_color(&name)
                            .map(|c| view! { <ProjectDot color=c /> })
                    } else {
                        None
                    }
                }}
                <h1 class="content__title">{title}</h1>
                {move || {
                    if main.searching() {
                        return None;
                    }
                    let View::Project(name) = main.view.get() else {
                        return None;
                    };
                    let archive_target = StoredValue::new(name);
                    Some(view! {
                        <div class="proj-page__menu-wrap">
                            <IconButton
                                icon=IconName::More
                                label=i18n.label(key::INBOX_PROJECT_PAGE_MENU)
                                expanded=Signal::derive(move || main.project_menu_open.get())
                                on_click=move |()| {
                                    main.project_menu_open.update(|open| *open = !*open);
                                }
                            />
                            <Show when=move || main.project_menu_open.get()>
                                <div class="proj-page__menu" role="menu">
                                    <button
                                        type="button"
                                        role="menuitem"
                                        class="proj-page__menu-item"
                                        on:click=move |_| {
                                            main.project_menu_open.set(false);
                                            main.project_renaming.set(true);
                                        }
                                    >
                                        {i18n.text(key::INBOX_PROJECT_PAGE_RENAME)}
                                    </button>
                                    <button
                                        type="button"
                                        role="menuitem"
                                        class="proj-page__menu-item"
                                        on:click=move |_| {
                                            let target = archive_target.get_value();
                                            main.project_menu_open.set(false);
                                            spawn_local(async move {
                                                match api::project_set_archived(&target, true).await {
                                                    Ok(()) => {
                                                        main.toaster.info(Message::Plain(
                                                            key::INBOX_PROJECT_PAGE_ARCHIVED_TOAST,
                                                        ));
                                                        main.go(View::Inbox);
                                                    }
                                                    Err(error) => main.fail(error),
                                                }
                                            });
                                        }
                                    >
                                        {i18n.text(key::INBOX_PROJECT_PAGE_ARCHIVE)}
                                    </button>
                                </div>
                            </Show>
                        </div>
                    })
                }}
            </div>
            <Show when=move || !in_settings()>
                <div class="search" role="search">
                    <span class="search__icon">
                        <Icon name=IconName::Search size=14 />
                    </span>
                    <input
                        node_ref=main.search
                        class="field"
                        type="text"
                        aria-label=i18n.text(key::INBOX_SEARCH_LABEL)
                        placeholder=i18n.text(key::INBOX_SEARCH_LABEL)
                        use:bind_text=Signal::derive(move || main.filters.with(|filters| filters.text.clone()))
                        on:input=move |event| main.set_search_text(event_target_value(&event))
                        spellcheck="false"
                    />
                    <Show when=move || main.searching()>
                        <IconButton
                            class="search__clear"
                            icon=IconName::Close
                            label=i18n.label(key::INBOX_SEARCH_CLEAR)
                            on_click=move |()| main.clear_search()
                        />
                    </Show>
                </div>
                <IconButton
                    icon=IconName::Filter
                    label=i18n.label(key::INBOX_FILTER_TOGGLE)
                    expanded=Signal::derive(move || main.searching() || main.filters_open.get())
                    on_click=move |()| main.filters_open.update(|open| *open = !*open)
                />
            </Show>
        </header>
    }
}

/// The controls that narrow a search: status and the day of capture.
#[component]
pub(super) fn FilterBar(main: Main) -> impl IntoView {
    let i18n = main.i18n;
    let found = move || {
        (main.searching() && main.loaded.get()).then(|| {
            let limit = main.limit.get();
            let (shown, more) = main
                .notes
                .with(|notes| (notes.len().min(limit) as u64, notes.len() > limit));
            let t = i18n.translator();
            match (shown, more) {
                (0, _) => t.text(key::INBOX_FILTER_NO_MATCHES).to_owned(),
                (shown, false) => t.plural(key::INBOX_FILTER_COUNT, shown, &[]),
                (shown, true) => t.plural(key::INBOX_FILTER_COUNT_MORE, shown, &[]),
            }
        })
    };
    view! {
        <div class="filters">
            <Segmented
                i18n=i18n
                label=key::INBOX_FILTER_STATUS
                value=Signal::derive(move || main.filters.with(|filters| filters.status))
                // The status narrows a search; on its own it searches nothing.
                disabled=Signal::derive(move || !main.searching())
                options=vec![
                    (StatusFilter::All, key::INBOX_FILTER_ALL),
                    (StatusFilter::Open, key::INBOX_FILTER_OPEN),
                    (StatusFilter::Completed, key::INBOX_FILTER_COMPLETED),
                    (StatusFilter::Archived, key::INBOX_FILTER_ARCHIVED),
                ]
                on_change=move |status| main.filters.update(|filters| filters.status = status)
            />
            <date_filter::DateFilter
                i18n=i18n
                locale=main.locale
                value=Signal::derive(move || main.filters.with(|filters| filters.date))
                on_change=move |day| main.set_date(day)
            />
            {move || found().map(|text| view! {
                <span class="filters__count" aria-live="polite">{text}</span>
            })}
        </div>
    }
}
