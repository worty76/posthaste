use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Translator};
use posthaste_model::UpdateInfo;

use crate::components::Button;
use crate::ipc::api;

/// Shown in place of the main window when the database could not be
/// opened. The settings live in that database, so this is always English.
#[component]
pub fn Fatal(code: String, message: String) -> impl IntoView {
    let t = Translator::default();
    let is_newer_schema = code == "db_newer_schema";
    let checking = RwSignal::new(false);
    let installing = RwSignal::new(false);
    let found = RwSignal::new(None::<UpdateInfo>);
    let checked_none = RwSignal::new(false);
    let update_error = RwSignal::new(None::<String>);

    let check = move || {
        checking.set(true);
        checked_none.set(false);
        update_error.set(None);
        spawn_local(async move {
            match api::update_check().await {
                Ok(Some(info)) => found.set(Some(info)),
                Ok(None) => checked_none.set(true),
                Err(error) => update_error.set(Some(error.message)),
            }
            checking.set(false);
        });
    };

    let install = move |()| {
        installing.set(true);
        update_error.set(None);
        spawn_local(async move {
            if let Err(error) = api::update_install().await {
                update_error.set(Some(error.message));
            }
            installing.set(false);
        });
    };

    if is_newer_schema {
        check();
    }

    view! {
        <div class="fatal" role="alert">
            <h1 class="fatal__title">{t.text(key::INBOX_FATAL_TITLE)}</h1>
            <p class="fatal__text">{message}</p>
            {if is_newer_schema {
                view! {
                    <p class="fatal__text">{t.text(key::INBOX_FATAL_NEWER_SCHEMA_TEXT)}</p>
                    <div class="row__control">
                        {move || match found.get() {
                            Some(info) => view! {
                                <Button busy=installing on_click=install>
                                    {t.format(key::INBOX_FATAL_UPDATE_TO, &[("version", &info.version)])}
                                </Button>
                            }.into_any(),
                            None => view! {
                                <Button busy=checking on_click=move |()| check()>
                                    {t.text(key::INBOX_FATAL_CHECK_UPDATES)}
                                </Button>
                            }.into_any(),
                        }}
                    </div>
                    <Show when=move || checked_none.get() && found.with(|f| f.is_none())>
                        <p class="fatal__text">{t.text(key::INBOX_FATAL_NO_UPDATE)}</p>
                    </Show>
                    {move || update_error.get().map(|err| view! {
                        <p class="status selectable" data-state="error" role="alert">{err}</p>
                    })}
                }.into_any()
            } else {
                view! {
                    <p class="fatal__text">{t.text(key::INBOX_FATAL_TEXT)}</p>
                }.into_any()
            }}
        </div>
    }
}
