use leptos::prelude::*;

/// A row of key caps, e.g. `<Kbd keys=vec!["⌘".into(), "⇧".into(), "Space".into()] />`.
#[component]
pub fn Kbd(#[prop(into)] keys: Signal<Vec<String>>) -> impl IntoView {
    view! {
        <span class="kbd">
            {move || keys.get().into_iter().map(|cap| view! { <kbd>{cap}</kbd> }).collect_view()}
        </span>
    }
}
