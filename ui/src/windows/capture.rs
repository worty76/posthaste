//! The capture window: one field, Enter saves, Esc closes.
//!
//! The rule everything here serves: text is never lost. A note counts as
//! saved only after the shell says the database committed it. Until then
//! the text stays on screen and in the draft.

use leptos::ev::KeyboardEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Key, Translator};
use posthaste_model::{CommandError, Os, Project};

use crate::browser::bind_text;
use crate::components::{Kbd, ProjectDot, ProjectPicker};
use crate::draft_sync::DraftSync;
use crate::ipc::{self, api, events};
use crate::logic::capture::{hints, key_action, HintWeight, KeyAction, KeyPress, KeyRules};
use crate::logic::project_picker::{PickerMode, PickerRow};
use crate::logic::shortcut::{format_accelerator, has_mod, mod_cap};
use crate::state::{use_appearance, use_i18n, use_settings, I18n};

/// The line under the field. Messages are kept as keys, not text, so the
/// line follows a language change like everything else.
#[derive(Debug, Clone, PartialEq)]
enum Status {
    Idle,
    /// Text from last time is back in the field.
    Restored,
    /// Something is in flight: saving or polishing.
    Busy(Key),
    Done(Key),
    /// The action needs text and there is none.
    Empty(Key),
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq)]
enum Failure {
    /// The shell's own explanation.
    Plain(CommandError),
    /// "Not saved: <the shell's explanation>".
    Because(Key, CommandError),
}

impl Status {
    /// The `data-kind` the stylesheet colours the line by.
    fn kind(&self) -> &'static str {
        match self {
            Status::Idle => "idle",
            Status::Restored => "restored",
            Status::Busy(_) => "saving",
            Status::Done(_) => "saved",
            Status::Empty(_) => "empty",
            Status::Failed(_) => "error",
        }
    }

    fn text(&self, t: Translator) -> String {
        match self {
            Status::Idle => String::new(),
            Status::Restored => t.text(key::CAPTURE_STATUS_DRAFT_RESTORED).to_owned(),
            Status::Busy(k) | Status::Done(k) | Status::Empty(k) => t.text(*k).to_owned(),
            Status::Failed(Failure::Plain(e)) => ipc::message(e, t),
            Status::Failed(Failure::Because(k, e)) => {
                t.format(*k, &[("reason", &ipc::message(e, t))])
            }
        }
    }
}

/// Everything the window's actions read and write. `Copy`, so each handler
/// takes it by value.
#[derive(Clone, Copy)]
struct Capture {
    content: RwSignal<String>,
    category: RwSignal<String>,
    projects: RwSignal<Vec<Project>>,
    picker_open: RwSignal<bool>,
    status: RwSignal<Status>,
    /// The app the user came from, when the shell knows it.
    source: RwSignal<Option<String>>,
    polishing: RwSignal<bool>,
    shake: RwSignal<bool>,
    /// A save is in flight; a second Enter must not file the note twice.
    saving: StoredValue<bool>,
    /// The user has typed since the window was last shown.
    touched: StoredValue<bool>,
    drafts: StoredValue<DraftSync, LocalStorage>,
    input: NodeRef<html::Textarea>,
}

impl Capture {
    fn new() -> Self {
        let status = RwSignal::new(Status::Idle);
        let drafts = DraftSync::new(
            |content, category| async move { api::draft_set(&content, category.as_deref()).await },
            move |error| {
                status.set(Status::Failed(Failure::Because(
                    key::CAPTURE_STATUS_DRAFT_NOT_SAVED,
                    error,
                )));
            },
        );
        Capture {
            content: RwSignal::new(String::new()),
            category: RwSignal::new(String::new()),
            projects: RwSignal::new(Vec::new()),
            picker_open: RwSignal::new(false),
            status,
            source: RwSignal::new(None),
            polishing: RwSignal::new(false),
            shake: RwSignal::new(false),
            saving: StoredValue::new(false),
            touched: StoredValue::new(false),
            drafts: StoredValue::new_local(drafts),
            input: NodeRef::new(),
        }
    }

    fn drafts(self) -> DraftSync {
        self.drafts.with_value(DraftSync::clone)
    }

    /// The category as the shell wants it: `None` when the field is blank.
    fn category_or_none(self, trim: bool) -> Option<String> {
        let category = self.category.get_untracked();
        let category = if trim {
            category.trim().to_owned()
        } else {
            category
        };
        (!category.is_empty()).then_some(category)
    }

    fn focus(self) {
        if let Some(input) = self.input.get_untracked() {
            let _ = input.focus();
        }
    }

    fn clear(self) {
        self.touched.set_value(false);
        self.content.set(String::new());
        self.category.set(String::new());
    }

    fn load_projects(self) {
        spawn_local(async move {
            if let Ok(projects) = api::projects_list(true).await {
                self.projects.set(projects);
            }
        });
    }

    fn project_color(self) -> Option<String> {
        let current = self.category.get();
        if current.is_empty() {
            return None;
        }
        self.projects
            .with(|list| list.iter().find(|p| p.name == current)?.color.clone())
    }

    fn open_picker(self) {
        self.picker_open.set(true);
        self.load_projects();
    }

    fn close_picker(self) {
        self.picker_open.set(false);
        self.focus();
    }

    fn pick_project(self, row: PickerRow) {
        let next = match row {
            PickerRow::NoProject => String::new(),
            PickerRow::Project { name, .. }
            | PickerRow::Create(name)
            | PickerRow::Unarchive(name) => name,
        };
        self.picker_open.set(false);
        self.edit(self.content.get_untracked(), next);
        self.focus();
    }

    /// Every change to either field goes through here, so it is drafted.
    fn edit(self, content: String, category: String) {
        self.touched.set_value(true);
        self.drafts().schedule(
            content.clone(),
            (!category.is_empty()).then(|| category.clone()),
        );
        self.content.set(content);
        self.category.set(category);
        // A message about the last action no longer describes the field.
        if !matches!(self.status.get_untracked(), Status::Idle | Status::Busy(_)) {
            self.status.set(Status::Idle);
        }
    }

    fn refuse_empty(self, message: Key) {
        self.status.set(Status::Empty(message));
        self.shake.set(true);
    }

    async fn restore_draft(self) {
        match api::draft_get().await {
            // Never overwrite what the user has started typing meanwhile.
            Ok(Some(draft)) if !self.touched.get_value() => {
                self.content.set(draft.content);
                self.category.set(draft.category.unwrap_or_default());
                self.status.set(Status::Restored);
            }
            Ok(_) => {}
            Err(error) => self.status.set(Status::Failed(Failure::Plain(error))),
        }
    }

    async fn save(self, hide: bool) {
        if self.saving.get_value() {
            return;
        }
        let content = self.content.get_untracked();
        if content.trim().is_empty() {
            self.refuse_empty(key::CAPTURE_STATUS_NOTHING_TO_SAVE);
            return;
        }
        self.saving.set_value(true);
        self.status.set(Status::Busy(key::CAPTURE_STATUS_SAVING));
        // Make sure no draft write can land after the note is saved.
        self.drafts().cancel().await;
        let category = self.category_or_none(true);
        match api::capture_save(&content, category.as_deref(), hide).await {
            Ok(()) => {
                // Only now, with the note committed, is the text let go.
                if hide {
                    self.clear();
                    self.status.set(Status::Idle);
                } else {
                    // Staying open for another note: clear only the text so
                    // batch-capturing into the same project needs no re-picking.
                    self.touched.set_value(false);
                    self.content.set(String::new());
                    self.status.set(Status::Done(key::CAPTURE_STATUS_SAVED));
                    self.focus();
                }
            }
            Err(error) => {
                self.status.set(Status::Failed(Failure::Because(
                    key::CAPTURE_STATUS_NOT_SAVED,
                    error,
                )));
                // Keep the text safe as a draft while the user decides.
                self.drafts()
                    .schedule(content, self.category_or_none(false));
            }
        }
        self.saving.set_value(false);
    }

    async fn polish(self) {
        // A second press while a polish is in flight is not a mistake to flag.
        if self.polishing.get_untracked() {
            return;
        }
        let content = self.content.get_untracked();
        if content.trim().is_empty() {
            self.refuse_empty(key::CAPTURE_STATUS_NOTHING_TO_POLISH);
            return;
        }
        self.polishing.set(true);
        self.status.set(Status::Busy(key::CAPTURE_STATUS_POLISHING));
        match api::ai_polish(&content).await {
            Ok(polished) => {
                self.edit(polished, self.category.get_untracked());
                self.status.set(Status::Done(key::CAPTURE_STATUS_POLISHED));
            }
            Err(error) => self.status.set(Status::Failed(Failure::Plain(error))),
        }
        self.polishing.set(false);
    }

    async fn dismiss(self) {
        self.drafts().flush().await;
        // Nothing useful to show if hiding fails; the draft is stored.
        let _ = api::capture_dismiss().await;
    }

    async fn discard(self) {
        self.drafts().cancel().await;
        self.clear();
        self.status.set(Status::Idle);
        let _ = api::draft_clear().await;
        self.focus();
    }

    /// The shell just showed the window: take focus and catch up.
    fn shown(self, source: Option<String>) {
        self.touched.set_value(false);
        self.picker_open.set(false);
        self.source.set(source);
        if let Some(input) = self.input.get_untracked() {
            let _ = input.focus();
            let end = input.value().encode_utf16().count() as u32;
            let _ = input.set_selection_range(end, end);
        }
        if self.content.get_untracked().trim().is_empty() {
            self.status.set(Status::Idle);
            spawn_local(self.restore_draft());
        } else if !matches!(self.status.get_untracked(), Status::Failed(_)) {
            // Text left from last time: say so, and offer the way out.
            self.status.set(Status::Restored);
        }
        self.load_projects();
    }
}

#[component]
pub fn CaptureWindow(os: Os) -> impl IntoView {
    let settings = use_settings();
    use_appearance(settings);
    let i18n = use_i18n(settings);
    let capture = Capture::new();

    spawn_local(capture.restore_draft());
    capture.load_projects();
    Effect::new(move |_| {
        // Runs once the field exists.
        if capture.input.get().is_some() {
            capture.focus();
        }
    });
    events::on::<events::CaptureShown>(move |shown| capture.shown(shown.source));
    // If the window is hidden by anything other than our own keys (focus
    // loss, the shortcut again, the OS), make sure the text is on disk.
    events::on::<events::CaptureHidden>(move |()| drop(capture.drafts().flush()));
    let blur = window_event_listener(leptos::ev::blur, move |_| drop(capture.drafts().flush()));
    on_cleanup(move || blur.remove());

    let rules = Memo::new(move |_| {
        settings
            .settings
            .with(|s| KeyRules::from_settings(s.as_ref()))
    });
    let ai_on = Memo::new(move |_| {
        settings.settings.with(|s| {
            s.as_ref()
                .is_some_and(|s| !s.ai_provider.is_empty() && s.ai_provider != "none")
        })
    });
    let accelerator = Memo::new(move |_| {
        settings.shortcuts.with(|s| match s {
            Some(report) if report.capture.registered => {
                format_accelerator(&report.capture.accelerator, os)
            }
            _ => Vec::new(),
        })
    });

    let on_keydown = move |event: KeyboardEvent| {
        if capture.picker_open.get_untracked() {
            return;
        }
        let key = event.key();
        let press = KeyPress {
            key: &key,
            shift: event.shift_key(),
            primary: has_mod(event.meta_key(), event.ctrl_key(), os),
            composing: event.is_composing(),
        };
        let Some(action) = key_action(press, rules.get_untracked()) else {
            return;
        };
        event.prevent_default();
        match action {
            KeyAction::Dismiss => spawn_local(capture.dismiss()),
            KeyAction::Polish => spawn_local(capture.polish()),
            KeyAction::PickProject => capture.open_picker(),
            KeyAction::Save { hide } => spawn_local(capture.save(hide)),
        }
    };

    view! {
        <div class="capture" on:keydown=on_keydown>
            <section
                class="capture__panel"
                aria-label=i18n.text(key::CAPTURE_TITLE)
                data-shake=move || capture.shake.get().to_string()
                on:animationend=move |_| capture.shake.set(false)
            >
                <header class="capture__header" data-tauri-drag-region>
                    <span class="capture__mark" aria-hidden="true"></span>
                    <span class="capture__title">{i18n.text(key::CAPTURE_TITLE)}</span>
                    {move || {
                        capture.source.get().map(|app| {
                            let text = i18n.translator().format(key::CAPTURE_FROM_SOURCE, &[("app", &app)]);
                            view! { <span class="capture__source">{text}</span> }
                        })
                    }}
                    <span class="capture__spacer"></span>
                    <Show when=move || !accelerator.get().is_empty()>
                        <Kbd keys=accelerator />
                    </Show>
                </header>

                <textarea
                    node_ref=capture.input
                    class="capture__input"
                    aria-label=i18n.text(key::CAPTURE_NOTE_LABEL)
                    placeholder=i18n.text(key::CAPTURE_PLACEHOLDER)
                    use:bind_text=Signal::derive(move || capture.content.get())
                    on:input=move |event| {
                        capture.edit(event_target_value(&event), capture.category.get_untracked())
                    }
                    // Typing during a polish would be overwritten by the result.
                    readonly=move || capture.polishing.get()
                    aria-busy=move || capture.polishing.get().to_string()
                    autofocus
                    spellcheck="true"
                ></textarea>

                <Show when=move || capture.picker_open.get()>
                    <div class="capture__picker">
                        <ProjectPicker
                            i18n=i18n
                            mode=PickerMode::SetNote
                            projects=Signal::derive(move || capture.projects.get())
                            on_pick=move |row| capture.pick_project(row)
                            on_close=move |()| capture.close_picker()
                        />
                    </div>
                </Show>

                <StatusBar capture=capture i18n=i18n os=os rules=rules ai_on=ai_on />
            </section>
        </div>
    }
}

/// The footer: category, what just happened, and the keys that do things.
#[component]
fn StatusBar(
    capture: Capture,
    i18n: I18n,
    os: Os,
    rules: Memo<KeyRules>,
    ai_on: Memo<bool>,
) -> impl IntoView {
    let status_text = move || capture.status.with(|s| s.text(i18n.translator()));
    let is = move |kind: &'static str| capture.status.with(|s| s.kind() == kind);

    view! {
        <footer class="capture__footer" data-status=move || (!is("idle")).to_string()>
            <button
                type="button"
                class="capture__category"
                aria-label=i18n.text(key::CAPTURE_CATEGORY_LABEL)
                aria-haspopup="dialog"
                aria-expanded=move || capture.picker_open.get().to_string()
                aria-keyshortcuts=if os == Os::Macos { "Meta+P" } else { "Control+P" }
                title=move || {
                    format!(
                        "{} ({}+P)",
                        i18n.translator().text(key::CAPTURE_CATEGORY_LABEL),
                        mod_cap(os)
                    )
                }
                on:click=move |_| {
                    if capture.picker_open.get_untracked() {
                        capture.close_picker();
                    } else {
                        capture.open_picker();
                    }
                }
            >
                {move || {
                    let chosen = capture.category.get();
                    if chosen.is_empty() {
                        view! {
                            <span class="capture__category-placeholder">
                                {i18n.text(key::CAPTURE_CATEGORY_PLACEHOLDER)}
                            </span>
                        }
                        .into_any()
                    } else {
                        view! {
                            {capture.project_color().map(|c| view! { <ProjectDot color=c /> })}
                            <span class="capture__category-name">{chosen}</span>
                        }
                        .into_any()
                    }
                }}
            </button>

            <span
                class="capture__status"
                data-kind=move || capture.status.with(Status::kind)
                role=move || if is("error") { "alert" } else { "status" }
                title=move || Some(status_text()).filter(|text| !text.is_empty())
            >
                {status_text}
                <Show when=move || is("restored")>
                    <button
                        type="button"
                        class="capture__link"
                        on:click=move |_| spawn_local(capture.discard())
                    >
                        {i18n.text(key::CAPTURE_DISCARD)}
                    </button>
                </Show>
            </span>

            <Show when=move || ai_on.get()>
                // Reads like the hints beside it: the key first, then the verb.
                <button
                    type="button"
                    class="capture__hint capture__polish"
                    on:click=move |_| spawn_local(capture.polish())
                    disabled=move || capture.polishing.get()
                    aria-label=i18n.text(key::CAPTURE_POLISH_LABEL)
                    aria-keyshortcuts=if os == Os::Macos { "Meta+I" } else { "Control+I" }
                >
                    <Kbd keys=vec![mod_cap(os).to_owned(), "I".to_owned()] />
                    {i18n.text(key::CAPTURE_HINT_POLISH)}
                </button>
            </Show>

            <span class="capture__hints" aria-hidden="true">
                {move || {
                    hints(rules.get(), os)
                        .into_iter()
                        .map(|hint| {
                            let class = match hint.weight {
                                HintWeight::Core => "capture__hint",
                                HintWeight::Extra => "capture__hint capture__hint--extra",
                                HintWeight::Spare => "capture__hint capture__hint--spare",
                            };
                            let keys: Vec<String> = hint.keys.iter().map(|k| (*k).to_owned()).collect();
                            view! {
                                <span class=class>
                                    <Kbd keys=keys />
                                    {i18n.text(hint.label)}
                                </span>
                            }
                        })
                        .collect_view()
                }}
            </span>
        </footer>
    }
}
