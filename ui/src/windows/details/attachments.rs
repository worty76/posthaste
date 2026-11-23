//! File attachment actions, paste/drop handlers, and preview tiles in Note details.

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Translator};
use posthaste_model::{
    AttachReport, Attachment, CommandError, MAX_ATTACHMENTS_PER_NOTE, MAX_ATTACHMENT_BYTES,
};
use wasm_bindgen_futures::JsFuture;

use super::Details;
use crate::components::{Icon, IconButton, IconName};
use crate::ipc::api;
use crate::logic::attachments::{
    data_url, format_bytes, has_room, image_extension, is_unnamed_paste, type_label, Kind,
};
use crate::state::{I18n, Message};
use crate::time;

impl Details {
    fn file_count(self) -> usize {
        self.detail
            .with_untracked(|detail| detail.as_ref().map_or(0, |d| d.attachments.len()))
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

    pub(super) fn add_files(self) {
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

    pub(super) fn add_paths(self, paths: Vec<String>) {
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
    pub(super) fn paste(self, event: web_sys::ClipboardEvent, t: Translator) {
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

    pub(super) fn remove(self, attachment: &Attachment) {
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

    pub(super) fn save_copy(self, attachment: &Attachment) {
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

    pub(super) fn open_file(self, attachment: &Attachment) {
        let id = attachment.id.clone();
        spawn_local(async move {
            if let Err(error) = api::attachment_open(&id).await {
                self.fail(error);
            }
        });
    }

    /// Fetches the thumbnail of every image that does not have one yet.
    pub(super) fn load_previews(self, attachments: &[Attachment]) {
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

/// One attached file: its thumbnail or type, name and size, and what can be
/// done with it.
#[component]
pub(super) fn Tile(details: Details, i18n: I18n, attachment: Attachment) -> impl IntoView {
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
