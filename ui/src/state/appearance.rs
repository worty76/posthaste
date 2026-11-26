//! Applies the active look and theme to `<html>`. This is the only code
//! that writes CSS custom properties or the `data-theme`, `data-shape` and
//! `data-font` attributes.

use std::cell::RefCell;

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_model::{Base, FontFace, ResolvedLook, Shape, TextSize};
use wasm_bindgen::prelude::*;

use super::SettingsHandle;
use crate::ipc::{api, events};
use crate::logic::theme::Scheme;

thread_local! {
    /// The properties the last look set, so the next one (or none) removes
    /// exactly those and never leaves a stale token behind.
    static APPLIED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn root() -> Option<web_sys::HtmlElement> {
    document().document_element()?.dyn_into().ok()
}

fn apply_look(look: Option<&ResolvedLook>) {
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
                let _ = root.set_attribute("data-shape", shape_name(look.shape));
                let _ = root.set_attribute("data-font", font_name(look.font));
            }
            None => {
                let _ = root.remove_attribute("data-shape");
                let _ = root.remove_attribute("data-font");
            }
        }
    });
}

/// The names the stylesheets select on.
fn shape_name(shape: Shape) -> &'static str {
    match shape {
        Shape::Sharp => "sharp",
        Shape::Soft => "soft",
        Shape::Round => "round",
    }
}

fn font_name(font: FontFace) -> &'static str {
    match font {
        FontFace::System => "system",
        FontFace::Rounded => "rounded",
        FontFace::Serif => "serif",
        FontFace::Mono => "mono",
        FontFace::BeVietnamPro => "be-vietnam-pro",
        FontFace::Nunito => "nunito",
        FontFace::Quicksand => "quicksand",
    }
}

/// What a window can ask of its appearance.
#[derive(Clone, Copy)]
pub struct Appearance {
    preview: RwSignal<Option<ResolvedLook>>,
}

impl Appearance {
    /// Wears `look` instead of the active one until given `None`. For the
    /// look editor, which shows a look before it is saved.
    pub fn preview(self, look: Option<ResolvedLook>) {
        self.preview.set(look);
    }
}

/// Keeps the window wearing the active look. There is always one: its
/// base pins light or dark, and nothing else decides. Nothing is touched
/// until the active look is known: the page paints its first frame as
/// Ink, and guessing early would flash the wrong scheme.
pub fn use_appearance(settings: SettingsHandle) -> Appearance {
    // `None` = not known yet.
    let (active, set_active) = signal(None::<ResolvedLook>);
    let preview = RwSignal::new(None::<ResolvedLook>);

    let reload = move || {
        spawn_local(async move {
            // A rejected lookup cannot happen while the app runs: the
            // store falls back to the factory default.
            if let Ok(look) = api::look_active().await {
                set_active.set(Some(look));
            }
        });
    };
    let look_id = Memo::new(move |_| {
        settings
            .settings
            .with(|s| s.as_ref().map(|s| s.look_id.clone()))
    });
    Effect::new(move |_| {
        // Read, not `track()`: a memo that is only tracked is never computed,
        // so it never hears about the settings it is made from.
        look_id.with(|_| ());
        reload();
    });
    events::on::<events::LooksChanged>(move |_| reload());

    // The look worn: a preview first, else the active one.
    let worn = Memo::new(move |_| active.get().map(|active| preview.get().unwrap_or(active)));
    let scheme = Memo::new(move |_| {
        worn.with(|worn| {
            worn.as_ref().map(|look| match look.base {
                Base::Light => Scheme::Light,
                Base::Dark => Scheme::Dark,
            })
        })
    });
    Effect::new(move |_| {
        worn.with(|worn| {
            if let Some(look) = worn {
                apply_look(Some(look));
            }
        });
    });
    Effect::new(move |_| {
        if let (Some(scheme), Some(root)) = (scheme.get(), root()) {
            let _ = root.set_attribute("data-theme", scheme.as_str());
        }
    });
    let text_size = Memo::new(move |_| {
        settings
            .settings
            .with(|s| s.as_ref().map(|s| s.text_size).unwrap_or_default())
    });
    Effect::new(move |_| {
        let Some(root) = root() else { return };
        match text_size.get() {
            TextSize::Small => {
                let _ = root.set_attribute("data-text-size", "small");
            }
            TextSize::Medium => {
                let _ = root.remove_attribute("data-text-size");
            }
            TextSize::Large => {
                let _ = root.set_attribute("data-text-size", "large");
            }
        }
    });
    let macos_glass = Memo::new(move |_| {
        settings
            .settings
            .with(|s| s.as_ref().is_some_and(|s| s.macos_glass))
    });
    Effect::new(move |_| {
        let Some(root) = root() else { return };
        if macos_glass.get() {
            let _ = root.set_attribute("data-glass", "true");
        } else {
            let _ = root.remove_attribute("data-glass");
        }
    });
    Appearance { preview }
}
