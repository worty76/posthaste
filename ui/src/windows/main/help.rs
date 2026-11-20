use leptos::prelude::*;
use posthaste_i18n::{key, Key};

use crate::components::{caps, Button, Dialog, Kbd};
use crate::state::I18n;

/// The list of keys, opened with `?`.
#[component]
pub(super) fn ShortcutHelp(
    i18n: I18n,
    open: RwSignal<bool>,
    mod_key: &'static str,
) -> impl IntoView {
    let rows: [(Key, &[&str]); 16] = [
        (key::INBOX_HELP_MOVE, &["↑", "↓"]),
        (key::INBOX_HELP_EDIT, &["E"]),
        (key::INBOX_HELP_DETAILS, &["O"]),
        (key::INBOX_HELP_COMPLETE, &["X"]),
        (key::INBOX_HELP_ARCHIVE, &["A"]),
        (key::INBOX_HELP_RESTORE, &["R"]),
        (key::INBOX_HELP_LATER, &["L"]),
        (key::INBOX_HELP_COPY, &["C"]),
        (key::INBOX_HELP_DELETE, &["Del"]),
        (key::INBOX_HELP_SEARCH, &["/"]),
        (key::INBOX_HELP_SECTIONS, &["1", "2", "3", "4", "5"]),
        (key::INBOX_HELP_TRASH_RESTORE, &["R", "↵"]),
        (key::INBOX_HELP_TRASH_DELETE, &["Del"]),
        (key::INBOX_HELP_NEW_NOTE, &["N"]),
        (key::INBOX_HELP_SIDEBAR, &[mod_key, "B"]),
        (key::INBOX_HELP_SETTINGS, &[mod_key, ","]),
    ];
    let rows: Vec<(Key, Vec<String>)> = rows
        .into_iter()
        .map(|(label, keys)| (label, caps(keys)))
        .collect();
    let close = move |()| open.set(false);
    view! {
        <Dialog
            open=open
            title=i18n.label(key::INBOX_HELP_TITLE)
            on_close=close
            actions=Box::new(move || {
                view! { <Button on_click=close>{i18n.text(key::COMMON_CLOSE)}</Button> }.into_any()
            })
        >
            <dl class="shortcuts">
                {rows
                    .into_iter()
                    .map(|(label, keys)| view! {
                        <div>
                            <dt>{i18n.text(label)}</dt>
                            <dd>
                                <Kbd keys=keys />
                            </dd>
                        </div>
                    })
                    .collect_view()}
            </dl>
        </Dialog>
    }
}
