//! The Note details window: a note's title, its longer description and the
//! files attached to it.
//!
//! Nothing here has a Save button. The title and the description are
//! written shortly after typing pauses, and always before the window closes
//! or moves to another note.

use std::collections::BTreeMap;

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Translator};
use posthaste_model::{
    AttachReport, Attachment, CommandError, DetailsTarget, NewStep, Note, NoteDetail, Os, Status,
    Step, MAX_ATTACHMENTS_PER_NOTE, MAX_ATTACHMENT_BYTES, MAX_STEPS_PER_NOTE, MAX_STEP_CHARS,
};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::browser::{self, bind_text};
use crate::components::{caps, Icon, IconButton, IconName, Kbd, Toaster, Toasts};
use crate::draft_sync::DraftSync;
use crate::ipc::{api, events};
use crate::logic::attachments::{
    data_url, format_bytes, has_room, image_extension, is_unnamed_paste, type_label, Kind,
};
use crate::logic::details::{
    badge, carried_days, field_after_load, key_action, was_edited, Badge, KeyAction, KeyPress,
};
use crate::logic::selection::neighbour;
use crate::logic::shortcut::{has_mod, mod_cap};
use crate::logic::steps::{badge_text, key_action as step_key_action, split_paste, StepKey};
use crate::state::{use_appearance, use_i18n, use_settings, I18n, Message};
use crate::time::{self, DateStyle, Width};

/// How long typing pauses before the text is written.
const SAVE_DELAY_MS: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Save {
    Idle,
    Saving,
    Saved,
}

/// Which step editor is open. The add line's row exists only from its
/// first non-blank keystroke, so its id starts as `None`.
#[derive(Debug, Clone)]
struct StepEditor {
    id: Option<String>,
    is_add_line: bool,
}

/// A step text write in flight or pending.
#[derive(Clone)]
struct StepSync {
    sync: DraftSync,
}

/// Thumbnails by attachment id. `None` while one is being fetched.
type Previews = BTreeMap<String, Option<String>>;

/// Everything the window's actions read and write. `Copy`, so each handler
/// takes it by value.
#[derive(Clone, Copy)]
struct Details {
    detail: RwSignal<Option<NoteDetail>>,
    title: RwSignal<String>,
    text: RwSignal<String>,
    save: RwSignal<Save>,
    /// What just happened to a file. Shown in place of the save state.
    notice: RwSignal<Option<Message>>,
    errors: RwSignal<Vec<Message>>,
    previews: RwSignal<Previews>,
    /// Files are being held over the window.
    dropping: RwSignal<bool>,
    /// The selected step row, when one is.
    step_selected: RwSignal<Option<String>>,
    /// A step whose text is being edited: the row editor or the quiet
    /// "Add a step" line, whose row exists only from the first keystroke.
    step_editor: RwSignal<Option<StepEditor>>,
    /// What the step editor holds.
    step_text: RwSignal<String>,
    /// The open editor's debounced write, once its row exists.
    step_sync: StoredValue<Option<StepSync>, LocalStorage>,
    /// A step row is being created (the write is in flight): keystrokes
    /// that land meanwhile wait for it instead of making another row.
    step_creating: StoredValue<bool, LocalStorage>,
    /// Counts step editor sessions so an in-flight `steps_add` knows
    /// whether the user is still typing that same step or already pressed
    /// Enter / Esc.
    step_epoch: StoredValue<u32>,
    /// Latest text typed for the step whose `steps_add` is in flight.
    creating_text: StoredValue<String>,
    toaster: Toaster,
    /// The note being shown. Answers for any other note are dropped.
    id: StoredValue<Option<String>>,
    /// The text as last stored, to tell unsaved typing from a stale field.
    stored_text: StoredValue<String>,
    stored_title: StoredValue<String>,
    text_sync: StoredValue<Option<DraftSync>, LocalStorage>,
    title_sync: StoredValue<Option<DraftSync>, LocalStorage>,
    /// Counts loads, so a slow answer to an old one cannot replace a newer.
    request: StoredValue<u32>,
    closing: StoredValue<bool>,
    text_field: NodeRef<html::Textarea>,
    add_button: NodeRef<html::Button>,
}

impl Details {
    fn new() -> Self {
        Details {
            detail: RwSignal::new(None),
            title: RwSignal::new(String::new()),
            text: RwSignal::new(String::new()),
            save: RwSignal::new(Save::Idle),
            notice: RwSignal::new(None),
            errors: RwSignal::new(Vec::new()),
            previews: RwSignal::new(Previews::new()),
            dropping: RwSignal::new(false),
            step_selected: RwSignal::new(None),
            step_editor: RwSignal::new(None),
            step_text: RwSignal::new(String::new()),
            step_sync: StoredValue::new_local(None),
            step_creating: StoredValue::new_local(false),
            step_epoch: StoredValue::new(0),
            creating_text: StoredValue::new(String::new()),
            toaster: Toaster::new(),
            id: StoredValue::new(None),
            stored_text: StoredValue::new(String::new()),
            stored_title: StoredValue::new(String::new()),
            text_sync: StoredValue::new_local(None),
            title_sync: StoredValue::new_local(None),
            request: StoredValue::new(0),
            closing: StoredValue::new(false),
            text_field: NodeRef::new(),
            add_button: NodeRef::new(),
        }
    }

    fn fail(self, error: CommandError) {
        self.errors.set(vec![Message::Failure(error)]);
    }

    fn shows(self, id: &str) -> bool {
        self.id.with_value(|shown| shown.as_deref() == Some(id))
    }

    fn file_count(self) -> usize {
        self.detail
            .with_untracked(|detail| detail.as_ref().map_or(0, |d| d.attachments.len()))
    }

    fn focus_text(self) {
        if let Some(field) = self.text_field.get_untracked() {
            browser::focus_end(&field);
        }
    }

    /// Writes anything typed but not yet stored.
    async fn flush(self) {
        let text = self
            .text_sync
            .with_value(|sync| sync.as_ref().map(DraftSync::flush));
        let title = self
            .title_sync
            .with_value(|sync| sync.as_ref().map(DraftSync::flush));
        let step = self
            .step_sync
            .with_value(|sync| sync.as_ref().map(|s| s.sync.flush()));
        if let Some(text) = text {
            text.await;
        }
        if let Some(title) = title {
            title.await;
        }
        if let Some(step) = step {
            step.await;
        }
    }

    async fn close(self) {
        if self.closing.get_value() {
            return;
        }
        self.closing.set_value(true);
        self.flush().await;
        // Nothing to show if hiding fails; the window is still here.
        let _ = api::details_dismiss().await;
        self.closing.set_value(false);
    }

    /// Fetches the note. `fresh` replaces the text fields; otherwise they
    /// are replaced only when the user has nothing unsaved in them.
    async fn load(self, id: String, fresh: bool) {
        let request = self.request.get_value().wrapping_add(1);
        self.request.set_value(request);
        let answer = api::note_detail_get(&id).await;
        if self.request.get_value() != request || !self.shows(&id) {
            return;
        }
        match answer {
            Ok(detail) => {
                if fresh {
                    self.errors.set(Vec::new());
                }
                let was_text = self.stored_text.get_value();
                let was_title = self.stored_title.get_value();
                self.text.update(|text| {
                    *text = field_after_load(text, &was_text, &detail.details, fresh)
                });
                self.title.update(|title| {
                    *title = field_after_load(title, &was_title, &detail.note.content, fresh);
                });
                self.stored_text.set_value(detail.details.clone());
                self.stored_title.set_value(detail.note.content.clone());
                self.detail.set(Some(detail));
                if fresh {
                    request_animation_frame(move || self.focus_text());
                }
            }
            // Deleted elsewhere: there is nothing left to show or save into.
            Err(error) if error.code == "not_found" => {
                self.text_sync.set_value(None);
                self.title_sync.set_value(None);
                let _ = api::details_dismiss().await;
            }
            Err(error) => self.fail(error),
        }
    }

    /// Shows a note, storing what was typed into the one before it.
    async fn open(self, target: DetailsTarget) {
        let id = target.id;
        if self.shows(&id) {
            self.load(id, false).await;
            request_animation_frame(move || self.focus_text());
            return;
        }
        self.flush().await;
        self.id.set_value(Some(id.clone()));
        self.detail.set(None);
        self.previews.set(Previews::new());
        self.notice.set(None);
        self.errors.set(Vec::new());
        self.save.set(Save::Idle);
        self.step_selected.set(None);
        self.step_editor.set(None);
        self.step_sync.set_value(None);
        self.step_creating.set_value(false);
        self.step_epoch
            .update_value(|epoch| *epoch = epoch.wrapping_add(1));
        self.creating_text.set_value(String::new());

        let note = id.clone();
        self.text_sync.set_value(Some(DraftSync::with_delay(
            SAVE_DELAY_MS,
            move |text, _| {
                let id = note.clone();
                async move {
                    api::note_details_set(&id, &text).await?;
                    if self.shows(&id) {
                        self.stored_text.set_value(text);
                        self.save.set(Save::Saved);
                    }
                    Ok(())
                }
            },
            move |error| self.fail(error),
        )));
        let note = id.clone();
        self.title_sync.set_value(Some(DraftSync::with_delay(
            SAVE_DELAY_MS,
            move |title, _| {
                let id = note.clone();
                async move {
                    let category = self.detail.with_untracked(|detail| {
                        detail.as_ref().and_then(|d| d.note.category.clone())
                    });
                    api::note_update(&id, &title, category.as_deref()).await?;
                    if self.shows(&id) {
                        self.stored_title.set_value(title);
                        self.save.set(Save::Saved);
                    }
                    Ok(())
                }
            },
            move |error| self.fail(error),
        )));
        self.load(id, true).await;
    }

    fn edit_text(self, text: String) {
        self.text.set(text.clone());
        self.save.set(Save::Saving);
        self.text_sync.with_value(|sync| {
            if let Some(sync) = sync {
                sync.schedule(text, None);
            }
        });
    }

    fn edit_title(self, title: String) {
        self.title.set(title.clone());
        // An emptied title would make the note empty: it is not written.
        if title.trim().is_empty() {
            return;
        }
        self.save.set(Save::Saving);
        self.title_sync.with_value(|sync| {
            if let Some(sync) = sync {
                sync.schedule(title, None);
            }
        });
    }

    /// Leaving the title empty puts the stored one back.
    fn restore_empty_title(self) {
        if self.title.with_untracked(|title| title.trim().is_empty()) {
            self.title.set(self.stored_title.get_value());
        }
    }

    fn toggle_done(self) {
        let Some((id, status)) = self
            .detail
            .with_untracked(|detail| detail.as_ref().map(|d| (d.note.id.clone(), d.note.status)))
        else {
            return;
        };
        let next = if status == Status::Completed {
            Status::Inbox
        } else {
            Status::Completed
        };
        spawn_local(async move {
            if let Err(error) = api::note_set_status(&id, next).await {
                self.fail(error);
            }
        });
    }

    // ---- steps ------------------------------------------------------------

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

    /// The note to attach to, or `None` with the reason on screen.
    fn note_with_room(self) -> Option<String> {
        let id = self.id.get_value()?;
        if !has_room(self.file_count()) {
            self.errors.set(vec![Message::with(
                key::DETAILS_FILES_FULL,
                "max",
                MAX_ATTACHMENTS_PER_NOTE.to_string(),
            )]);
            return None;
        }
        Some(id)
    }

    /// Says what happened to a batch of files, one line per refusal.
    fn report(self, report: AttachReport) {
        match report.added.as_slice() {
            [] => {}
            [one] => self.notice.set(Some(Message::with(
                key::DETAILS_FILES_ATTACHED,
                "name",
                one.name.clone(),
            ))),
            many => self.notice.set(Some(Message::count(
                key::DETAILS_FILES_ATTACHED_MANY,
                many.len() as u64,
            ))),
        }
        let failed = report
            .failed
            .into_iter()
            .map(|failure| {
                Message::With(
                    key::DETAILS_FILES_FAILED,
                    vec![("name", failure.name), ("reason", failure.message)],
                )
            })
            .collect();
        self.errors.set(failed);
    }

    fn add_files(self) {
        let Some(id) = self.note_with_room() else {
            return;
        };
        spawn_local(async move {
            match api::attachment_add_files(&id).await {
                Ok(Some(report)) => self.report(report),
                // The user closed the file dialog.
                Ok(None) => {}
                Err(error) => self.fail(error),
            }
        });
    }

    fn add_paths(self, paths: Vec<String>) {
        let Some(id) = self.note_with_room() else {
            return;
        };
        spawn_local(async move {
            match api::attachment_add_paths(&id, &paths).await {
                Ok(report) => self.report(report),
                Err(error) => self.fail(error),
            }
        });
    }

    /// Attaches images pasted from the clipboard. Other pastes are text for
    /// whichever field has the caret, and are left alone.
    fn paste(self, event: web_sys::ClipboardEvent, t: Translator) {
        let images = pasted_images(&event);
        if images.is_empty() || self.id.with_value(Option::is_none) {
            return;
        }
        event.prevent_default();
        let Some(id) = self.note_with_room() else {
            return;
        };
        spawn_local(async move {
            let mut failed = Vec::new();
            let mut added = 0u64;
            for image in images {
                let mime = image.type_();
                let name = if is_unnamed_paste(&image.name()) {
                    let stamp = time::file_stamp(time::now_ms());
                    let name = t.format(key::DETAILS_FILES_PASTED_NAME, &[("stamp", &stamp)]);
                    format!("{name}.{}", image_extension(&mime))
                } else {
                    image.name()
                };
                if image.size() > MAX_ATTACHMENT_BYTES as f64 {
                    let megabytes = (MAX_ATTACHMENT_BYTES / (1024 * 1024)).to_string();
                    failed.push(Message::With(
                        key::DETAILS_FILES_TOO_BIG,
                        vec![("name", name), ("size", megabytes)],
                    ));
                    continue;
                }
                let stored = match JsFuture::from(image.array_buffer()).await {
                    Ok(buffer) => {
                        let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                        api::attachment_add_bytes(&id, &name, &mime, &bytes).await
                    }
                    Err(_) => Err(CommandError {
                        code: "unknown".into(),
                        message: String::new(),
                    }),
                };
                match stored {
                    Ok(_) => added += 1,
                    Err(error) => failed.push(Message::With(
                        key::DETAILS_FILES_FAILED,
                        vec![("name", name), ("reason", crate::ipc::message(&error, t))],
                    )),
                }
            }
            match added {
                0 => {}
                1 => self
                    .notice
                    .set(Some(Message::Plain(key::DETAILS_FILES_PASTED))),
                many => self
                    .notice
                    .set(Some(Message::count(key::DETAILS_FILES_PASTED_MANY, many))),
            }
            self.errors.set(failed);
        });
    }

    fn remove(self, attachment: &Attachment) {
        let (id, name) = (attachment.id.clone(), attachment.name.clone());
        spawn_local(async move {
            match api::attachment_remove(&id).await {
                Ok(()) => {
                    self.notice.set(Some(Message::with(
                        key::DETAILS_FILES_REMOVED,
                        "name",
                        name,
                    )));
                    // The tile that had focus is gone.
                    if let Some(button) = self.add_button.get_untracked() {
                        let _ = button.focus();
                    }
                }
                Err(error) => self.fail(error),
            }
        });
    }

    fn save_copy(self, attachment: &Attachment) {
        let (id, name) = (attachment.id.clone(), attachment.name.clone());
        spawn_local(async move {
            match api::attachment_save_copy(&id).await {
                Ok(Some(_)) => self.notice.set(Some(Message::with(
                    key::DETAILS_FILES_COPY_SAVED,
                    "name",
                    name,
                ))),
                Ok(None) => {}
                Err(error) => self.fail(error),
            }
        });
    }

    fn open_file(self, attachment: &Attachment) {
        let id = attachment.id.clone();
        spawn_local(async move {
            if let Err(error) = api::attachment_open(&id).await {
                self.fail(error);
            }
        });
    }

    /// Fetches the thumbnail of every image that does not have one yet.
    fn load_previews(self, attachments: &[Attachment]) {
        for attachment in attachments {
            let known = self
                .previews
                .with_untracked(|previews| previews.contains_key(&attachment.id));
            if known || Kind::of(&attachment.mime) != Kind::Image {
                continue;
            }
            let (id, mime) = (attachment.id.clone(), attachment.mime.clone());
            self.previews.update(|previews| {
                previews.insert(id.clone(), None);
            });
            spawn_local(async move {
                // A tile without a thumbnail still names its file.
                if let Ok(bytes) = api::attachment_preview(&id).await {
                    self.previews.update(|previews| {
                        previews.insert(id, Some(data_url(&bytes, &mime)));
                    });
                }
            });
        }
    }

    fn on_key(self, event: KeyboardEvent, os: Os) {
        if event.default_prevented() {
            return;
        }
        let key = event.key();
        let press = KeyPress {
            key: &key,
            primary: has_mod(event.meta_key(), event.ctrl_key(), os),
            shift: event.shift_key(),
            alt: event.alt_key(),
            composing: event.is_composing(),
        };
        let Some(action) = key_action(press) else {
            return;
        };
        event.prevent_default();
        match action {
            KeyAction::Close => spawn_local(self.close()),
            KeyAction::ToggleDone => self.toggle_done(),
            KeyAction::AddFiles => self.add_files(),
        }
    }
}

/// The image files a paste carries.
fn pasted_images(event: &web_sys::ClipboardEvent) -> Vec<web_sys::File> {
    let Some(files) = event.clipboard_data().and_then(|data| data.files()) else {
        return Vec::new();
    };
    (0..files.length())
        .filter_map(|index| files.item(index))
        .filter(|file| file.type_().starts_with("image/"))
        .collect()
}

#[component]
pub fn DetailsWindow(os: Os) -> impl IntoView {
    let settings = use_settings();
    use_appearance(settings);
    let i18n = use_i18n(settings);
    let details = Details::new();
    let locale = Memo::new(move |_| time::locale_for(i18n.translator().language().code));

    spawn_local(async move {
        match api::details_current().await {
            Ok(Some(target)) => details.open(target).await,
            Ok(None) => {}
            Err(error) => details.fail(error),
        }
    });
    events::on::<events::DetailsOpen>(move |target| spawn_local(details.open(target)));
    events::on::<events::DetailsCloseRequested>(move |()| spawn_local(details.close()));
    events::on::<events::NotesChanged>(move |()| {
        if let Some(id) = details.id.get_value() {
            spawn_local(details.load(id, false));
        }
    });
    events::on_file_drop(move |drop| match drop {
        events::FileDrop::Over => details.dropping.set(true),
        events::FileDrop::Left => details.dropping.set(false),
        events::FileDrop::Dropped(paths) => {
            details.dropping.set(false);
            if !paths.is_empty() {
                details.add_paths(paths);
            }
        }
    });
    Effect::new(move |_| {
        details.detail.with(|detail| {
            if let Some(detail) = detail {
                details.load_previews(&detail.attachments);
            }
        });
    });

    // On the window, so keys and paste work wherever focus sits in the page.
    let keys = window_event_listener(leptos::ev::keydown, move |event| details.on_key(event, os));
    let paste = window_event_listener(leptos::ev::paste, move |event| {
        if let Ok(event) = event.dyn_into::<web_sys::ClipboardEvent>() {
            details.paste(event, i18n.translator());
        }
    });
    // The OS may hand the window focus after the page asked for it: put the
    // caret back in the description when nothing else holds focus.
    let refocus = window_event_listener(leptos::ev::focus, move |_| {
        if browser::focus_is_on_page() {
            details.focus_text();
        }
    });
    on_cleanup(move || {
        keys.remove();
        paste.remove();
        refocus.remove();
    });

    let has_note = move || details.detail.with(Option::is_some);
    let files = Memo::new(move |_| {
        details.detail.with(|detail| {
            detail
                .as_ref()
                .map(|d| d.attachments.clone())
                .unwrap_or_default()
        })
    });
    let count = move || files.with(Vec::len);
    let has_files = move || files.with(|files| !files.is_empty());
    let mod_key = mod_cap(os);
    let add_keys = format!("{mod_key}+O");
    let footer = move || {
        let t = i18n.translator();
        match (details.notice.get(), details.save.get()) {
            (Some(notice), _) => notice.text(t),
            (None, Save::Saving) => t.text(key::DETAILS_SAVE_SAVING).to_owned(),
            (None, Save::Saved) => t.text(key::DETAILS_SAVE_SAVED).to_owned(),
            (None, Save::Idle) => String::new(),
        }
    };
    let hint = move |keys: &[&str], label| {
        view! {
            <span class="details__hint">
                <Kbd keys=caps(keys) />
                " "
                {i18n.text(label)}
            </span>
        }
    };

    view! {
        <div class="details">
            <section
                class="details__panel"
                aria-label=i18n.text(key::DETAILS_TITLE)
                data-dropping=move || details.dropping.get().to_string()
            >
                <header class="details__header" data-tauri-drag-region>
                    <input
                        class="details__title"
                        aria-label=i18n.text(key::DETAILS_TITLE_FIELD)
                        use:bind_text=Signal::derive(move || details.title.get())
                        disabled=move || !has_note()
                        on:input=move |event| details.edit_title(event_target_value(&event))
                        on:blur=move |_| details.restore_empty_title()
                        on:keydown=move |event| {
                            let plain_enter = event.key() == "Enter"
                                && !event.is_composing()
                                && !has_mod(event.meta_key(), event.ctrl_key(), os);
                            if plain_enter {
                                event.prevent_default();
                                if let Some(field) = details.text_field.get_untracked() {
                                    let _ = field.focus();
                                }
                            }
                        }
                    />
                    <StatusButton details=details i18n=i18n os=os />
                    <IconButton
                        icon=IconName::Close
                        label=i18n.label(key::DETAILS_CLOSE)
                        on_click=move |()| spawn_local(details.close())
                    />
                </header>

                <Meta details=details i18n=i18n locale=locale />

                <div class="details__body">
                    <StepsSection details=details i18n=i18n os=os />
                    <textarea
                        node_ref=details.text_field
                        class="details__text selectable"
                        aria-label=i18n.text(key::DETAILS_TEXT_FIELD)
                        placeholder=i18n.text(key::DETAILS_TEXT_PLACEHOLDER)
                        use:bind_text=Signal::derive(move || details.text.get())
                        disabled=move || !has_note()
                        on:input=move |event| details.edit_text(event_target_value(&event))
                    ></textarea>

                    <section class="details__files" aria-label=i18n.text(key::DETAILS_FILES_TITLE)>
                        <h2 class="details__files-title">
                            {i18n.text(key::DETAILS_FILES_TITLE)}
                            <Show when=has_files>
                                <span class="details__count">{count}</span>
                            </Show>
                        </h2>
                        <Show when=has_files>
                            <ul class="details__grid">
                                <For each=move || files.get() key=|file| file.id.clone() let:file>
                                    <Tile details=details i18n=i18n attachment=file />
                                </For>
                            </ul>
                        </Show>
                        <div class="details__drop" data-empty=move || (count() == 0).to_string()>
                            <button
                                node_ref=details.add_button
                                type="button"
                                class="btn"
                                disabled=move || !has_note()
                                on:click=move |_| details.add_files()
                            >
                                <Icon name=IconName::Paperclip size=14 />
                                " "
                                {i18n.text(key::DETAILS_FILES_ADD)}
                            </button>
                            <span class="details__drop-hint">
                                {move || {
                                    let t = i18n.translator();
                                    if details.dropping.get() {
                                        t.text(key::DETAILS_FILES_DROP_NOW).to_owned()
                                    } else {
                                        t.format(key::DETAILS_FILES_DROP_HINT, &[("keys", &add_keys)])
                                    }
                                }}
                            </span>
                        </div>
                    </section>

                    <Show when=move || details.errors.with(|errors| !errors.is_empty())>
                        <ul class="details__errors" role="alert">
                            {move || {
                                let t = i18n.translator();
                                details
                                    .errors
                                    .get()
                                    .into_iter()
                                    .map(|error| view! { <li>{error.text(t)}</li> })
                                    .collect_view()
                            }}
                        </ul>
                    </Show>
                </div>

                <footer class="details__footer">
                    <p class="details__saved" role="status">{footer}</p>
                    <span class="details__hints" aria-hidden="true">
                        {hint(&["esc"], key::DETAILS_HINT_CLOSE)}
                        {hint(&[mod_key, "↵"], key::DETAILS_HINT_DONE)}
                        {hint(&[mod_key, "O"], key::DETAILS_HINT_ADD_FILE)}
                    </span>
                </footer>
            </section>

            <Toasts toaster=details.toaster i18n=i18n />
        </div>
    }
}

/// The note's steps: a labelled list between the title and the description,
/// then a quiet "Add a step" line. Keys work wherever focus sits in the
/// section; the window's own keys keep working around it.
#[component]
fn StepsSection(details: Details, i18n: I18n, os: Os) -> impl IntoView {
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

/// The button that says where the note is and marks it done.
#[component]
fn StatusButton(details: Details, i18n: I18n, os: Os) -> impl IntoView {
    let state = Memo::new(move |_| details.detail.with(|d| d.as_ref().map(|d| badge(&d.note))));
    let keys = format!("{}+Enter", mod_cap(os));
    move || {
        let state = state.get()?;
        let done = state == Badge::Done;
        let name = match state {
            Badge::Done => key::DETAILS_STATUS_DONE,
            Badge::Archived => key::DETAILS_STATUS_ARCHIVED,
            Badge::Later => key::DETAILS_STATUS_LATER,
            Badge::Inbox => key::DETAILS_STATUS_INBOX,
        };
        let tip = if done {
            key::DETAILS_STATUS_MARK_NOT_COMPLETED
        } else {
            key::DETAILS_STATUS_MARK_COMPLETED
        };
        let keys = keys.clone();
        Some(view! {
            <button
                type="button"
                class="details__status"
                data-done=done.to_string()
                aria-pressed=done.to_string()
                title=move || i18n.translator().format(tip, &[("keys", &keys)])
                on:click=move |_| details.toggle_done()
            >
                {done.then(|| view! { <Icon name=IconName::Check size=12 /> })}
                {i18n.text(name)}
            </button>
        })
    }
}

/// "today", "yesterday" or a date, for the line under the title.
enum When {
    Today,
    Yesterday,
    On(String),
}

fn when(ms: i64, locale: &str) -> When {
    let (day, today) = (time::day_of(ms), time::today());
    match day.days_until(today) {
        0 => When::Today,
        1 => When::Yesterday,
        _ => When::On(time::format_day(
            day,
            locale,
            DateStyle {
                weekday: Some(Width::Long),
                month: Some(Width::Long),
                day: true,
                year: day.year != today.year,
            },
        )),
    }
}

/// When the note was captured, where from, and what has happened since.
fn meta_line(note: &Note, locale: &str, t: Translator) -> String {
    let stamp = |ms: i64, today, yesterday, on| {
        let time = time::format_time(ms, locale);
        match when(ms, locale) {
            When::Today => t.format(today, &[("time", &time)]),
            When::Yesterday => t.format(yesterday, &[("time", &time)]),
            When::On(date) => t.format(on, &[("date", &date), ("time", &time)]),
        }
    };
    let mut parts = vec![stamp(
        note.created_at,
        key::DETAILS_META_CREATED_TODAY,
        key::DETAILS_META_CREATED_YESTERDAY,
        key::DETAILS_META_CREATED_ON,
    )];
    if let Some(source) = &note.source {
        parts.push(t.format(key::DETAILS_META_FROM, &[("source", source)]));
    }
    let carried = carried_days(note, time::day_of(note.created_at), time::today());
    if carried > 0 {
        parts.push(t.plural(key::DETAILS_META_CARRIED, carried, &[]));
    }
    if was_edited(note) {
        parts.push(stamp(
            note.updated_at,
            key::DETAILS_META_EDITED_TODAY,
            key::DETAILS_META_EDITED_YESTERDAY,
            key::DETAILS_META_EDITED_ON,
        ));
    }
    parts.join(" · ")
}

#[component]
fn Meta(details: Details, i18n: I18n, locale: Memo<String>) -> impl IntoView {
    move || {
        details.detail.with(|detail| {
            let note = &detail.as_ref()?.note;
            let line = meta_line(note, &locale.get(), i18n.translator());
            Some(view! { <p class="details__meta">{line}</p> })
        })
    }
}

/// One attached file: its thumbnail or type, name and size, and what can be
/// done with it.
#[component]
fn Tile(details: Details, i18n: I18n, attachment: Attachment) -> impl IntoView {
    let kind = Kind::of(&attachment.mime);
    let size = format_bytes(attachment.size);
    let label = type_label(&attachment.name, &attachment.mime);
    let file = StoredValue::new(attachment);
    let name = move || file.with_value(|file| file.name.clone());
    let preview = Memo::new(move |_| {
        let id = file.with_value(|file| file.id.clone());
        details
            .previews
            .with(|previews| previews.get(&id).cloned().flatten())
    });
    let named =
        move |key| Signal::derive(move || i18n.translator().format(key, &[("name", &name())]));
    let described = {
        let size = size.clone();
        move || {
            i18n.translator().format(
                key::DETAILS_TILE_LABEL,
                &[("name", &name()), ("size", &size)],
            )
        }
    };

    view! {
        <li class="tile" data-kind=kind.as_str()>
            <button
                type="button"
                class="tile__main"
                title=move || named(key::DETAILS_TILE_TIP).get()
                aria-label=described
                on:click=move |_| file.with_value(|file| details.open_file(file))
                on:keydown=move |event| {
                    if matches!(event.key().as_str(), "Delete" | "Backspace") {
                        event.prevent_default();
                        file.with_value(|file| details.remove(file));
                    }
                }
            >
                {move || match preview.get() {
                    Some(source) => view! { <img class="tile__thumb" src=source alt="" /> }.into_any(),
                    None => {
                        let icon = if kind == Kind::Image { IconName::Image } else { IconName::File };
                        view! {
                            <span class="tile__glyph" aria-hidden="true">
                                <Icon name=icon size=20 />
                                <span class="tile__type">{label.clone()}</span>
                            </span>
                        }
                        .into_any()
                    }
                }}
                <span class="tile__name">{name}</span>
                <span class="tile__size">{size}</span>
            </button>
            <span class="tile__actions">
                <IconButton
                    icon=IconName::Open
                    label=named(key::DETAILS_TILE_OPEN)
                    on_click=move |()| file.with_value(|file| details.open_file(file))
                />
                <IconButton
                    icon=IconName::Download
                    label=named(key::DETAILS_TILE_SAVE_COPY)
                    on_click=move |()| file.with_value(|file| details.save_copy(file))
                />
                <IconButton
                    icon=IconName::Trash
                    label=named(key::DETAILS_TILE_REMOVE)
                    danger=true
                    on_click=move |()| file.with_value(|file| details.remove(file))
                />
            </span>
        </li>
    }
}
