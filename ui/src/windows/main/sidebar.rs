//! The main window sidebar: section list, active project links, new note
//! button, and keyboard arrow navigation.

use leptos::prelude::*;
use posthaste_i18n::{key, Key};
use wasm_bindgen::JsCast;

use super::Main;
use crate::components::{Icon, IconButton, IconName, Kbd, ProjectDot};
use crate::logic::inbox::View;
use crate::logic::shortcut::mod_cap;

pub(super) fn view_title(view: &View) -> Key {
    match view {
        View::Inbox => key::INBOX_NAV_INBOX,
        View::Today => key::INBOX_NAV_TODAY,
        View::Later => key::INBOX_NAV_LATER,
        View::Archive => key::INBOX_NAV_ARCHIVE,
        View::Trash => key::INBOX_NAV_TRASH,
        View::Settings => key::INBOX_NAV_SETTINGS,
        View::ManageProjects => key::INBOX_NAV_MANAGE_PROJECTS,
        View::Project(_) => key::INBOX_NAV_PROJECTS,
    }
}

pub(super) fn view_icon(view: &View) -> IconName {
    match view {
        View::Inbox => IconName::Inbox,
        View::Today => IconName::Sun,
        View::Later => IconName::Later,
        View::Archive => IconName::Archive,
        View::Trash => IconName::Trash,
        View::Settings => IconName::Settings,
        View::Project(_) | View::ManageProjects => IconName::Lines,
    }
}

#[component]
pub(super) fn Sidebar(
    main: Main,
    sidebar_collapsed: Memo<bool>,
    capture_keys: Memo<Vec<String>>,
) -> impl IntoView {
    let i18n = main.i18n;
    let os = main.os;
    let count = move |view: &View| {
        main.counts.with(|counts| {
            let counts = counts.as_ref()?;
            let count = match view {
                View::Inbox => counts.inbox,
                View::Today => counts.today,
                View::Later => counts.later,
                View::Trash => counts.trash,
                View::Archive | View::Settings | View::Project(_) | View::ManageProjects => {
                    return None
                }
            };
            (count > 0).then_some(count)
        })
    };
    let sidebar_shortcut = format!("{}+B", mod_cap(os));
    let sidebar_toggle_label = Signal::derive(move || {
        let key = if sidebar_collapsed.get() {
            key::INBOX_SIDEBAR_EXPAND
        } else {
            key::INBOX_SIDEBAR_COLLAPSE
        };
        i18n.translator()
            .format(key, &[("keys", &sidebar_shortcut)])
    });
    let nav_item = move |view: View| {
        let view_cur = view.clone();
        let view_click = view.clone();
        let view_count = view.clone();
        let title_key = view_title(&view);
        let icon = view_icon(&view);
        // A search is its own place: no section is the current one then.
        let current = move || {
            let here = main.view.with(|v| *v == view_cur)
                && (view_cur == View::Settings || !main.searching());
            here.then_some("page")
        };
        let label = i18n.label(title_key);
        view! {
            <button
                type="button"
                class="nav__item"
                data-collapsed=move || sidebar_collapsed.get().then_some("")
                aria-current=current
                aria-label=move || sidebar_collapsed.get().then(|| label.get())
                title=move || sidebar_collapsed.get().then(|| label.get())
                on:click=move |_| main.go(view_click.clone())
            >
                <Icon name=icon />
                <span class="nav__label">{i18n.text(title_key)}</span>
                {move || count(&view_count).map(|count| view! { <span class="nav__count">{count}</span> })}
            </button>
        }
    };

    view! {
        <aside
            class="sidebar"
            data-collapsed=move || sidebar_collapsed.get().then_some("")
            on:keydown=move |event| {
                let key = event.key();
                if key != "ArrowDown" && key != "ArrowUp" {
                    return;
                }
                let Some(aside) = event
                    .current_target()
                    .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                else {
                    return;
                };
                fn collect(el: &web_sys::Element, out: &mut Vec<web_sys::HtmlElement>) {
                    if el.class_list().contains("nav__item") {
                        if let Ok(btn) = el.clone().dyn_into::<web_sys::HtmlElement>() {
                            out.push(btn);
                        }
                    }
                    let children = el.children();
                    for i in 0..children.length() {
                        if let Some(child) = children.item(i) {
                            collect(&child, out);
                        }
                    }
                }
                let mut buttons = Vec::new();
                collect(&aside, &mut buttons);
                let active = document().active_element();
                let Some(pos) = buttons
                    .iter()
                    .position(|b| active.as_ref().is_some_and(|a| a.loose_eq(b.unchecked_ref())))
                else {
                    return;
                };
                let next = if key == "ArrowDown" {
                    (pos + 1).min(buttons.len().saturating_sub(1))
                } else {
                    pos.saturating_sub(1)
                };
                if let Some(target) = buttons.get(next) {
                    event.prevent_default();
                    event.stop_propagation();
                    let _ = target.focus();
                }
            }
        >
            <div class="sidebar__header">
                <div class="sidebar__brand">"Posthaste"</div>
                <IconButton
                    class="sidebar__toggle"
                    icon=IconName::Sidebar
                    label=sidebar_toggle_label
                    expanded=Signal::derive(move || !sidebar_collapsed.get())
                    on_click=move |()| main.toggle_sidebar()
                />
            </div>
            <nav class="nav" aria-label=i18n.text(key::INBOX_SECTIONS)>
                {View::LISTS.into_iter().map(nav_item).collect_view()}
            </nav>
            <div class="sidebar__projects-wrap">
                <div class="sidebar__group-header">
                    <span class="sidebar__group-label">{i18n.text(key::INBOX_NAV_PROJECTS)}</span>
                    <button
                        type="button"
                        class="nav__item sidebar__manage"
                        aria-label=i18n.text(key::INBOX_NAV_MANAGE_PROJECTS)
                        title=i18n.text(key::INBOX_NAV_MANAGE_PROJECTS)
                        aria-current=move || {
                            (main.view.get() == View::ManageProjects && !main.searching())
                                .then_some("page")
                        }
                        on:click=move |_| main.go(View::ManageProjects)
                    >
                        <Icon name=IconName::Settings size=13 />
                    </button>
                </div>
                <nav class="nav sidebar__projects" aria-label=i18n.text(key::INBOX_NAV_PROJECTS)>
                    <For
                        each=move || {
                            main.projects.with(|list| {
                                list.iter()
                                    .filter(|p| !p.archived)
                                    .cloned()
                                    .collect::<Vec<_>>()
                            })
                        }
                        key=|p| (p.name.clone(), p.color.clone(), p.total)
                        let:project
                    >
                        {
                            let name = project.name.clone();
                            let name_cur = name.clone();
                            let name_click = name.clone();
                            let color = project.color.clone();
                            let total = project.total;
                            let current = move || {
                                let here = main.view.with(|v| {
                                    matches!(v, View::Project(cur) if cur == &name_cur)
                                }) && !main.searching();
                                here.then_some("page")
                            };
                            view! {
                                <button
                                    type="button"
                                    class="nav__item"
                                    aria-current=current
                                    on:click=move |_| main.go(View::Project(name_click.clone()))
                                >
                                    {color.map(|c| view! { <ProjectDot color=c /> })}
                                    <span class="nav__label">{name}</span>
                                    {(total > 0).then(|| view! {
                                        <span class="nav__count">{total}</span>
                                    })}
                                </button>
                            }
                        }
                    </For>
                </nav>
            </div>
            <div class="sidebar__spacer"></div>
            <button
                type="button"
                class="btn sidebar__new"
                data-collapsed=move || sidebar_collapsed.get().then_some("")
                aria-label=move || sidebar_collapsed.get().then(|| i18n.translator().text(key::INBOX_NEW_NOTE).to_owned())
                title=move || sidebar_collapsed.get().then(|| i18n.translator().text(key::INBOX_NEW_NOTE).to_owned())
                on:click=move |_| main.new_note()
            >
                <span class="sidebar__new__icon">
                    <Icon name=IconName::Plus />
                </span>
                <span class="sidebar__new__label">
                    {i18n.text(key::INBOX_NEW_NOTE)}
                </span>
                <Show when=move || !sidebar_collapsed.get() && capture_keys.with(|keys| !keys.is_empty())>
                    <Kbd keys=capture_keys />
                </Show>
            </button>
            {nav_item(View::Settings)}
        </aside>
    }
}
