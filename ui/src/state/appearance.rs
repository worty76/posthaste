//! Applies the active look and theme to `<html>`. This is the only code
//! that writes CSS custom properties or the `data-theme`, `data-shape` and
//! `data-font` attributes.

use std::cell::RefCell;

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_model::Theme;
use wasm_bindgen::prelude::*;

use super::SettingsHandle;
use crate::ipc::{api, api::ActiveLook, events};
use crate::logic::theme::{resolve_theme, Scheme};

thread_local! {
    /// The properties the last look set, so the next one (or none) removes
    /// exactly those and never leaves a stale token behind.
    static APPLIED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn root() -> Option<web_sys::HtmlElement> {
    document().document_element()?.dyn_into().ok()
}

fn apply_look(look: Option<&ActiveLook>) {
    let Some(root) = root() else { return };
    let style = root.style();
    APPLIED.with_borrow_mut(|applied| {
        for name in applied.drain(..) {
            let _ = style.remove_property(&name);
        }
        match look {
            Some(look) => {
                for (name, value) in &look.tokens {
                    let _ = style.set_property(name, value);
                    applied.push(name.clone());
                }
                let _ = root.set_attribute("data-shape", &look.shape);
                let _ = root.set_attribute("data-font", &look.font);
            }
            None => {
                let _ = root.remove_attribute("data-shape");
                let _ = root.remove_attribute("data-font");
            }
        }
    });
}

/// Whether the OS prefers dark, kept current while the window lives.
fn use_system_dark() -> ReadSignal<bool> {
    let media = window()
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .flatten();
    let (dark, set_dark) = signal(media.as_ref().is_some_and(|m| m.matches()));
    if let Some(media) = media {
        let watched = media.clone();
        let on_change = Closure::<dyn FnMut()>::new(move || set_dark.set(watched.matches()));
        let _ =
            media.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref());
        // The window never outlives its page, so the listener stays for good.
        on_change.forget();
    }
    dark
}

/// Keeps the window wearing the active look. While a look is active its
/// base pins light or dark; with Plain (no look) the theme setting decides.
/// Nothing is touched until the active look is known: the page paints its
/// first frame as Ink, and clearing that early would flash the OS theme.
pub fn use_appearance(settings: SettingsHandle) {
    // `None` = not known yet; `Some(None)` = Plain.
    let (active, set_active) = signal(None::<Option<ActiveLook>>);

    let reload = move || {
        spawn_local(async move {
            // A rejected lookup (corrupt row, unknown id) is just "no look".
            set_active.set(Some(api::look_active().await.ok().flatten()));
        });
    };
    let look_id = Memo::new(move |_| {
        settings
            .settings
            .with(|s| s.as_ref().map(|s| s.look_id.clone()))
    });
    Effect::new(move |_| {
        look_id.track();
        reload();
    });
    events::on::<events::LooksChanged>(move |_| reload());

    let theme = Memo::new(move |_| {
        settings
            .settings
            .with(|s| s.as_ref().map_or(Theme::System, |s| s.theme))
    });
    let system_dark = use_system_dark();
    Effect::new(move |_| {
        let Some(look) = active.get() else { return };
        apply_look(look.as_ref());
        let scheme = match look.as_ref().map(|l| l.base.as_str()) {
            Some("light") => Scheme::Light,
            Some(_) => Scheme::Dark,
            None => resolve_theme(theme.get(), system_dark.get()),
        };
        if let Some(root) = root() {
            let _ = root.set_attribute("data-theme", scheme.as_str());
        }
    });
}
