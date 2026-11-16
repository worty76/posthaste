use leptos::prelude::*;

/// What a list shows when it has nothing: a title and a line on what to do.
#[component]
pub fn EmptyState(#[prop(into)] title: Signal<String>, children: Children) -> impl IntoView {
    view! {
        <div class="empty">
            <p class="empty__title">{move || title.get()}</p>
            <p class="empty__text">{children()}</p>
        </div>
    }
}
