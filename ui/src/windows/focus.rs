//! The floating focus pill: a compact always-on-top bar showing the one open
//! note currently in progress.

use std::time::Duration;

use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::{DetailsFrom, Note, Status};

use crate::components::{Icon, IconButton, IconName};
use crate::ipc::{api, events};
use crate::state::{use_appearance, use_i18n, use_settings};

/// Brief beat so the user sees the green checkmark before the pill hides.
const COMPLETE_BEAT: Duration = Duration::from_millis(260);

/// Everything the focus pill reads and writes. `Copy`, so handlers take it
/// by value.
#[derive(Clone, Copy)]
struct Focus {
    note: RwSignal<Option<Note>>,
    completing: RwSignal<bool>,
    overflowing: RwSignal<bool>,
    title_viewport: NodeRef<html::Div>,
    title_measure: NodeRef<html::Span>,
}

impl Focus {
    fn new() -> Self {
        Focus {
            note: RwSignal::new(None),
            completing: RwSignal::new(false),
            overflowing: RwSignal::new(false),
            title_viewport: NodeRef::new(),
            title_measure: NodeRef::new(),
        }
    }

    fn load(self) {
        spawn_local(async move {
            if let Ok(note) = api::focus_get().await {
                self.note.set(note);
            }
        });
    }

    fn complete(self, id: String) {
        if self.completing.get_untracked() {
            return;
        }
        self.completing.set(true);
        set_timeout(
            move || {
                spawn_local(async move {
                    if api::note_set_status(&id, Status::Completed).await.is_err() {
                        self.completing.set(false);
                    }
                });
            },
            COMPLETE_BEAT,
        );
    }

    fn clear(self) {
        self.completing.set(false);
        spawn_local(async move {
            let _ = api::focus_clear().await;
        });
    }

    fn open_details(self, id: String) {
        spawn_local(async move {
            let _ = api::details_open(&id, DetailsFrom::Inbox).await;
        });
    }

    /// Measures whether the title overflows its viewport so the pill only
    /// scrolls long titles.
    fn measure_overflow(self) {
        let Some(viewport) = self.title_viewport.get_untracked() else {
            return;
        };
        let Some(measure) = self.title_measure.get_untracked() else {
            return;
        };
        let overflows = measure.scroll_width() > viewport.client_width() + 2;
        if self.overflowing.get_untracked() != overflows {
            self.overflowing.set(overflows);
        }
    }
}

/// First non-empty line of a note, trimmed for the single-line pill.
fn first_line(content: &str) -> &str {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(content.trim())
}

#[component]
pub fn FocusWindow() -> impl IntoView {
    let settings = use_settings();
    use_appearance(settings);
    let i18n = use_i18n(settings);
    let focus = Focus::new();

    focus.load();
    events::on::<events::FocusChanged>(move |note| {
        focus.completing.set(false);
        focus.overflowing.set(false);
        focus.note.set(note);
    });

    // Re-measure whenever the note or text size changes.
    Effect::new(move |_| {
        focus.note.with(|_| ());
        settings.settings.with(|_| ());
        request_animation_frame(move || focus.measure_overflow());
    });

    view! {
        <div class="focus-shell">
            {move || {
                let note = focus.note.get()?;
                let id_complete = note.id.clone();
                let id_open = note.id.clone();
                let id_open_btn = note.id.clone();
                let title = first_line(&note.content).to_owned();
                let title_dup = title.clone();
                let step_total = note.step_total;
                let step_done = note.step_done;
                let step_pct = (step_total > 0).then(|| {
                    ((step_done.min(step_total) as f64 / step_total as f64) * 100.0).round() as u32
                });
                let steps_label = (step_total > 0).then(|| {
                    i18n.translator().format(
                        key::FOCUS_STEPS,
                        &[
                            ("done", &step_done.to_string()),
                            ("total", &step_total.to_string()),
                        ],
                    )
                });
                Some(view! {
                    <div
                        class="focus-pill"
                        role="region"
                        aria-label=i18n.label(key::FOCUS_LABEL)
                        data-completing=move || focus.completing.get().then_some("true")
                        data-tauri-drag-region=""
                    >
                        <button
                            type="button"
                            class="focus-pill__check"
                            aria-label=i18n.label(key::FOCUS_MARK_DONE)
                            title=i18n.label(key::FOCUS_MARK_DONE)
                            disabled=move || focus.completing.get()
                            on:click=move |_| focus.complete(id_complete.clone())
                        >
                            <Icon name=IconName::Check size=11 />
                        </button>
                        <div
                            node_ref=focus.title_viewport
                            class="focus-pill__title"
                            data-overflow=move || focus.overflowing.get().to_string()
                            data-tauri-drag-region=""
                            on:dblclick=move |_| focus.open_details(id_open.clone())
                        >
                            <span class="focus-pill__track" data-tauri-drag-region="">
                                <span
                                    node_ref=focus.title_measure
                                    class="focus-pill__copy"
                                    data-tauri-drag-region=""
                                >
                                    {title}
                                </span>
                                <Show when=move || focus.overflowing.get()>
                                    <span
                                        class="focus-pill__copy focus-pill__copy--dup"
                                        aria-hidden="true"
                                        data-tauri-drag-region=""
                                    >
                                        {title_dup.clone()}
                                    </span>
                                </Show>
                            </span>
                        </div>
                        {steps_label.map(|label| view! {
                            <span
                                class="focus-pill__steps"
                                aria-label=label
                                data-tauri-drag-region=""
                            >
                                {format!("{step_done}/{step_total}")}
                            </span>
                        })}
                        <IconButton
                            class="focus-pill__open"
                            icon=IconName::Open
                            label=i18n.label(key::INBOX_HELP_DETAILS)
                            on_click=move |_| focus.open_details(id_open_btn.clone())
                        />
                        <IconButton
                            class="focus-pill__clear"
                            icon=IconName::Close
                            label=i18n.label(key::FOCUS_CLEAR)
                            on_click=move |_| focus.clear()
                        />
                        {step_pct.map(|pct| view! {
                            <svg
                                class="focus-pill__progress"
                                viewBox="0 0 100 2"
                                preserveAspectRatio="none"
                                aria-hidden="true"
                                focusable="false"
                            >
                                <rect class="focus-pill__progress-fill" width=pct height="2" />
                            </svg>
                        })}
                    </div>
                })
            }}
        </div>
    }
}
