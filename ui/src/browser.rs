//! Small things asked of the page itself: the clipboard, where focus is,
//! what a key press landed on. Kept in one place so windows read as intent.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Copies text. Falls back to a hidden field where the clipboard API is
/// blocked. `false` when neither worked.
pub async fn copy_text(text: &str) -> bool {
    let written = JsFuture::from(window().navigator().clipboard().write_text(text)).await;
    written.is_ok() || copy_through_field(text)
}

fn copy_through_field(text: &str) -> bool {
    let document = document();
    let Some(body) = document.body() else {
        return false;
    };
    let Some(field) = document
        .create_element("textarea")
        .ok()
        .and_then(|field| field.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
    else {
        return false;
    };
    field.set_value(text);
    let _ = field.set_attribute("readonly", "");
    let _ = field.set_attribute("class", "sr-only");
    if body.append_child(&field).is_err() {
        return false;
    }
    field.select();
    let copied = document
        .dyn_ref::<web_sys::HtmlDocument>()
        .is_some_and(|document| document.exec_command("copy").unwrap_or(false));
    field.remove();
    copied
}

/// Puts the caret at the end of a text field.
pub fn focus_end(field: &web_sys::HtmlTextAreaElement) {
    let _ = field.focus();
    let end = field.value().encode_utf16().count() as u32;
    let _ = field.set_selection_range(end, end);
}

fn target_element(event: &web_sys::Event) -> Option<web_sys::Element> {
    event.target()?.dyn_into().ok()
}

/// Whether the event happened in something the user types into.
pub fn is_typing(event: &web_sys::Event) -> bool {
    target_element(event).is_some_and(|target| {
        let editable = target
            .dyn_ref::<web_sys::HtmlElement>()
            .is_some_and(web_sys::HtmlElement::is_content_editable);
        editable || matches!(target.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
    })
}

/// Whether the event happened on, or inside, an element matching `selector`.
pub fn is_within(event: &web_sys::Event, selector: &str) -> bool {
    target_element(event).is_some_and(|target| target.closest(selector).ok().flatten().is_some())
}

/// Whether a modal dialog is open. Its keys are its own.
pub fn dialog_is_open() -> bool {
    document()
        .query_selector("dialog[open]")
        .ok()
        .flatten()
        .is_some()
}

/// Whether nothing on the page holds focus.
pub fn focus_is_on_page() -> bool {
    document()
        .active_element()
        .is_none_or(|active| active.tag_name().eq_ignore_ascii_case("body"))
}

/// Keeps a text field showing `text`: `<input use:bind_text=signal />`.
///
/// The field is written only when it shows something else. Writing back
/// what was just typed would end an input method's composition, which is
/// how Vietnamese and many other languages are typed.
pub fn bind_text(field: web_sys::Element, text: Signal<String>) {
    Effect::new(move |_| {
        let text = text.get();
        if let Some(input) = field.dyn_ref::<web_sys::HtmlInputElement>() {
            if input.value() != text {
                input.set_value(&text);
            }
        } else if let Some(area) = field.dyn_ref::<web_sys::HtmlTextAreaElement>() {
            if area.value() != text {
                area.set_value(&text);
            }
        }
    });
}
