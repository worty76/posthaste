use leptos::html;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

/// A modal dialog on the native `<dialog>`, which traps focus and closes on
/// Esc by itself. `on_close` runs however it was closed; the owner then
/// sets `open` to false.
#[component]
pub fn Dialog(
    #[prop(into)] open: Signal<bool>,
    #[prop(into)] title: Signal<String>,
    #[prop(into)] on_close: Callback<()>,
    #[prop(optional)] actions: Option<Children>,
    children: Children,
) -> impl IntoView {
    let dialog = NodeRef::<html::Dialog>::new();
    Effect::new(move |_| {
        let Some(dialog) = dialog.get() else { return };
        match (open.get(), dialog.open()) {
            (true, false) => {
                let _ = dialog.show_modal();
            }
            (false, true) => dialog.close(),
            _ => {}
        }
    });
    // A click on the backdrop lands on the dialog element itself.
    let on_click = move |event: leptos::ev::MouseEvent| {
        let on_backdrop = event
            .target()
            .is_some_and(|target| target.dyn_ref::<web_sys::HtmlDialogElement>().is_some());
        if on_backdrop {
            on_close.run(());
        }
    };
    view! {
        <dialog
            node_ref=dialog
            class="dialog"
            aria-label=move || title.get()
            on:close=move |_| on_close.run(())
            on:click=on_click
        >
            <div class="dialog__body">
                <h2 class="dialog__title">{move || title.get()}</h2>
                {children()}
            </div>
            {actions.map(|actions| view! { <div class="dialog__actions">{actions()}</div> })}
        </dialog>
    }
}
