//! The Trash section: deleted notes for 30 days, to restore or erase for
//! good.

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::{Note, TrashedNote, TRASH_RETENTION_MS};

use super::notes::day_label;
use crate::browser;
use crate::components::{Button, Dialog, EmptyState, IconButton, IconName, Toaster, Variant};
use crate::ipc::{api, events};
use crate::logic::dates::Day;
use crate::logic::inbox::group_runs;
use crate::logic::selection::{moved_from_none, neighbour};
use crate::logic::trash::{
    excerpt, key_action, returns_to, KeyAction, KeyContext, KeyPress, Returns,
};
use crate::state::{I18n, Message};
use crate::time::{self, DateStyle, Width};
use crate::windows::note_badges::NoteBadges;

const RETENTION_DAYS: i64 = TRASH_RETENTION_MS / 86_400_000;
/// How much of a note names it in the confirmation.
const EXCERPT_CHARS: usize = 80;

/// What the user is being asked to confirm.
#[derive(Clone, PartialEq)]
enum Confirm {
    One(Note),
    All,
}

#[derive(Clone, Copy)]
struct Trash {
    toaster: Toaster,
    /// `None` until the first answer arrives.
    items: RwSignal<Option<Vec<TrashedNote>>>,
    selected: RwSignal<Option<String>>,
    confirm: RwSignal<Option<Confirm>>,
    request: StoredValue<u32>,
}

impl Trash {
    fn load(self) {
        let request = self.request.get_value().wrapping_add(1);
        self.request.set_value(request);
        spawn_local(async move {
            let answer = api::trash_list().await;
            if self.request.get_value() != request {
                return;
            }
            match answer {
                Ok(items) => self.items.set(Some(items)),
                Err(error) => self.toaster.fail(error),
            }
        });
    }

    fn with_ids<T>(self, read: impl FnOnce(&[&str]) -> T) -> T {
        self.items.with_untracked(|items| {
            let ids: Vec<&str> = items
                .iter()
                .flatten()
                .map(|item| item.note.id.as_str())
                .collect();
            read(&ids)
        })
    }

    fn selected_note(self) -> Option<Note> {
        let id = self.selected.get_untracked()?;
        self.items.with_untracked(|items| {
            items
                .iter()
                .flatten()
                .find(|item| item.note.id == id)
                .map(|item| item.note.clone())
        })
    }

    /// Keeps the selection on a neighbour when a row leaves the list.
    fn select_neighbour(self, id: &str) {
        self.selected.set(self.with_ids(|ids| neighbour(ids, id)));
    }

    fn restore(self, note: &Note) {
        self.select_neighbour(&note.id);
        let id = note.id.clone();
        let restored = match returns_to(note) {
            Returns::Inbox => key::TRASH_RESTORED_INBOX,
            Returns::Later => key::TRASH_RESTORED_LATER,
            Returns::Archive => key::TRASH_RESTORED_ARCHIVE,
        };
        let toaster = self.toaster;
        spawn_local(async move {
            match api::note_undelete(&id).await {
                Ok(()) => toaster.undoable(Message::Plain(restored), move || {
                    let id = id.clone();
                    spawn_local(async move {
                        if let Err(error) = api::note_delete(&id).await {
                            toaster.fail(error);
                        }
                    });
                }),
                Err(error) => toaster.fail(error),
            }
        });
    }

    /// Erases what the open confirmation is about.
    fn erase(self) {
        let Some(confirm) = self.confirm.get_untracked() else {
            return;
        };
        self.confirm.set(None);
        let toaster = self.toaster;
        match confirm {
            Confirm::One(note) => {
                self.select_neighbour(&note.id);
                spawn_local(async move {
                    match api::trash_purge(&note.id).await {
                        Ok(()) => toaster.info(Message::Plain(key::TRASH_DELETED)),
                        Err(error) => toaster.fail(error),
                    }
                });
            }
            Confirm::All => {
                self.selected.set(None);
                // Only what the user saw: up to the newest deletion listed.
                let newest = self.items.with_untracked(|items| {
                    items
                        .iter()
                        .flatten()
                        .next()
                        .map_or(0, |item| item.deleted_at)
                });
                spawn_local(async move {
                    match api::trash_empty(newest).await {
                        Ok(count) => {
                            toaster.info(Message::count(key::TRASH_DELETED_MANY, count as u64));
                        }
                        Err(error) => toaster.fail(error),
                    }
                });
            }
        }
    }

    fn on_key(self, event: KeyboardEvent) {
        let key = event.key();
        let selected = self.selected_note();
        let press = KeyPress {
            key: &key,
            modified: event.meta_key() || event.ctrl_key() || event.alt_key(),
            composing: event.is_composing(),
        };
        let context = KeyContext {
            busy: browser::dialog_is_open() || browser::is_typing(&event),
            has_selection: selected.is_some(),
            on_button: browser::is_within(&event, "button"),
        };
        let Some(action) = key_action(press, context) else {
            return;
        };
        event.prevent_default();
        match (action, selected) {
            (KeyAction::Move(delta), _) => {
                let current = self.selected.get_untracked();
                let moved = self.with_ids(|ids| moved_from_none(ids, current.as_deref(), delta));
                if moved.is_some() {
                    self.selected.set(moved);
                }
            }
            (KeyAction::Restore, Some(note)) => self.restore(&note),
            (KeyAction::ConfirmErase, Some(note)) => self.confirm.set(Some(Confirm::One(note))),
            // `key_action` only asks for these with a note selected.
            (KeyAction::Restore | KeyAction::ConfirmErase, None) => {}
        }
    }
}

#[component]
pub(super) fn TrashView(i18n: I18n, locale: Memo<String>, toaster: Toaster) -> impl IntoView {
    let trash = Trash {
        toaster,
        items: RwSignal::new(None),
        selected: RwSignal::new(None),
        confirm: RwSignal::new(None),
        request: StoredValue::new(0),
    };
    trash.load();
    events::on::<events::NotesChanged>(move |()| trash.load());
    let keys = window_event_listener(leptos::ev::keydown, move |event| trash.on_key(event));
    on_cleanup(move || keys.remove());

    let count = move || trash.items.with(|items| items.as_ref().map_or(0, Vec::len));
    // Newest first, a day of deletions at a time.
    let groups = Memo::new(move |_| {
        trash.items.with(|items| {
            group_runs(items.iter().flatten().cloned(), |item| {
                time::day_of(item.deleted_at)
            })
        })
    });
    let days = move || groups.with(|groups| groups.iter().map(|(day, _)| *day).collect::<Vec<_>>());
    let items_of = move |day: Day| {
        groups.with(|groups| {
            groups
                .iter()
                .find(|(group, _)| *group == day)
                .map(|(_, items)| items.clone())
                .unwrap_or_default()
        })
    };
    let kept_for = move |key| {
        let days = RETENTION_DAYS.to_string();
        i18n.translator().format(key, &[("days", &days)])
    };
    let close = move |()| trash.confirm.set(None);
    let confirming_one = move || {
        trash
            .confirm
            .with(|confirm| matches!(confirm, Some(Confirm::One(_))))
    };
    let confirming_all = move || {
        trash
            .confirm
            .with(|confirm| matches!(confirm, Some(Confirm::All)))
    };
    let quoted = move || {
        trash.confirm.with(|confirm| match confirm {
            Some(Confirm::One(note)) => Some(excerpt(&note.content, EXCERPT_CHARS)),
            _ => None,
        })
    };

    view! {
        <Show when=move || trash.items.with(Option::is_some)>
            <Show
                when=move || count() != 0
                fallback=move || view! {
                    <EmptyState title=i18n.label(key::TRASH_EMPTY_TITLE)>
                        {move || kept_for(key::TRASH_EMPTY_TEXT)}
                    </EmptyState>
                }
            >
                <div class="trash__bar">
                    <p class="trash__note">
                        {move || kept_for(key::TRASH_NOTE)}
                    </p>
                    <Button variant=Variant::Quiet on_click=move |()| trash.confirm.set(Some(Confirm::All))>
                        {i18n.text(key::TRASH_EMPTY_ACTION)}
                    </Button>
                </div>
                <For each=days key=|day| *day let:day>
                    {
                        let label = move || day_label(day, time::today(), &locale.get(), i18n.translator());
                        view! {
                            <section class="day" aria-label=label>
                                <h2 class="day__label">
                                    {label}
                                    <span class="day__count">{move || items_of(day).len()}</span>
                                </h2>
                                <ul class="notes">
                                    <For each=move || items_of(day) key=|item| item.note.id.clone() let:item>
                                        <Row trash=trash i18n=i18n locale=locale item=item />
                                    </For>
                                </ul>
                            </section>
                        }
                    }
                </For>
            </Show>
        </Show>

        <Dialog
            open=Signal::derive(confirming_one)
            title=i18n.label(key::TRASH_CONFIRM_ONE_TITLE)
            on_close=close
            actions=Box::new(move || view! {
                <Button autofocus=true on_click=close>{i18n.text(key::COMMON_CANCEL)}</Button>
                <Button variant=Variant::Danger on_click=move |()| trash.erase()>
                    {i18n.text(key::TRASH_CONFIRM_ONE_ACTION)}
                </Button>
            }.into_any())
        >
            {move || quoted().map(|text| view! { <p class="dialog__quote selectable">{text}</p> })}
            <p class="dialog__text">{i18n.text(key::TRASH_CONFIRM_ONE_TEXT)}</p>
        </Dialog>
        <Dialog
            open=Signal::derive(confirming_all)
            title=i18n.label(key::TRASH_CONFIRM_ALL_TITLE)
            on_close=close
            actions=Box::new(move || view! {
                <Button autofocus=true on_click=close>{i18n.text(key::COMMON_CANCEL)}</Button>
                <Button variant=Variant::Danger on_click=move |()| trash.erase()>
                    {i18n.text(key::TRASH_CONFIRM_ALL_ACTION)}
                </Button>
            }.into_any())
        >
            <p class="dialog__text">
                {move || i18n.translator().plural(key::TRASH_CONFIRM_ALL_TEXT, count() as u64, &[])}
            </p>
        </Dialog>
    }
}

/// One deleted note: where it goes back to, when it is erased, and the two
/// things that can be done with it.
#[component]
fn Row(trash: Trash, i18n: I18n, locale: Memo<String>, item: TrashedNote) -> impl IntoView {
    let row = NodeRef::<html::Li>::new();
    let erased_at = item.deleted_at + TRASH_RETENTION_MS;
    let same_year = time::day_of(erased_at).year == time::day_of(item.deleted_at).year;
    let returns = match returns_to(&item.note) {
        Returns::Inbox => key::TRASH_RETURNS_INBOX,
        Returns::Later => key::TRASH_RETURNS_LATER,
        Returns::Archive => key::TRASH_RETURNS_ARCHIVE,
    };
    let note = StoredValue::new(item.note);
    let id = note.with_value(|note| note.id.clone());
    let selected = Memo::new(move |_| trash.selected.with(|s| s.as_deref() == Some(id.as_str())));

    // Keyboard selection can move past the visible rows: keep it on screen,
    // so Delete never acts on a row the user cannot see.
    Effect::new(move |_| {
        if let (true, Some(row)) = (selected.get(), row.get()) {
            let options = web_sys::ScrollIntoViewOptions::new();
            options.set_block(web_sys::ScrollLogicalPosition::Nearest);
            row.scroll_into_view_with_scroll_into_view_options(&options);
        }
    });
    let erased = move || {
        let style = DateStyle {
            weekday: None,
            month: Some(Width::Short),
            day: true,
            year: !same_year,
        };
        let date = time::format_day(time::day_of(erased_at), &locale.get(), style).replace(',', "");
        i18n.translator()
            .format(key::TRASH_ERASED_ON, &[("date", &date)])
    };

    view! {
        <li
            node_ref=row
            class="note note--trash"
            data-selected=move || selected.get().to_string()
            aria-current=move || selected.get().then_some("true")
            on:click=move |_| note.with_value(|note| trash.selected.set(Some(note.id.clone())))
        >
            <div class="note__body">
                <p class="note__content selectable" data-clamped="true">
                    {note.with_value(|note| note.content.clone())}
                </p>
                <div class="note__meta">
                    <span class="trash__returns">{i18n.text(returns)}</span>
                    <span>{erased}</span>
                    <NoteBadges note=note.get_value() i18n=i18n />
                </div>
            </div>
            <div class="note__actions" on:click=move |event| event.stop_propagation()>
                <IconButton
                    icon=IconName::Restore
                    label=i18n.label(key::TRASH_RESTORE)
                    on_click=move |()| note.with_value(|note| trash.restore(note))
                />
                <IconButton
                    icon=IconName::Trash
                    label=i18n.label(key::TRASH_DELETE_FOREVER)
                    danger=true
                    on_click=move |()| trash.confirm.set(Some(Confirm::One(note.get_value())))
                />
            </div>
        </li>
    }
}
