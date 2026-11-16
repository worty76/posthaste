use leptos::prelude::*;
use posthaste_i18n::{key, Translator};

/// Shown in place of the main window when the database could not be
/// opened. The settings live in that database, so this is always English.
#[component]
pub fn Fatal(message: String) -> impl IntoView {
    let t = Translator::default();
    view! {
        <div class="fatal" role="alert">
            <h1 class="fatal__title">{t.text(key::INBOX_FATAL_TITLE)}</h1>
            <p class="fatal__text">{message}</p>
            <p class="fatal__text">{t.text(key::INBOX_FATAL_TEXT)}</p>
        </div>
    }
}
