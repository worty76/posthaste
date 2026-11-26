//! Curated palette presets and interactive seed-colour palette suggestions
//! inside the Look Editor.

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::{PalettePreset, ThemeEdit};

use crate::ipc::api;
use crate::logic::look::picker_value;
use crate::state::I18n;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Curated,
    AroundColor,
}

/// Curated WCAG-AA palette pills plus an interactive seed colour picker that
/// generates six harmonious palettes around any colour the user chooses.
#[component]
pub(super) fn PaletteSuggestions(i18n: I18n, theme: RwSignal<ThemeEdit>) -> impl IntoView {
    let presets = RwSignal::new(Vec::<PalettePreset>::new());
    let source = RwSignal::new(Source::Curated);
    let seed = RwSignal::new(theme.with_untracked(|t| picker_value(&t.colors.accent)));
    let base = Memo::new(move |_| theme.with(|t| t.base));
    // Counts fetches so rapid color-wheel drags never let an older reply win.
    let request = StoredValue::new(0u32);

    Effect::new(move |_| {
        let current_base = base.get();
        let current_source = source.get();
        let current_seed = seed.get();
        let number = request.get_value().wrapping_add(1);
        request.set_value(number);
        spawn_local(async move {
            let result = match current_source {
                Source::Curated => api::look_palette_presets(current_base).await,
                Source::AroundColor => api::look_palette_around(&current_seed, current_base).await,
            };
            if request.try_get_value() == Some(number) {
                if let Ok(list) = result {
                    presets.set(list);
                }
            }
        });
    });

    let on_pick = move |event| {
        let picked = event_target_value(&event);
        seed.set(picked);
        source.set(Source::AroundColor);
    };

    view! {
        <div class="look-editor__suggest">
            <div class="look-editor__suggest-bar">
                <span class="look-editor__suggest-title">
                    {i18n.text(key::LOOKS_EDITOR_SUGGESTIONS)}
                </span>
                <div class="look-editor__suggest-actions">
                    <label
                        class="look-editor__seed-wrap"
                        data-active=move || (source.get() == Source::AroundColor).to_string()
                        on:click=move |_| source.set(Source::AroundColor)
                    >
                        <span class="look-editor__seed-label">
                            {i18n.text(key::LOOKS_EDITOR_AROUND_COLOR)}
                        </span>
                        <input
                            type="color"
                            class="look-editor__seed"
                            aria-label=i18n.text(key::LOOKS_EDITOR_AROUND_COLOR)
                            prop:value=move || seed.get()
                            on:input=on_pick
                            on:change=on_pick
                        />
                    </label>
                    <button
                        type="button"
                        class="look-editor__suggest-btn"
                        data-active=move || (source.get() == Source::Curated).to_string()
                        on:click=move |_| source.set(Source::Curated)
                    >
                        {i18n.text(key::LOOKS_EDITOR_CURATED)}
                    </button>
                </div>
            </div>

            <div class="look-editor__presets" role="group" aria-label=i18n.text(key::LOOKS_EDITOR_SUGGESTIONS)>
                <For
                    each=move || presets.get()
                    key=|preset| {
                        (
                            preset.id.clone(),
                            preset.colors.bg.clone(),
                            preset.colors.accent.clone(),
                        )
                    }
                    let:preset
                >
                    {
                        let colors = preset.colors.clone();
                        let bg = colors.bg.clone();
                        let surface = colors.surface.clone();
                        let text = colors.text.clone();
                        let accent = colors.accent.clone();
                        let colors_match = colors.clone();
                        let active = move || {
                            theme.with(|t| {
                                t.colors.bg.eq_ignore_ascii_case(&colors_match.bg)
                                    && t.colors.surface.eq_ignore_ascii_case(&colors_match.surface)
                                    && t.colors.text.eq_ignore_ascii_case(&colors_match.text)
                                    && t.colors.accent.eq_ignore_ascii_case(&colors_match.accent)
                            })
                        };
                        view! {
                            <button
                                type="button"
                                class="look-editor__preset"
                                data-active=move || active().to_string()
                                on:click=move |_| {
                                    let chosen = colors.clone();
                                    theme.update(|t| t.colors = chosen);
                                }
                            >
                                <svg
                                    class="look-editor__preset-dots"
                                    viewBox="0 0 38 12"
                                    aria-hidden="true"
                                    focusable="false"
                                >
                                    <circle cx="6" cy="6" r="5" fill=bg />
                                    <circle cx="14" cy="6" r="5" fill=surface />
                                    <circle cx="22" cy="6" r="5" fill=text />
                                    <circle cx="30" cy="6" r="5" fill=accent />
                                </svg>
                                <span class="look-editor__preset-name">{preset.name}</span>
                            </button>
                        }
                    }
                </For>
            </div>
        </div>
    }
}
