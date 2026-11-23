//! Ordered one-line steps inside Note details: state mutations and UI components.

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::{NewStep, Os, Step, MAX_STEPS_PER_NOTE, MAX_STEP_CHARS};

use super::{Details, Save, StepEditor, StepSync, SAVE_DELAY_MS};
use crate::browser::{self, bind_text};
use crate::components::{Icon, IconButton, IconName};
use crate::draft_sync::DraftSync;
use crate::ipc::api;
use crate::logic::details::KeyPress;
use crate::logic::selection::neighbour;
use crate::logic::shortcut::has_mod;
use crate::logic::steps::{badge_text, key_action as step_key_action, split_paste, StepKey};
use crate::state::{I18n, Message};

impl Details {
    /// The note's live steps, in display order.
    fn steps(self) -> Vec<Step> {
        self.detail
            .with_untracked(|detail| detail.as_ref().map(|d| d.steps.clone()).unwrap_or_default())
    }

    fn note_id(self) -> Option<String> {
        self.id.get_value()
    }

    fn next_step_epoch(self) -> u32 {
        let next = self.step_epoch.get_value().wrapping_add(1);
        self.step_epoch.set_value(next);
        next
    }

    /// Adding stops when the note holds the normal limit or more (steps
    /// merged in from another computer are never dropped to make room).
    fn step_limit_reached(self) -> bool {
        self.detail.with_untracked(|detail| {
            detail
                .as_ref()
                .is_some_and(|d| d.note.step_total as usize >= MAX_STEPS_PER_NOTE)
        })
    }

    fn select_step(self, id: Option<String>) {
        self.step_selected.set(id);
    }

    /// Moves the selection one step up or down from `id`.
    fn select_neighbour(self, id: &str, delta: isize) {
        let steps = self.steps();
        let Some(pos) = steps.iter().position(|s| s.id == id) else {
            return;
        };
        let next = pos as isize + delta;
        if next < 0 || next as usize >= steps.len() {
            return;
        }
        self.step_selected
            .set(Some(steps[next as usize].id.clone()));
    }

    fn step_write(&self, note_id: String, id: String) -> DraftSync {
        let details = *self;
        DraftSync::with_delay(
            SAVE_DELAY_MS,
            move |text, _| {
                let id = id.clone();
                let note_id = note_id.clone();
                async move {
                    api::step_set_text(&id, &text).await?;
                    if details.shows(&note_id) {
                        details.save.set(Save::Saved);
                    }
                    Ok(())
                }
            },
            move |error| details.fail(error),
        )
    }

    /// Edits a step's text in place, saving as typing pauses.
    fn edit_step(self, id: String) {
        let Some(note_id) = self.note_id() else {
            return;
        };
        let Some(text) = self.detail.with_untracked(|detail| {
            detail
                .as_ref()
                .and_then(|d| d.steps.iter().find(|s| s.id == id).map(|s| s.text.clone()))
        }) else {
            return;
        };
        let epoch = self.next_step_epoch();
        spawn_local(async move {
            self.flush().await;
            if self.step_epoch.get_value() != epoch {
                return;
            }
            self.step_text.set(text);
            self.step_editor.set(Some(StepEditor {
                id: Some(id.clone()),
                is_add_line: false,
            }));
            let sync = self.step_write(note_id, id.clone());
            self.step_sync.set_value(Some(StepSync { sync }));
            self.step_selected.set(Some(id));
        });
    }

    /// Opens the quiet "Add a step" line.
    fn open_add(self) {
        if self.step_limit_reached() || self.note_id().is_none() {
            return;
        }
        let epoch = self.next_step_epoch();
        spawn_local(async move {
            self.flush().await;
            if self.step_epoch.get_value() != epoch {
                return;
            }
            self.step_sync.set_value(None);
            self.step_text.set(String::new());
            self.step_editor.set(Some(StepEditor {
                id: None,
                is_add_line: true,
            }));
            self.step_selected.set(None);
        });
    }

    /// What typing in the open editor does. The add line's row is created
    /// on its first non-blank keystroke; from then on typing writes.
    fn edit_step_text(self, text: String) {
        self.step_text.set(text.clone());
        if text.trim().is_empty() {
            return;
        }
        match self.step_sync.get_value() {
            Some(sync) => {
                self.save.set(Save::Saving);
                sync.sync.schedule(text, None);
            }
            None => {
                let Some(note_id) = self.note_id() else {
                    return;
                };
                // One creation in flight: the keystrokes that land while it
                // runs are recorded in `creating_text` and go out once the
                // row exists. Another write now would make another row.
                if self.step_creating.get_value() {
                    self.creating_text.set_value(text);
                    return;
                }
                self.step_creating.set_value(true);
                self.creating_text.set_value(text.clone());
                let epoch = self.step_epoch.get_value();
                let after = self.steps().last().map(|s| s.id.clone());
                spawn_local(async move {
                    let created = api::steps_add(
                        &note_id,
                        after.as_deref(),
                        &[NewStep { text, done: false }],
                    )
                    .await;
                    self.step_creating.set_value(false);
                    match created {
                        Ok(created) => {
                            let Some(step) = created.first() else {
                                return;
                            };
                            self.save.set(Save::Saved);
                            self.step_selected.set(Some(step.id.clone()));
                            let same_epoch = self.step_epoch.get_value() == epoch;
                            // The editor may have closed (Esc) or moved on to
                            // the next step (Enter) while the row was being
                            // made: never attach this row to a newer add line.
                            let add_line_open = same_epoch
                                && self.step_editor.with_untracked(|editor| {
                                    editor
                                        .as_ref()
                                        .is_some_and(|e| e.is_add_line && e.id.is_none())
                                });
                            if add_line_open {
                                self.step_editor.update(|editor| {
                                    if let Some(editor) = editor {
                                        editor.id = Some(step.id.clone());
                                    }
                                });
                                let sync = self.step_write(note_id, step.id.clone());
                                // What was typed while the write ran goes out
                                // once the row exists.
                                let typed = self.step_text.get_untracked();
                                if typed != step.text && !typed.trim().is_empty() {
                                    self.save.set(Save::Saving);
                                    sync.schedule(typed, None);
                                }
                                self.step_sync.set_value(Some(StepSync { sync }));
                            } else {
                                let final_text = self.creating_text.get_value();
                                if final_text != step.text && !final_text.trim().is_empty() {
                                    if let Err(error) =
                                        api::step_set_text(&step.id, &final_text).await
                                    {
                                        self.fail(error);
                                    }
                                }
                                // If the user already pressed Enter and began
                                // typing the next step while this one was being
                                // created, start creating that next step now.
                                let next_waiting = self.step_editor.with_untracked(|editor| {
                                    editor
                                        .as_ref()
                                        .is_some_and(|e| e.is_add_line && e.id.is_none())
                                });
                                let next_text = self.step_text.get_untracked();
                                if next_waiting && !next_text.trim().is_empty() {
                                    self.edit_step_text(next_text);
                                }
                            }
                        }
                        Err(error) => self.fail(error),
                    }
                });
            }
        }
    }

    /// A paste of several lines becomes one step per line, all or nothing.
    fn paste_steps(self, text: String) {
        let lines = split_paste(&text);
        if lines.len() < 2 {
            return;
        }
        let Some(note_id) = self.note_id() else {
            return;
        };
        let after = self.steps().last().map(|s| s.id.clone());
        spawn_local(async move {
            match api::steps_add(&note_id, after.as_deref(), &lines).await {
                Ok(_) => {
                    self.save.set(Save::Saved);
                    self.step_editor.set(None);
                    self.step_sync.set_value(None);
                    self.step_text.set(String::new());
                }
                Err(error) => self.fail(error),
            }
        });
    }

    /// Enter in the editor: save, then make room for the next step.
    fn step_enter(self) {
        let epoch = self.next_step_epoch();
        let current = self.step_editor.get_untracked();
        let was_add_line = current.as_ref().is_some_and(|e| e.is_add_line);
        let empty = self.step_text.with_untracked(|t| t.trim().is_empty());
        if !was_add_line && empty {
            if let Some(id) = current.and_then(|e| e.id) {
                self.remove_step(id);
            }
            return;
        }
        spawn_local(async move {
            self.flush().await;
            if self.step_epoch.get_value() != epoch {
                return;
            }
            self.step_sync.set_value(None);
            self.step_text.set(String::new());
            if self.step_limit_reached() {
                self.step_editor.set(None);
            } else if was_add_line {
                // Ready for the next step; its row appears on the first key.
                self.step_editor.update(|editor| {
                    if let Some(editor) = editor {
                        editor.id = None;
                    }
                });
            } else {
                self.step_editor.set(Some(StepEditor {
                    id: None,
                    is_add_line: true,
                }));
            }
        });
    }

    /// Esc, or Enter/blur on an empty line: leave, keeping what was saved.
    fn step_leave(self) {
        let Some(current) = self.step_editor.get_untracked() else {
            return;
        };
        let epoch = self.next_step_epoch();
        let was_add_line = current.is_add_line;
        let empty = self.step_text.with_untracked(|t| t.trim().is_empty());
        let empty_add_id = (was_add_line && empty).then_some(current.id).flatten();
        spawn_local(async move {
            self.flush().await;
            if let Some(id) = empty_add_id {
                let _ = api::step_remove(&id).await;
            }
            if self.step_epoch.get_value() != epoch {
                return;
            }
            self.step_editor.set(None);
            self.step_sync.set_value(None);
            if was_add_line {
                self.step_text.set(String::new());
            }
        });
    }

    fn toggle_step(self, id: String, done: bool) {
        spawn_local(async move {
            if let Err(error) = api::step_set_done(&id, done).await {
                self.fail(error);
            }
        });
    }

    /// Removes a step, with an undo toast that puts it back.
    fn remove_step(self, id: String) {
        let next = {
            let steps = self.steps();
            let ids: Vec<&str> = steps.iter().map(|s| s.id.as_str()).collect();
            neighbour(&ids, &id)
        };
        let editing_this = self
            .step_editor
            .with_untracked(|e| e.as_ref().is_some_and(|e| e.id.as_deref() == Some(&id)));
        if editing_this {
            self.step_epoch.update_value(|e| *e = e.wrapping_add(1));
            self.step_editor.set(None);
            self.step_sync.set_value(None);
            self.step_text.set(String::new());
        }
        spawn_local(async move {
            match api::step_remove(&id).await {
                Ok(()) => {
                    self.step_selected.set(next);
                    self.toaster
                        .undoable(Message::Plain(key::DETAILS_STEPS_REMOVED), move || {
                            let id = id.clone();
                            spawn_local(async move {
                                if let Err(error) = api::step_restore(&id).await {
                                    self.fail(error);
                                }
                            });
                        });
                }
                Err(error) => self.fail(error),
            }
        });
    }

    /// Moves a step one place up or down; only the rank changes.
    fn move_step(self, id: String, up: bool) {
        let steps = self.steps();
        let Some(pos) = steps.iter().position(|s| s.id == id) else {
            return;
        };
        let after = if up {
            if pos == 0 {
                return;
            }
            (pos > 1).then(|| steps[pos - 2].id.clone())
        } else {
            if pos + 1 >= steps.len() {
                return;
            }
            Some(steps[pos + 1].id.clone())
        };
        spawn_local(async move {
            if let Err(error) = api::step_move(&id, after.as_deref()).await {
                self.fail(error);
            }
        });
    }
}

/// The note's steps: a labelled list between the title and the description,
/// then a quiet "Add a step" line. Keys work wherever focus sits in the
/// section; the window's own keys keep working around it.
#[component]
pub(super) fn StepsSection(details: Details, i18n: I18n, os: Os) -> impl IntoView {
    // While a new step is still being typed in the add line, it is drawn
    // inside AddLine itself, never duplicated as a separate row above it.
    let add_target_id = Memo::new(move |_| {
        details.step_editor.with(|e| {
            e.as_ref()
                .and_then(|e| e.is_add_line.then(|| e.id.clone()).flatten())
        })
    });
    let steps = Memo::new(move |_| {
        let adding_id = add_target_id.get();
        details.detail.with(|d| {
            d.as_ref()
                .map(|d| {
                    d.steps
                        .iter()
                        .filter(|s| adding_id.as_deref() != Some(s.id.as_str()))
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    let any = move || steps.with(|s| !s.is_empty());
    let badge = Memo::new(move |_| {
        details.detail.with(|d| {
            d.as_ref()
                .and_then(|d| badge_text(d.note.step_done, d.note.step_total))
        })
    });
    let at_limit = Memo::new(move |_| {
        let adding = details
            .step_editor
            .with(|e| e.as_ref().is_some_and(|e| e.is_add_line));
        !adding
            && details.detail.with(|detail| {
                detail
                    .as_ref()
                    .is_some_and(|d| d.note.step_total as usize >= MAX_STEPS_PER_NOTE)
            })
    });

    view! {
        <section class="details__steps" aria-label=i18n.text(key::DETAILS_STEPS_TITLE)>
            <h2 class="details__steps-title">
                {i18n.text(key::DETAILS_STEPS_TITLE)}
                <Show when=move || badge.with(|b| b.is_some())>
                    <span class="details__count">{move || badge.get().unwrap_or_default()}</span>
                </Show>
            </h2>
            <Show when=any>
                <ul class="steps" role="list">
                    <For each=move || steps.get() key=|s| s.id.clone() let:step>
                        <StepRow details=details i18n=i18n os=os step=step />
                    </For>
                </ul>
            </Show>
            <Show
                when=move || !at_limit.get()
                fallback=move || {
                    view! { <p class="details__steps-limit">{i18n.text(key::DETAILS_STEPS_LIMIT)}</p> }
                }
            >
                <AddLine details=details i18n=i18n os=os />
            </Show>
        </section>
    }
}

#[component]
fn StepRow(details: Details, i18n: I18n, os: Os, step: Step) -> impl IntoView {
    let row = NodeRef::<html::Button>::new();
    let edit_field = NodeRef::<html::Input>::new();
    let id = StoredValue::new(step.id.clone());
    let is_this = move |shown: &Option<String>| shown.as_deref() == Some(id.get_value().as_str());
    let selected = Memo::new(move |_| details.step_selected.with(is_this));
    let editor = Memo::new(move |_| {
        details.step_editor.with(|e| {
            e.as_ref()
                .is_some_and(|e| !e.is_add_line && e.id.as_deref() == Some(id.get_value().as_str()))
        })
    });
    let any_editor = Memo::new(move |_| details.step_editor.with(Option::is_some));
    // Focus follows the selection when no step input is open, so typing in
    // the add line never loses focus when its row mounts.
    Effect::new(move |_| {
        if selected.get() && !any_editor.get() {
            if let Some(row) = row.get() {
                let _ = row.focus();
            }
        }
    });
    // Put the caret at the end of the step text as soon as edit mode opens.
    Effect::new(move |_| {
        if editor.get() {
            if let Some(field) = edit_field.get() {
                browser::focus_input_end(&field);
            }
        }
    });
    // The row reads its current text and state from the note, so a save
    // updates it in place.
    let text = Memo::new(move |_| {
        details.detail.with(|d| {
            d.as_ref()
                .and_then(|d| d.steps.iter().find(|s| s.id == id.get_value()))
                .map(|s| s.text.clone())
                .unwrap_or_default()
        })
    });
    let done = Memo::new(move |_| {
        details.detail.with(|d| {
            d.as_ref()
                .and_then(|d| d.steps.iter().find(|s| s.id == id.get_value()))
                .is_some_and(|s| s.done)
        })
    });

    let on_key = move |event: KeyboardEvent| {
        if event.is_composing() {
            return;
        }
        let press = KeyPress {
            key: &event.key(),
            primary: has_mod(event.meta_key(), event.ctrl_key(), os),
            shift: event.shift_key(),
            alt: event.alt_key(),
            composing: event.is_composing(),
        };
        let Some(action) = step_key_action(press, false, false) else {
            return;
        };
        event.prevent_default();
        event.stop_propagation();
        let id = id.get_value();
        match action {
            StepKey::Up => details.select_neighbour(&id, -1),
            StepKey::Down => details.select_neighbour(&id, 1),
            StepKey::Toggle => details.toggle_step(id, !done.get_untracked()),
            StepKey::Edit => details.edit_step(id),
            StepKey::Remove => details.remove_step(id),
            StepKey::MoveUp => details.move_step(id, true),
            StepKey::MoveDown => details.move_step(id, false),
            StepKey::Enter | StepKey::Leave => {}
        }
    };

    view! {
        <li class="steps__item">
            {move || {
                if editor.get() {
                    view! {
                        <div class="step step--editing" aria-checked=move || done.get().to_string()>
                            <span
                                class="step__mark"
                                aria-hidden="true"
                                on:mousedown=move |event| event.prevent_default()
                                on:click=move |_| {
                                    details.toggle_step(id.get_value(), !done.get_untracked());
                                }
                            >
                                {(done.get()).then(|| view! { <Icon name=IconName::Check size=12 /> })}
                            </span>
                            <input
                                node_ref=edit_field
                                class="step__input"
                                aria-label=i18n.text(key::DETAILS_STEPS_TITLE)
                                maxlength=MAX_STEP_CHARS.to_string()
                                spellcheck="false"
                                use:bind_text=Signal::derive(move || details.step_text.get())
                                on:input=move |event| details.edit_step_text(event_target_value(&event))
                                on:blur=move |_| {
                                    if editor.get_untracked() {
                                        if details.step_text.with_untracked(|t| t.trim().is_empty()) {
                                            details.remove_step(id.get_value());
                                        } else {
                                            details.step_leave();
                                        }
                                    }
                                }
                                on:keydown=move |event| {
                                    if event.is_composing() {
                                        return;
                                    }
                                    let press = KeyPress {
                                        key: &event.key(),
                                        primary: has_mod(event.meta_key(), event.ctrl_key(), os),
                                        shift: event.shift_key(),
                                        alt: event.alt_key(),
                                        composing: event.is_composing(),
                                    };
                                    // Mod+Enter and Mod+O stay with the window.
                                    if press.primary {
                                        return;
                                    }
                                    let empty = details.step_text.with(|t| t.trim().is_empty());
                                    if empty && matches!(press.key, "Backspace" | "Delete" | "Enter") {
                                        event.prevent_default();
                                        event.stop_propagation();
                                        details.remove_step(id.get_value());
                                        return;
                                    }
                                    match step_key_action(press, true, empty) {
                                        Some(StepKey::Enter) => {
                                            event.prevent_default();
                                            event.stop_propagation();
                                            details.step_enter();
                                        }
                                        Some(StepKey::Leave) => {
                                            event.prevent_default();
                                            event.stop_propagation();
                                            details.step_leave();
                                        }
                                        _ => {}
                                    }
                                }
                            />
                        </div>
                    }
                    .into_any()
                } else {
                    view! {
                        <button
                            node_ref=row
                            type="button"
                            role="checkbox"
                            class="step"
                            aria-checked=move || done.get().to_string()
                            data-selected=move || selected.get().to_string()
                            on:click=move |_| {
                                details.select_step(Some(id.get_value()));
                                details.toggle_step(id.get_value(), !done.get_untracked());
                            }
                            on:keydown=on_key
                        >
                            <span class="step__mark" aria-hidden="true">
                                {(done.get()).then(|| view! { <Icon name=IconName::Check size=12 /> })}
                            </span>
                            <span
                                class="step__text"
                                on:click=move |event| {
                                    event.stop_propagation();
                                    details.edit_step(id.get_value());
                                }
                            >
                                {text}
                            </span>
                        </button>
                    }
                    .into_any()
                }
            }}
            <IconButton
                class="step__remove"
                icon=IconName::Close
                label=i18n.label(key::DETAILS_STEPS_REMOVE)
                on_click=move |()| details.remove_step(id.get_value())
            />
        </li>
    }
}

/// The quiet "Add a step" line: a row appears on the first non-blank
/// keystroke, and a paste of several lines becomes several steps.
#[component]
fn AddLine(details: Details, i18n: I18n, os: Os) -> impl IntoView {
    let open = Memo::new(move |_| {
        details
            .step_editor
            .with(|e| e.as_ref().is_some_and(|e| e.is_add_line))
    });
    let field = NodeRef::<html::Input>::new();
    Effect::new(move |_| {
        if open.get() {
            if let Some(field) = field.get() {
                let _ = field.focus();
            }
        }
    });

    let on_key = move |event: KeyboardEvent| {
        if event.is_composing() {
            return;
        }
        let press = KeyPress {
            key: &event.key(),
            primary: has_mod(event.meta_key(), event.ctrl_key(), os),
            shift: event.shift_key(),
            alt: event.alt_key(),
            composing: event.is_composing(),
        };
        // Mod+Enter and Mod+O stay with the window.
        if press.primary {
            return;
        }
        let empty = details.step_text.with(|t| t.trim().is_empty());
        match step_key_action(press, true, empty) {
            Some(StepKey::Enter) => {
                event.prevent_default();
                event.stop_propagation();
                details.step_enter();
            }
            Some(StepKey::Leave) => {
                event.prevent_default();
                event.stop_propagation();
                details.step_leave();
            }
            _ => {}
        }
    };
    let on_paste = move |event: web_sys::ClipboardEvent| {
        let Some(data) = event.clipboard_data() else {
            return;
        };
        let Ok(text) = data.get_data("text/plain") else {
            return;
        };
        if text.lines().count() < 2 {
            return;
        }
        event.prevent_default();
        event.stop_propagation();
        details.paste_steps(text);
    };

    view! {
        <Show
            when=move || open.get()
            fallback=move || {
                view! {
                    <button
                        type="button"
                        class="details__steps-add"
                        on:click=move |_| details.open_add()
                    >
                        <Icon name=IconName::Plus size=12 />
                        " "
                        {i18n.text(key::DETAILS_STEPS_ADD)}
                    </button>
                }
            }
        >
            <div class="step step--editing">
                <span class="step__mark" aria-hidden="true"></span>
                <input
                    node_ref=field
                    class="step__input"
                    aria-label=i18n.text(key::DETAILS_STEPS_ADD)
                    placeholder=i18n.text(key::DETAILS_STEPS_ADD)
                    maxlength=MAX_STEP_CHARS.to_string()
                    spellcheck="false"
                    use:bind_text=Signal::derive(move || details.step_text.get())
                    on:input=move |event| details.edit_step_text(event_target_value(&event))
                    on:blur=move |_| {
                        if open.get_untracked() {
                            details.step_leave();
                        }
                    }
                    on:keydown=on_key
                    on:paste=on_paste
                />
            </div>
        </Show>
    }
}
