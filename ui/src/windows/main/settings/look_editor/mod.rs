mod colors;
mod scene;
mod suggest;

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Key};
use posthaste_model::{Base, CommandError, FontFace, LookSummary, Shape, ThemeEdit};

use crate::browser::{bind_text, text_widths};
use crate::components::{Button, Dialog, Segmented, Variant};
use crate::ipc::{self, api};
use crate::logic::look::{font_seems_installed, MAX_FONT_FAMILY_CHARS, MAX_NAME_CHARS};
use crate::state::{use_scene_preview, Appearance, I18n};

use colors::{Color, ColorRow};
use scene::{ScenePanel, SceneState};
use suggest::PaletteSuggestions;

/// How long typing pauses before the draft goes to the shell for a preview.
const PREVIEW_PAUSE: Duration = Duration::from_millis(120);

/// The font presets, in the order the picker lists them.
const FONT_CHOICES: [(FontFace, Key); 7] = [
    (FontFace::System, key::LOOKS_EDITOR_FONT_SYSTEM),
    (FontFace::Rounded, key::LOOKS_EDITOR_FONT_ROUNDED),
    (FontFace::Serif, key::LOOKS_EDITOR_FONT_SERIF),
    (FontFace::Mono, key::LOOKS_EDITOR_FONT_MONO),
    (
        FontFace::BeVietnamPro,
        key::LOOKS_EDITOR_FONT_BE_VIETNAM_PRO,
    ),
    (FontFace::Nunito, key::LOOKS_EDITOR_FONT_NUNITO),
    (FontFace::Quicksand, key::LOOKS_EDITOR_FONT_QUICKSAND),
];

/// The wire value of a font preset, as the document stores it.
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

/// Makes or changes a look. Every pause in typing sends the draft to the
/// shell, which is the only judge of whether a look is valid: its answer is
/// either a live preview or the reason Save is off.
#[component]
pub(super) fn LookEditor(
    i18n: I18n,
    appearance: Appearance,
    initial: ThemeEdit,
    /// Whether Simple looks is on: the windows hide the scene, but this
    /// editor still previews it.
    #[prop(into)]
    simple_looks: Signal<bool>,
    /// The look being changed, or `None` for a new one.
    editing_id: Option<String>,
    #[prop(into)] on_close: Callback<()>,
    #[prop(into)] on_saved: Callback<LookSummary>,
) -> impl IntoView {
    let theme = RwSignal::new(initial);
    let error = RwSignal::new(None::<CommandError>);
    let contrast_x10 = RwSignal::new(None::<u16>);
    // Whether the typed font name names a font this computer has; the
    // name is only checked here, never judged by the interface.
    let font_installed = RwSignal::new(true);
    let busy = RwSignal::new(false);
    let open = RwSignal::new(true);
    // Counts drafts, so only the newest one is checked and shown.
    let draft = StoredValue::new(0u32);
    let scene_preview_runs = StoredValue::new(0u32);
    let scene = SceneState::new(editing_id.clone());
    let scene_preview = use_scene_preview();
    let title = if editing_id.is_some() {
        key::LOOKS_EDITOR_EDIT
    } else {
        key::LOOKS_EDITOR_NEW
    };
    let editing_id = StoredValue::new(editing_id);

    Effect::new(move |_| {
        let current = theme.get();
        let number = draft.get_value().wrapping_add(1);
        draft.set_value(number);
        set_timeout(
            move || {
                if draft.try_get_value() != Some(number) {
                    return;
                }
                spawn_local(async move {
                    let answer = api::look_preview(&current).await;
                    if draft.try_get_value() != Some(number) || !open.get_untracked() {
                        return;
                    }
                    font_installed.set(match current.font_family.as_deref() {
                        None => true,
                        Some(name) => match (text_widths(Some(name)), text_widths(None)) {
                            (Some(with), Some(fallback)) => font_seems_installed(with, fallback),
                            _ => true,
                        },
                    });
                    match answer {
                        Ok(look) => {
                            error.set(None);
                            contrast_x10.set((look.contrast_x10 > 0).then_some(look.contrast_x10));
                            appearance.preview(Some(look));
                        }
                        Err(refused) => {
                            error.set(Some(refused));
                            contrast_x10.set(None);
                            appearance.preview(None);
                        }
                    }
                });
            },
            PREVIEW_PAUSE,
        );
    });

    // The scene edit is checked live, the same way the colours are; its
    // answer is what the windows draw until the editor closes.
    Effect::new(move |_| {
        let edit = scene.edit.get();
        let number = scene_preview_runs.get_value().wrapping_add(1);
        scene_preview_runs.set_value(number);
        set_timeout(
            move || {
                if scene_preview_runs.try_get_value() != Some(number) || !open.get_untracked() {
                    return;
                }
                spawn_local(async move {
                    if let Ok(resolved) = api::look_scene_preview(&edit).await {
                        scene.skipped.set(resolved.skipped);
                        scene_preview.set(Some(resolved));
                    }
                });
            },
            PREVIEW_PAUSE,
        );
    });

    let close = move |()| {
        if open.get_untracked() {
            open.set(false);
            appearance.preview(None);
            scene_preview.set(None);
            on_close.run(());
        }
    };
    let save = move |()| {
        busy.set(true);
        spawn_local(async move {
            let id = editing_id.get_value();
            // A scene is sent only once it is the look's own; before the
            // load finishes, none goes and the stored scene is kept.
            let scene_edit = scene
                .loaded
                .get_untracked()
                .then(|| scene.edit.get_untracked());
            match api::look_save(&theme.get_untracked(), scene_edit.as_ref(), id.as_deref()).await {
                Ok(saved) => {
                    on_saved.run(saved);
                    close(());
                }
                Err(refused) => {
                    error.set(Some(refused));
                    busy.set(false);
                }
            }
        });
    };
    let can_save = move || {
        error.with(Option::is_none)
            && theme.with(|t| !t.name.trim().is_empty())
            && scene.loaded.get_untracked()
            && !busy.get()
    };

    view! {
        <Dialog
            open=open
            title=i18n.label(title)
            on_close=close
            actions=Box::new(move || view! {
                <Button on_click=close>{i18n.text(key::COMMON_CANCEL)}</Button>
                <Button
                    variant=Variant::Primary
                    busy=busy
                    disabled=Signal::derive(move || !can_save())
                    on_click=save
                >
                    {i18n.text(key::COMMON_SAVE)}
                </Button>
            }.into_any())
        >
            <div class="look-editor">
                <div class="look-editor__preview" aria-hidden="true">
                    <div class="look-editor__preview-head">
                        <span class="look-editor__preview-mark"></span>
                        <span class="look-editor__preview-title">
                            {move || {
                                let name = theme.with(|t| t.name.trim().to_owned());
                                if name.is_empty() {
                                    i18n.translator().text(key::CAPTURE_TITLE).to_owned()
                                } else {
                                    name
                                }
                            }}
                        </span>
                        {move || contrast_x10.get().map(|x10| view! {
                            <span class="look-editor__contrast">
                                {format!("{}.{:01}:1 · AA", x10 / 10, x10 % 10)}
                            </span>
                        })}
                    </div>
                    <div class="look-editor__preview-card">
                        <span class="look-editor__preview-row">
                            <span class="look-editor__preview-dot look-editor__preview-dot--done"></span>
                            <span class="look-editor__preview-line look-editor__preview-line--done"></span>
                        </span>
                        <span class="look-editor__preview-row">
                            <span class="look-editor__preview-dot"></span>
                            <span class="look-editor__preview-line"></span>
                            <span class="look-editor__preview-pill"></span>
                        </span>
                    </div>
                </div>
                <div class="look-editor__row">
                    <span class="look-editor__label" aria-hidden="true">
                        {i18n.text(key::LOOKS_EDITOR_NAME)}
                    </span>
                    <input
                        class="field"
                        aria-label=i18n.text(key::LOOKS_EDITOR_NAME)
                        maxlength=MAX_NAME_CHARS.to_string()
                        use:bind_text=Signal::derive(move || theme.with(|t| t.name.clone()))
                        on:input=move |event| theme.update(|t| t.name = event_target_value(&event))
                    />
                </div>
                <div class="look-editor__row">
                    <span class="look-editor__label" aria-hidden="true">
                        {i18n.text(key::LOOKS_EDITOR_BASE)}
                    </span>
                    <Segmented
                        i18n=i18n
                        label=key::LOOKS_EDITOR_BASE
                        value=Signal::derive(move || theme.with(|t| t.base))
                        options=vec![(Base::Light, key::LOOKS_LIGHT), (Base::Dark, key::LOOKS_DARK)]
                        on_change=move |base| theme.update(|t| t.base = base)
                    />
                </div>
                <PaletteSuggestions i18n=i18n theme=theme />
                <div class="look-editor__colors">
                    {Color::ALL
                        .into_iter()
                        .map(|color| view! { <ColorRow i18n=i18n theme=theme color=color /> })
                        .collect_view()}
                </div>
                <div class="look-editor__row">
                    <span class="look-editor__label" aria-hidden="true">
                        {i18n.text(key::LOOKS_EDITOR_SHAPE)}
                    </span>
                    <Segmented
                        i18n=i18n
                        label=key::LOOKS_EDITOR_SHAPE
                        value=Signal::derive(move || theme.with(|t| t.shape))
                        options=vec![
                            (Shape::Sharp, key::LOOKS_EDITOR_SHARP),
                            (Shape::Soft, key::LOOKS_EDITOR_SOFT),
                            (Shape::Round, key::LOOKS_EDITOR_ROUND),
                        ]
                        on_change=move |shape| theme.update(|t| t.shape = shape)
                    />
                </div>
                <div class="look-editor__row">
                    <span class="look-editor__label" aria-hidden="true">
                        {i18n.text(key::LOOKS_EDITOR_FONT)}
                    </span>
                    <select
                        class="field select"
                        aria-label=i18n.text(key::LOOKS_EDITOR_FONT)
                        on:change=move |event| theme.update(|t| {
                            t.font = match event_target_value(&event).as_str() {
                                "rounded" => FontFace::Rounded,
                                "serif" => FontFace::Serif,
                                "mono" => FontFace::Mono,
                                "be-vietnam-pro" => FontFace::BeVietnamPro,
                                "nunito" => FontFace::Nunito,
                                "quicksand" => FontFace::Quicksand,
                                _ => FontFace::System,
                            };
                        })
                    >
                        {FONT_CHOICES
                            .into_iter()
                            .map(|(face, label)| {
                                let value = font_name(face);
                                let selected = move || theme.with(|t| t.font) == face;
                                view! {
                                    <option
                                        value=value
                                        selected=selected
                                        class=format!("look-editor__font-option--{value}")
                                    >
                                        {i18n.text(label)}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                </div>
                <div class="look-editor__row">
                    <span class="look-editor__label" aria-hidden="true">
                        {i18n.text(key::LOOKS_EDITOR_FONT_CUSTOM)}
                    </span>
                    <div class="look-editor__stack">
                        <input
                            class="field"
                            aria-label=i18n.text(key::LOOKS_EDITOR_FONT_CUSTOM)
                            aria-describedby="look-editor-font-custom-hint"
                            maxlength=MAX_FONT_FAMILY_CHARS.to_string()
                            spellcheck="false"
                            use:bind_text=Signal::derive(move || {
                                theme.with(|t| t.font_family.clone().unwrap_or_default())
                            })
                            on:input=move |event| theme.update(|t| {
                                let name = event_target_value(&event);
                                let trimmed = name.trim().to_string();
                                t.font_family = (!trimmed.is_empty()).then_some(trimmed);
                            })
                        />
                        <p id="look-editor-font-custom-hint" class="row__help">
                            {i18n.text(key::LOOKS_EDITOR_FONT_CUSTOM_HINT)}
                        </p>
                    </div>
                </div>
                {move || (!font_installed.get()).then(|| {
                    let name = theme
                        .with_untracked(|t| t.font_family.clone().unwrap_or_default());
                    view! {
                        <p class="dialog__status" role="status">
                            {i18n.translator().format(
                                key::LOOKS_EDITOR_FONT_NOT_INSTALLED,
                                &[("name", name.as_str())],
                            )}
                        </p>
                    }
                })}
                <ScenePanel i18n=i18n state=scene />
                {move || simple_looks.get().then(|| view! {
                    <p class="dialog__status" role="status">
                        {i18n.text(key::LOOKS_EDITOR_SIMPLE_NOTE)}
                    </p>
                })}
                {move || (scene.skipped.get() > 0).then(|| view! {
                    <p class="dialog__status" role="status">
                        {i18n.text(key::LOOKS_EDITOR_NEEDS_NEWER)}
                    </p>
                })}
                {move || match error.get() {
                    Some(refused) => {
                        let can_fix = refused.code == "invalid_look";
                        view! {
                            <div class="look-editor__alert-row">
                                <p class="dialog__alert" role="alert">
                                    {ipc::message(&refused, i18n.translator())}
                                </p>
                                {can_fix.then(|| view! {
                                    <button
                                        type="button"
                                        class="look-editor__fix-btn"
                                        on:click=move |_| {
                                            let (colors, base) =
                                                theme.with_untracked(|t| (t.colors.clone(), t.base));
                                            spawn_local(async move {
                                                if let Ok(fixed) =
                                                    api::look_palette_fix_contrast(&colors, base)
                                                        .await
                                                {
                                                    theme.update(|t| t.colors = fixed);
                                                }
                                            });
                                        }
                                    >
                                        {i18n.text(key::LOOKS_EDITOR_FIX_CONTRAST)}
                                    </button>
                                })}
                            </div>
                        }
                        .into_any()
                    }
                    None => view! {
                        <p class="dialog__status" role="status">{i18n.text(key::LOOKS_EDITOR_OK)}</p>
                    }
                    .into_any(),
                }}
            </div>
        </Dialog>
    }
}
