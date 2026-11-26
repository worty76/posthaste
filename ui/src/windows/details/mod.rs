//! The Note details window: a note's title, its longer description and the
//! files attached to it.
//!
//! Nothing here has a Save button. The title and the description are
//! written shortly after typing pauses, and always before the window closes
//! or moves to another note.
//!
//! - `header`: project chip, status toggle, and timestamp metadata line
//! - `steps`: ordered one-line steps inside the note
//! - `attachments`: file attachment grid, paste/drop handlers, and preview tiles

mod attachments;
mod header;
mod steps;

use std::collections::BTreeMap;

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::{CommandError, DetailsTarget, NoteDetail, Os, Project, Status};
use wasm_bindgen::JsCast;

use crate::browser::{self, bind_text};
use crate::components::{caps, Icon, IconButton, IconName, Kbd, Toaster, Toasts};
use crate::draft_sync::DraftSync;
use crate::ipc::{api, events};
use crate::logic::details::{field_after_load, key_action, KeyAction, KeyPress};
use crate::logic::shortcut::{has_mod, mod_cap};
use crate::state::{use_appearance, use_i18n, use_settings, Message};
use crate::time;

use attachments::Tile;
use header::{CategoryChip, Meta, StatusButton};
use steps::StepsSection;

/// How long typing pauses before the text is written.
pub(super) const SAVE_DELAY_MS: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Save {
    Idle,
    Saving,
    Saved,
}

/// Which step editor is open. The add line's row exists only from its
/// first non-blank keystroke, so its id starts as `None`.
#[derive(Debug, Clone)]
pub(super) struct StepEditor {
    pub(super) id: Option<String>,
    pub(super) is_add_line: bool,
}

/// A step text write in flight or pending.
#[derive(Clone)]
pub(super) struct StepSync {
    pub(super) sync: DraftSync,
}

/// Thumbnails by attachment id. `None` while one is being fetched.
pub(super) type Previews = BTreeMap<String, Option<String>>;

/// Everything the window's actions read and write. `Copy`, so each handler
/// takes it by value.
#[derive(Clone, Copy)]
pub(super) struct Details {
    pub(super) detail: RwSignal<Option<NoteDetail>>,
    pub(super) title: RwSignal<String>,
    pub(super) text: RwSignal<String>,
    pub(super) save: RwSignal<Save>,
    /// What just happened to a file. Shown in place of the save state.
    pub(super) notice: RwSignal<Option<Message>>,
    pub(super) errors: RwSignal<Vec<Message>>,
    pub(super) previews: RwSignal<Previews>,
    /// Files are being held over the window.
    pub(super) dropping: RwSignal<bool>,
    /// The selected step row, when one is.
    pub(super) step_selected: RwSignal<Option<String>>,
    /// A step whose text is being edited: the row editor or the quiet
    /// "Add a step" line, whose row exists only from the first keystroke.
    pub(super) step_editor: RwSignal<Option<StepEditor>>,
    /// What the step editor holds.
    pub(super) step_text: RwSignal<String>,
    /// The open editor's debounced write, once its row exists.
    pub(super) step_sync: StoredValue<Option<StepSync>, LocalStorage>,
    /// Project picker in the header.
    pub(super) picker_open: RwSignal<bool>,
    pub(super) projects: RwSignal<Vec<Project>>,
    /// A step row is being created (the write is in flight): keystrokes
    /// that land meanwhile wait for it instead of making another row.
    pub(super) step_creating: StoredValue<bool, LocalStorage>,
    /// Counts step editor sessions so an in-flight `steps_add` knows
    /// whether the user is still typing that same step or already pressed
    /// Enter / Esc.
    pub(super) step_epoch: StoredValue<u32>,
    /// Latest text typed for the step whose `steps_add` is in flight.
    pub(super) creating_text: StoredValue<String>,
    pub(super) toaster: Toaster,
    /// The note being shown. Answers for any other note are dropped.
    pub(super) id: StoredValue<Option<String>>,
    /// The text as last stored, to tell unsaved typing from a stale field.
    pub(super) stored_text: StoredValue<String>,
    pub(super) stored_title: StoredValue<String>,
    pub(super) text_sync: StoredValue<Option<DraftSync>, LocalStorage>,
    pub(super) title_sync: StoredValue<Option<DraftSync>, LocalStorage>,
    /// Counts loads, so a slow answer to an old one cannot replace a newer.
    pub(super) request: StoredValue<u32>,
    pub(super) closing: StoredValue<bool>,
    pub(super) text_field: NodeRef<html::Textarea>,
    pub(super) add_button: NodeRef<html::Button>,
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
            picker_open: RwSignal::new(false),
            projects: RwSignal::new(Vec::new()),
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

    pub(super) fn fail(self, error: CommandError) {
        self.errors.set(vec![Message::Failure(error)]);
    }

    pub(super) fn shows(self, id: &str) -> bool {
        self.id.with_value(|shown| shown.as_deref() == Some(id))
    }

    fn focus_text(self) {
        if let Some(field) = self.text_field.get_untracked() {
            browser::focus_end(&field);
        }
    }

    /// Writes anything typed but not yet stored.
    pub(super) async fn flush(self) {
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
        self.picker_open.set(false);
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
        self.load_projects();
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

    pub(super) fn toggle_done(self) {
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

    // ---- project ----------------------------------------------------------

    fn load_projects(self) {
        spawn_local(async move {
            if let Ok(all) = api::projects_list(true).await {
                self.projects.set(all);
            }
        });
    }

    pub(super) fn project_color(self, name: &str) -> Option<String> {
        self.projects
            .with(|list| list.iter().find(|p| p.name == name)?.color.clone())
    }

    pub(super) fn open_project_picker(self) {
        if self.id.with_value(Option::is_none) {
            return;
        }
        self.picker_open.set(true);
        self.load_projects();
    }

    pub(super) fn close_project_picker(self) {
        self.picker_open.set(false);
    }

    pub(super) fn set_project(self, project: Option<String>) {
        let Some(id) = self.id.get_value() else {
            return;
        };
        self.save.set(Save::Saving);
        spawn_local(async move {
            match api::note_set_project(&id, project.as_deref()).await {
                Ok(_) => {
                    self.picker_open.set(false);
                    if self.shows(&id) {
                        self.save.set(Save::Saved);
                    }
                }
                Err(error) => self.fail(error),
            }
        });
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
                <header class="details__header">
                    <div class="details__bar" data-tauri-drag-region>
                        <CategoryChip details=details i18n=i18n />
                        <StatusButton details=details i18n=i18n os=os />
                        <span class="details__spacer" data-tauri-drag-region></span>
                        <IconButton
                            icon=IconName::Close
                            label=i18n.label(key::DETAILS_CLOSE)
                            on_click=move |()| spawn_local(details.close())
                        />
                    </div>
                    <textarea
                        class="details__title"
                        rows="1"
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
                    ></textarea>
                </header>

                <Meta details=details i18n=i18n locale=locale />

                <div class="details__body">
                    <textarea
                        node_ref=details.text_field
                        class="details__text selectable"
                        aria-label=i18n.text(key::DETAILS_TEXT_FIELD)
                        placeholder=i18n.text(key::DETAILS_TEXT_PLACEHOLDER)
                        use:bind_text=Signal::derive(move || details.text.get())
                        disabled=move || !has_note()
                        on:input=move |event| details.edit_text(event_target_value(&event))
                    ></textarea>
                    <StepsSection details=details i18n=i18n os=os />

                    <section
                        class="details__files"
                        data-empty=move || (count() == 0).to_string()
                        aria-label=i18n.text(key::DETAILS_FILES_TITLE)
                    >
                        <Show when=has_files>
                            <h2 class="details__files-title">
                                {i18n.text(key::DETAILS_FILES_TITLE)}
                                <span class="details__count">{count}</span>
                            </h2>
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
                                class="details__add-file"
                                disabled=move || !has_note()
                                on:click=move |_| details.add_files()
                            >
                                <Icon name=IconName::Paperclip size=14 />
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
