use leptos::prelude::*;

/// A row of key caps, e.g. `<Kbd keys=caps(&["⌘", "⇧", "Space"]) />`.
#[component]
pub fn Kbd(#[prop(into)] keys: Signal<Vec<String>>) -> impl IntoView {
    view! {
        <span class="kbd">
            {move || keys.get().into_iter().map(|cap| view! { <kbd>{cap}</kbd> }).collect_view()}
        </span>
    }
}

/// Key caps written in the source, as `Kbd` takes them.
pub fn caps(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|key| (*key).to_owned()).collect()
}
