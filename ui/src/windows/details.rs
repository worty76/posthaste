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
    AttachReport, Attachment, CommandError, DetailsTarget, Note, NoteDetail, Os, Status,
    MAX_ATTACHMENTS_PER_NOTE, MAX_ATTACHMENT_BYTES,
};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::browser::{self, bind_text};
use crate::components::{caps, Icon, IconButton, IconName, Kbd};
use crate::draft_sync::DraftSync;
use crate::ipc::{api, events};
use crate::logic::attachments::{
    data_url, format_bytes, has_room, image_extension, is_unnamed_paste, type_label, Kind,
};
use crate::logic::details::{
    badge, carried_days, category_suggestions, field_after_load, key_action, was_edited, Badge,
    KeyAction, KeyPress,
};
use crate::logic::shortcut::{has_mod, mod_cap};
use crate::state::{use_appearance, use_i18n, use_settings, I18n, Message};
use crate::time::{self, DateStyle, Width};

/// How long typing pauses before the text is written.
const SAVE_DELAY_MS: u32 = 500;

/// Longest category a note can have.
const MAX_CATEGORY_CHARS: &str = "40";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Save {
    Idle,
    Saving,
    Saved,
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
    /// Editing the category in the header, and what is typed there.
    cat_editing: RwSignal<bool>,
    cat_value: RwSignal<String>,
    /// The categories in use, offered as suggestions while editing.
    cat_all: RwSignal<Vec<String>>,
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
            cat_editing: RwSignal::new(false),
            cat_value: RwSignal::new(String::new()),
            cat_all: RwSignal::new(Vec::new()),
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
        if let Some(text) = text {
            text.await;
        }
        if let Some(title) = title {
            title.await;
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
        self.cat_editing.set(false);

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

    // ---- category ---------------------------------------------------------

    fn open_category_edit(self) {
        if self.id.with_value(Option::is_none) {
            return;
        }
        let current = self
            .detail
            .with_untracked(|detail| detail.as_ref().and_then(|d| d.note.category.clone()));
        self.cat_value.set(current.unwrap_or_default());
        self.cat_editing.set(true);
        spawn_local(async move {
            // The chip still works without its suggestions.
            if let Ok(all) = api::categories_list().await {
                self.cat_all.set(all);
            }
        });
    }

    fn cancel_category_edit(self) {
        self.cat_editing.set(false);
    }

    /// Saves the typed category, or clears it when the field is empty.
    /// A failure keeps the editor open with the text in it.
    fn save_category(self) {
        let Some(id) = self.id.get_value() else {
            return;
        };
        let typed = self.cat_value.get_untracked();
        let category = (!typed.trim().is_empty()).then(|| typed.trim().to_owned());
        let title = self.title.get_untracked();
        self.save.set(Save::Saving);
        spawn_local(async move {
            match api::note_update(&id, &title, category.as_deref()).await {
                Ok(()) => {
                    self.cat_editing.set(false);
                    if self.shows(&id) {
                        self.save.set(Save::Saved);
                    }
                }
                Err(error) => self.fail(error),
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
                    <CategoryChip details=details i18n=i18n />
                    <StatusButton details=details i18n=i18n os=os />
                    <IconButton
                        icon=IconName::Close
                        label=i18n.label(key::DETAILS_CLOSE)
                        on_click=move |()| spawn_local(details.close())
                    />
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
        </div>
    }
}

/// The category as a small chip next to the status pill, or a quiet
/// "Add category" control when the note has none. Enter or a click turns it
/// into a field with suggestions; Esc puts the old value back.
#[component]
fn CategoryChip(details: Details, i18n: I18n) -> impl IntoView {
    let editing = details.cat_editing;
    let value = details.cat_value;
    let picked = RwSignal::new(0usize);
    let current = Memo::new(move |_| {
        details
            .detail
            .with(|detail| detail.as_ref().and_then(|d| d.note.category.clone()))
    });
    let suggestions = Memo::new(move |_| {
        details
            .cat_all
            .with(|all| category_suggestions(all, &value.get()))
    });
    let field = NodeRef::<html::Input>::new();
    Effect::new(move |_| {
        if editing.get() {
            if let Some(field) = field.get() {
                let _ = field.focus();
            }
        }
    });

    let choose = move |name: String| {
        details.cat_value.set(name);
        details.save_category();
    };
    let open = move |_| details.open_category_edit();

    view! {
        <Show
            when=move || editing.get()
            fallback=move || {
                match current.get() {
                    Some(name) => {
                        let named = name.clone();
                        view! {
                            <button
                                type="button"
                                class="details__category"
                                title=i18n.text(key::DETAILS_CATEGORY_EDIT)
                                on:click=open
                            >
                                {named}
                            </button>
                        }
                        .into_any()
                    }
                    None => view! {
                        <button
                            type="button"
                            class="details__category"
                            data-empty=""
                            title=i18n.text(key::DETAILS_CATEGORY_EDIT)
                            on:click=open
                        >
                            {i18n.text(key::DETAILS_CATEGORY_ADD)}
                        </button>
                    }
                    .into_any(),
                }
            }
        >
            <span class="details__cat-editor">
                <input
                    node_ref=field
                    class="field"
                    aria-label=i18n.text(key::DETAILS_CATEGORY_LABEL)
                    placeholder=i18n.text(key::DETAILS_CATEGORY_LABEL)
                    maxlength=MAX_CATEGORY_CHARS
                    spellcheck="false"
                    use:bind_text=Signal::derive(move || value.get())
                    on:input=move |event| details.cat_value.set(event_target_value(&event))
                    on:keydown=move |event| {
                        if event.is_composing() {
                            return;
                        }
                        match event.key().as_str() {
                            "ArrowDown" => {
                                event.prevent_default();
                                let options = suggestions.get_untracked();
                                if !options.is_empty() {
                                    picked.update(|at| *at = (*at + 1).min(options.len() - 1));
                                    value.set(options[picked.get_untracked()].clone());
                                }
                            }
                            "ArrowUp" => {
                                event.prevent_default();
                                let options = suggestions.get_untracked();
                                if !options.is_empty() {
                                    picked.update(|at| *at = at.saturating_sub(1));
                                    value.set(options[picked.get_untracked()].clone());
                                }
                            }
                            "Enter" => {
                                event.prevent_default();
                                event.stop_propagation();
                                details.save_category();
                            }
                            "Escape" => {
                                event.prevent_default();
                                event.stop_propagation();
                                details.cancel_category_edit();
                            }
                            _ => {}
                        }
                    }
                />
                <Show when=move || suggestions.with(|s| !s.is_empty())>
                    <ul class="details__suggestions">
                        <For each=move || suggestions.get() key=|name| name.clone() let:name>
                            {
                                let named = name.clone();
                                view! {
                                    <li>
                                        <button type="button" on:click=move |_| choose(named.clone())>
                                            {name}
                                        </button>
                                    </li>
                                }
                            }
                        </For>
                    </ul>
                </Show>
            </span>
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
