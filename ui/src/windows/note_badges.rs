use leptos::prelude::*;
use posthaste_i18n::key;
use posthaste_model::Note;

use crate::components::{Icon, IconName};
use crate::state::I18n;

/// Small marks for a note that has more inside: a description, files.
/// Renders nothing for a note with neither.
#[component]
pub fn NoteBadges(note: Note, i18n: I18n) -> impl IntoView {
    let has_details = note.has_details;
    let files = note.attachment_count;
    (has_details || files > 0).then(|| {
        let label = move || {
            let t = i18n.translator();
            let details = has_details.then(|| t.text(key::COMMON_BADGES_HAS_DETAILS).to_owned());
            let files =
                (files > 0).then(|| t.plural(key::COMMON_BADGES_FILES, u64::from(files), &[]));
            details
                .into_iter()
                .chain(files)
                .collect::<Vec<_>>()
                .join(", ")
        };
        view! {
            <span class="badges" aria-label=label title=label>
                {has_details.then(|| view! { <Icon name=IconName::Lines size=12 /> })}
                {(files > 0).then(|| view! {
                    <span class="badges__files">
                        <Icon name=IconName::Paperclip size=12 />
                        {files}
                    </span>
                })}
            </span>
        }
    })
}
