use leptos::prelude::*;

use super::{Icon, IconName};

/// A button that shows only an icon. `label` is its accessible name and
/// its tooltip.
#[component]
pub fn IconButton(
    icon: IconName,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] on_click: Callback<()>,
    /// Extra class names, after `icon-btn`.
    #[prop(optional)]
    class: &'static str,
    #[prop(into, optional)] disabled: Signal<bool>,
) -> impl IntoView {
    let class = if class.is_empty() {
        "icon-btn".to_owned()
    } else {
        format!("icon-btn {class}")
    };
    view! {
        <button
            type="button"
            class=class
            aria-label=move || label.get()
            title=move || label.get()
            disabled=move || disabled.get()
            on:click=move |_| on_click.run(())
        >
            <Icon name=icon />
        </button>
    }
}
