use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Key};
use posthaste_model::{Base, CommandError, FontFace, LookDoc, LookSummary, Shape};

use crate::browser::bind_text;
use crate::components::{Button, Dialog, Segmented, Variant};
use crate::ipc::{self, api};
use crate::logic::look::{picker_value, MAX_NAME_CHARS};
use crate::state::{Appearance, I18n};

/// How long typing pauses before the draft goes to the shell for a preview.
const PREVIEW_PAUSE: Duration = Duration::from_millis(120);

/// One of the four colours a look sets.
#[derive(Clone, Copy)]
enum Color {
    Background,
    Surface,
    Text,
    Accent,
}

impl Color {
    const ALL: [Color; 4] = [
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

    fn of(self, doc: &LookDoc) -> &String {
        match self {
            Color::Background => &doc.colors.bg,
            Color::Surface => &doc.colors.surface,
            Color::Text => &doc.colors.text,
            Color::Accent => &doc.colors.accent,
        }
    }

    fn set(self, doc: &mut LookDoc, value: String) {
        match self {
            Color::Background => doc.colors.bg = value,
            Color::Surface => doc.colors.surface = value,
            Color::Text => doc.colors.text = value,
            Color::Accent => doc.colors.accent = value,
        }
    }
}

/// Makes or changes a look. Every pause in typing sends the draft to the
/// shell, which is the only judge of whether a look is valid: its answer is
/// either a live preview or the reason Save is off.
#[component]
pub(super) fn LookEditor(
    i18n: I18n,
    appearance: Appearance,
    initial: LookDoc,
    /// The look being changed, or `None` for a new one.
    editing_id: Option<String>,
    #[prop(into)] on_close: Callback<()>,
    #[prop(into)] on_saved: Callback<LookSummary>,
) -> impl IntoView {
    let doc = RwSignal::new(initial);
    let error = RwSignal::new(None::<CommandError>);
    let busy = RwSignal::new(false);
    let open = RwSignal::new(true);
    // Counts drafts, so only the newest one is checked and shown.
    let draft = StoredValue::new(0u32);
    let title = if editing_id.is_some() {
        key::LOOKS_EDITOR_EDIT
    } else {
        key::LOOKS_EDITOR_NEW
    };
    let editing_id = StoredValue::new(editing_id);

    Effect::new(move |_| {
        let current = doc.get();
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
                    match answer {
                        Ok(look) => {
                            error.set(None);
                            appearance.preview(Some(look));
                        }
                        Err(refused) => {
                            error.set(Some(refused));
                            appearance.preview(None);
                        }
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
            on_close.run(());
        }
    };
    let save = move |()| {
        busy.set(true);
        spawn_local(async move {
            let id = editing_id.get_value();
            match api::look_save(&doc.get_untracked(), id.as_deref()).await {
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
        error.with(Option::is_none) && doc.with(|doc| !doc.name.trim().is_empty()) && !busy.get()
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
                <input
                    class="field"
                    aria-label=i18n.text(key::LOOKS_EDITOR_NAME)
                    maxlength=MAX_NAME_CHARS.to_string()
                    use:bind_text=Signal::derive(move || doc.with(|doc| doc.name.clone()))
                    on:input=move |event| doc.update(|doc| doc.name = event_target_value(&event))
                />
                <Segmented
                    i18n=i18n
                    label=key::LOOKS_EDITOR_BASE
                    value=Signal::derive(move || doc.with(|doc| doc.base))
                    options=vec![(Base::Light, key::LOOKS_LIGHT), (Base::Dark, key::LOOKS_DARK)]
                    on_change=move |base| doc.update(|doc| doc.base = base)
                />
                <div class="look-editor__colors">
                    {Color::ALL
                        .into_iter()
                        .map(|color| view! { <ColorRow i18n=i18n doc=doc color=color /> })
                        .collect_view()}
                </div>
                <Segmented
                    i18n=i18n
                    label=key::LOOKS_EDITOR_SHAPE
                    value=Signal::derive(move || doc.with(|doc| doc.shape))
                    options=vec![
                        (Shape::Sharp, key::LOOKS_EDITOR_SHARP),
                        (Shape::Soft, key::LOOKS_EDITOR_SOFT),
                        (Shape::Round, key::LOOKS_EDITOR_ROUND),
                    ]
                    on_change=move |shape| doc.update(|doc| doc.shape = shape)
                />
                <Segmented
                    i18n=i18n
                    label=key::LOOKS_EDITOR_FONT
                    value=Signal::derive(move || doc.with(|doc| doc.font))
                    options=vec![
                        (FontFace::System, key::LOOKS_EDITOR_FONT_SYSTEM),
                        (FontFace::Rounded, key::LOOKS_EDITOR_FONT_ROUNDED),
                        (FontFace::Serif, key::LOOKS_EDITOR_FONT_SERIF),
                        (FontFace::Mono, key::LOOKS_EDITOR_FONT_MONO),
                    ]
                    on_change=move |font| doc.update(|doc| doc.font = font)
                />
                {move || match error.get() {
                    Some(refused) => view! {
                        <p class="dialog__alert" role="alert">
                            {ipc::message(&refused, i18n.translator())}
                        </p>
                    }
                    .into_any(),
                    None => view! {
                        <p class="dialog__status" role="status">{i18n.text(key::LOOKS_EDITOR_OK)}</p>
                    }
                    .into_any(),
                }}
            </div>
        </Dialog>
    }
}

/// One colour: a hex field and the system's colour picker beside it.
#[component]
fn ColorRow(i18n: I18n, doc: RwSignal<LookDoc>, color: Color) -> impl IntoView {
    let value = Signal::derive(move || doc.with(|doc| color.of(doc).clone()));
    let set = move |event| doc.update(|doc| color.set(doc, event_target_value(&event)));
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
