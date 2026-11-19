//! The colour form: one row per colour, a hex field and the system's
//! colour picker beside it.

use leptos::prelude::*;
use posthaste_i18n::{key, Key};
use posthaste_model::ThemeEdit;

use crate::browser::bind_text;
use crate::logic::look::picker_value;
use crate::state::I18n;

/// One of the four colours a look sets.
#[derive(Clone, Copy)]
pub(super) enum Color {
    Background,
    Surface,
    Text,
    Accent,
}

impl Color {
    pub(super) const ALL: [Color; 4] = [
        Color::Background,
        Color::Surface,
        Color::Text,
        Color::Accent,
    ];

    fn name(self) -> Key {
        match self {
            Color::Background => key::LOOKS_EDITOR_BACKGROUND,
            Color::Surface => key::LOOKS_EDITOR_SURFACE,
            Color::Text => key::LOOKS_EDITOR_TEXT,
            Color::Accent => key::LOOKS_EDITOR_ACCENT,
        }
    }
}

/// One colour: a hex field and the system's colour picker beside it.
#[component]
pub(super) fn ColorRow(i18n: I18n, theme: RwSignal<ThemeEdit>, color: Color) -> impl IntoView {
    let value = Signal::derive(move || {
        theme.with(|t| match color {
            Color::Background => t.colors.bg.clone(),
            Color::Surface => t.colors.surface.clone(),
            Color::Text => t.colors.text.clone(),
            Color::Accent => t.colors.accent.clone(),
        })
    });
    let set = move |event| {
        let value = event_target_value(&event);
        theme.update(|t| match color {
            Color::Background => t.colors.bg = value.clone(),
            Color::Surface => t.colors.surface = value.clone(),
            Color::Text => t.colors.text = value.clone(),
            Color::Accent => t.colors.accent = value,
        });
    };
    let picker_name = move || {
        let t = i18n.translator();
        t.format(key::LOOKS_EDITOR_PICKER, &[("name", t.text(color.name()))])
    };
    view! {
        <div class="look-editor__color">
            <input
                class="field"
                aria-label=i18n.text(color.name())
                spellcheck="false"
                use:bind_text=value
                on:input=set
            />
            <input
                type="color"
                aria-label=picker_name
                prop:value=move || picker_value(&value.get())
                on:input=set
            />
        </div>
    }
}
