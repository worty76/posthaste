use std::time::Duration;

use leptos::prelude::*;
use posthaste_i18n::{key, Key};
use posthaste_model::CommandError;

use super::{Button, IconButton, IconName};
use crate::state::{I18n, Message};

const INFO: Duration = Duration::from_secs(5);
const ERROR: Duration = Duration::from_secs(9);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Info,
    Error,
}

#[derive(Clone)]
struct Toast {
    id: u32,
    message: Message,
    kind: Kind,
    /// "Undo": takes back what the toast reports.
    undo: Option<Callback<()>>,
}

/// The short messages along the bottom of the main window. A window makes
/// one and hands it to whatever needs to report something.
#[derive(Clone, Copy)]
pub struct Toaster {
    list: RwSignal<Vec<Toast>>,
    next_id: StoredValue<u32>,
}

impl Toaster {
    pub fn new() -> Self {
        Toaster {
            list: RwSignal::new(Vec::new()),
            next_id: StoredValue::new(1),
        }
    }

    pub fn info(self, message: Message) {
        self.show(message, Kind::Info, None);
    }

    /// Reports something that can be taken back.
    pub fn undoable(self, message: Message, undo: impl Fn() + Send + Sync + 'static) {
        self.show(message, Kind::Info, Some(Callback::new(move |()| undo())));
    }

    pub fn error(self, message: Message) {
        self.show(message, Kind::Error, None);
    }

    /// Reports a command that failed, in the shell's words.
    pub fn fail(self, error: CommandError) {
        self.error(Message::Failure(error));
    }

    fn show(self, message: Message, kind: Kind, undo: Option<Callback<()>>) {
        let id = self.next_id.get_value();
        self.next_id.set_value(id.wrapping_add(1));
        self.list.update(|list| {
            // At most two on screen, so they never pile over the notes.
            let keep = list.len().saturating_sub(1);
            list.drain(..keep);
            list.push(Toast {
                id,
                message,
                kind,
                undo,
            });
        });
        let stays = if kind == Kind::Error { ERROR } else { INFO };
        set_timeout(move || self.dismiss(id), stays);
    }

    fn dismiss(self, id: u32) {
        // Also reached by a timer after the window has gone.
        let _ = self
            .list
            .try_update(|list| list.retain(|toast| toast.id != id));
    }
}

impl Default for Toaster {
    fn default() -> Self {
        Toaster::new()
    }
}

#[component]
pub fn Toasts(toaster: Toaster, i18n: I18n) -> impl IntoView {
    let label = move |key: Key| Signal::derive(move || i18n.translator().text(key).to_owned());
    view! {
        <div class="toasts" role="status" aria-live="polite">
            <For each=move || toaster.list.get() key=|toast| toast.id let:toast>
                {
                    let id = toast.id;
                    let message = toast.message.clone();
                    let class = match toast.kind {
                        Kind::Info => "toast",
                        Kind::Error => "toast toast--error",
                    };
                    view! {
                        <div class=class>
                            <span class="toast__text">
                                {move || message.text(i18n.translator())}
                            </span>
                            {toast.undo.map(|undo| view! {
                                <Button on_click=move |()| {
                                    undo.run(());
                                    toaster.dismiss(id);
                                }>{i18n.text(key::INBOX_TOAST_UNDO)}</Button>
                            })}
                            <IconButton
                                icon=IconName::Close
                                label=label(key::COMMON_DISMISS)
                                on_click=move |()| toaster.dismiss(id)
                            />
                        </div>
                    }
                }
            </For>
        </div>
    }
}
