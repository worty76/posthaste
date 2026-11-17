use leptos::html;
use leptos::prelude::*;

use super::{Icon, IconName};

/// How much a button asks for attention.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Variant {
    #[default]
    Default,
    /// The one action the surface is for.
    Primary,
    /// Destroys something.
    Danger,
    /// Stays out of the way until it is wanted.
    Quiet,
}

impl Variant {
    fn class(self) -> &'static str {
        match self {
            Variant::Default => "btn",
            Variant::Primary => "btn btn--primary",
            Variant::Danger => "btn btn--danger",
            Variant::Quiet => "btn btn--quiet",
        }
    }
}

/// A button with text. `busy` shows the busy state and blocks repeat clicks.
#[component]
pub fn Button(
    #[prop(optional)] variant: Variant,
    /// Extra class names, after the button's own.
    #[prop(optional)]
    class: &'static str,
    #[prop(into, optional)] busy: Signal<bool>,
    #[prop(into, optional)] disabled: Signal<bool>,
    /// Takes focus when the dialog it is in opens.
    #[prop(optional)]
    autofocus: bool,
    #[prop(optional)] node_ref: NodeRef<html::Button>,
    #[prop(into)] on_click: Callback<()>,
    children: Children,
) -> impl IntoView {
    let class = if class.is_empty() {
        variant.class().to_owned()
    } else {
        format!("{} {class}", variant.class())
    };
    view! {
        <button
            node_ref=node_ref
            type="button"
            class=class
            aria-busy=move || busy.get().then_some("true")
            disabled=move || disabled.get() || busy.get()
            autofocus=autofocus
            on:click=move |_| on_click.run(())
        >
            {children()}
        </button>
    }
}

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
    #[prop(optional)] danger: bool,
    #[prop(into, optional)] disabled: Signal<bool>,
    /// For a button that opens something: whether it is open.
    #[prop(into, optional)]
    expanded: Option<Signal<bool>>,
) -> impl IntoView {
    let mut classes = String::from("icon-btn");
    if danger {
        classes.push_str(" icon-btn--danger");
    }
    if !class.is_empty() {
        classes.push(' ');
        classes.push_str(class);
    }
    view! {
        <button
            type="button"
            class=classes
            aria-label=move || label.get()
            title=move || label.get()
            aria-expanded=move || expanded.map(|open| open.get().to_string())
            disabled=move || disabled.get()
            on:click=move |_| on_click.run(())
        >
            <Icon name=icon />
        </button>
    }
}
