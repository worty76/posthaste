//! The Settings section. Every change is applied as it is made; there is
//! no Save button. A change the shell refuses leaves the old value in
//! place and says why.

mod ai;
mod appearance;
mod data;
mod devices;
mod look_editor;
mod shortcut_recorder;

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::{key, Key};
use posthaste_model::{
    AppInfo, DraftRetention, EnterBehavior, Os, Settings, ShortcutStatus, TextSize, WindowSize,
    DEFAULT_CAPTURE_SHORTCUT,
};

use crate::components::{Button, PositionPicker, Segmented, Switch, Toaster, Variant};
use crate::logic::shortcut::{default_inbox_shortcut, default_today_shortcut};
use crate::state::{Appearance, I18n, SettingsHandle, Updates};
use shortcut_recorder::ShortcutRecorder;

pub(super) fn is_recording_shortcut() -> bool {
    shortcut_recorder::is_recording()
}

/// What every part of Settings needs: the language, the settings and a
/// place to report a change that failed.
#[derive(Clone, Copy)]
struct Context {
    i18n: I18n,
    settings: SettingsHandle,
    toaster: Toaster,
}

impl Context {
    /// Applies a change now. On failure the old value stays.
    fn set(self, change: impl FnOnce(&mut Settings) + 'static) {
        spawn_local(async move {
            if let Err(error) = self.settings.update(change).await {
                self.toaster.fail(error);
            }
        });
    }

    /// One setting, kept current.
    fn read<T>(self, read: impl Fn(&Settings) -> T + Send + Sync + 'static) -> Signal<T>
    where
        T: Send + Sync + 'static,
    {
        Signal::derive(move || {
            self.settings.settings.with(|settings| match settings {
                Some(settings) => read(settings),
                // Settings is only shown once they have loaded.
                None => read(&Settings::default()),
            })
        })
    }
}

/// One setting: what it is, a line on what it does, and its control.
#[component]
fn Row(
    i18n: I18n,
    label: Key,
    #[prop(optional)] help: Option<Key>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="row">
            <div class="row__text">
                <div class="row__label">{i18n.text(label)}</div>
                {help.map(|help| view! { <div class="row__help">{i18n.text(help)}</div> })}
            </div>
            <div class="row__control">{children()}</div>
        </div>
    }
}

/// The six Settings tabs, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    General,
    Keyboard,
    Appearance,
    Ai,
    Devices,
    Data,
}

impl Tab {
    const ALL: [Tab; 6] = [
        Tab::General,
        Tab::Keyboard,
        Tab::Appearance,
        Tab::Ai,
        Tab::Devices,
        Tab::Data,
    ];

    fn label(self) -> Key {
        match self {
            Tab::General => key::SETTINGS_GENERAL_HEADING,
            Tab::Keyboard => key::SETTINGS_KEYBOARD_HEADING,
            Tab::Appearance => key::SETTINGS_TABS_APPEARANCE,
            Tab::Ai => key::SETTINGS_TABS_AI,
            Tab::Devices => key::SETTINGS_DEVICES_HEADING,
            Tab::Data => key::SETTINGS_DATA_HEADING,
        }
    }

    fn step(self, delta: isize) -> Tab {
        let len = Self::ALL.len() as isize;
        let idx = Self::ALL.iter().position(|&t| t == self).unwrap_or(0) as isize;
        Self::ALL[(idx + delta).rem_euclid(len) as usize]
    }
}

#[component]
pub(super) fn SettingsView(
    info: AppInfo,
    i18n: I18n,
    settings: SettingsHandle,
    appearance: Appearance,
    updates: Updates,
    toaster: Toaster,
) -> impl IntoView {
    let context = Context {
        i18n,
        settings,
        toaster,
    };
    let os = info.os;
    let tab = RwSignal::new(Tab::General);
    let version = StoredValue::new(info.version);
    let minimized_help = if info.tray_available {
        key::SETTINGS_GENERAL_MINIMIZED_HELP
    } else {
        key::SETTINGS_GENERAL_MINIMIZED_HELP_NO_TRAY
    };
    let position_help = if info.session == "wayland" {
        key::SETTINGS_CAPTURE_POSITION_HELP_WAYLAND
    } else {
        key::SETTINGS_CAPTURE_POSITION_HELP
    };
    let shortcut = move |read: fn(&posthaste_model::ShortcutReport) -> &ShortcutStatus| {
        Signal::derive(move || {
            settings.shortcuts.with(|report| {
                report
                    .as_ref()
                    .map(|report| read(report).clone())
                    .unwrap_or_default()
            })
        })
    };
    let limitation = Signal::derive(move || {
        settings
            .shortcuts
            .with(|report| report.as_ref().and_then(|report| report.limitation.clone()))
    });
    let language = context.read(|s| s.language.clone());

    view! {
        <div class="settings">
            <div
                class="settings__tabs"
                role="tablist"
                aria-label=i18n.text(key::SETTINGS_TABS_LABEL)
            >
                {Tab::ALL
                    .into_iter()
                    .map(|item| {
                        let active = move || tab.get() == item;
                        view! {
                            <button
                                type="button"
                                role="tab"
                                class="settings__tab"
                                aria-selected=move || active().to_string()
                                tabindex=move || if active() { "0" } else { "-1" }
                                on:click=move |_| tab.set(item)
                                on:keydown=move |event| {
                                    let next = match event.key().as_str() {
                                        "ArrowRight" => Some(item.step(1)),
                                        "ArrowLeft" => Some(item.step(-1)),
                                        "Home" => Some(Tab::ALL[0]),
                                        "End" => Some(Tab::ALL[Tab::ALL.len() - 1]),
                                        _ => None,
                                    };
                                    if let Some(next) = next {
                                        event.prevent_default();
                                        tab.set(next);
                                    }
                                }
                            >
                                {i18n.text(item.label())}
                            </button>
                        }
                    })
                    .collect_view()}
            </div>

            <div class="settings__panel" role="tabpanel">
                {move || match tab.get() {
                    Tab::General => view! {
                        <section class="settings__section" aria-labelledby="s-general">
                            <h2 class="settings__heading" id="s-general">{i18n.text(key::SETTINGS_GENERAL_HEADING)}</h2>
                            <Row i18n=i18n label=key::SETTINGS_GENERAL_LANGUAGE help=key::SETTINGS_GENERAL_LANGUAGE_HELP>
                                <select
                                    class="field select"
                                    aria-label=i18n.text(key::SETTINGS_GENERAL_LANGUAGE)
                                    on:change=move |event| {
                                        let code = event_target_value(&event);
                                        context.set(move |s| s.language = code);
                                    }
                                >
                                    // Each language under its own name, so it can be
                                    // found by someone who cannot read the current one.
                                    {posthaste_i18n::languages()
                                        .iter()
                                        .map(|choice| view! {
                                            <option
                                                value=choice.code
                                                lang=choice.code
                                                selected=move || i18n.translator().language().code == choice.code
                                            >
                                                {choice.name}
                                            </option>
                                        })
                                        .collect_view()}
                                </select>
                            </Row>
                            <Row i18n=i18n label=key::SETTINGS_GENERAL_LAUNCH help=key::SETTINGS_GENERAL_LAUNCH_HELP>
                                <Switch
                                    label=i18n.label(key::SETTINGS_GENERAL_LAUNCH)
                                    checked=context.read(|s| s.launch_at_login)
                                    on_change=move |on| context.set(move |s| s.launch_at_login = on)
                                />
                            </Row>
                            <Row i18n=i18n label=key::SETTINGS_GENERAL_MINIMIZED help=minimized_help>
                                <Switch
                                    label=i18n.label(key::SETTINGS_GENERAL_MINIMIZED)
                                    checked=context.read(|s| s.start_minimized)
                                    on_change=move |on| context.set(move |s| s.start_minimized = on)
                                />
                            </Row>
                        </section>
                        <UpdatesSection context=context updates=updates version=version.get_value() />
                    }
                    .into_any(),
                    Tab::Keyboard => view! {
                        <section class="settings__section" aria-labelledby="s-keyboard">
                            <h2 class="settings__heading" id="s-keyboard">{i18n.text(key::SETTINGS_KEYBOARD_HEADING)}</h2>
                            <ShortcutRow
                                context=context
                                os=os
                                label=key::SETTINGS_KEYBOARD_CAPTURE
                                help=key::SETTINGS_KEYBOARD_CAPTURE_HELP
                                value=context.read(|s| s.capture_shortcut.clone())
                                default=DEFAULT_CAPTURE_SHORTCUT
                                status=shortcut(|report| &report.capture)
                                limitation=limitation
                                on_change=move |keys: String| context.set(move |s| s.capture_shortcut = keys)
                            />
                            <ShortcutRow
                                context=context
                                os=os
                                label=key::SETTINGS_KEYBOARD_INBOX
                                help=key::SETTINGS_KEYBOARD_INBOX_HELP
                                value=context.read(|s| s.inbox_shortcut.clone())
                                default=default_inbox_shortcut(os)
                                status=shortcut(|report| &report.inbox)
                                limitation=limitation
                                on_change=move |keys: String| context.set(move |s| s.inbox_shortcut = keys)
                            />
                            <ShortcutRow
                                context=context
                                os=os
                                label=key::SETTINGS_KEYBOARD_TODAY
                                help=key::SETTINGS_KEYBOARD_TODAY_HELP
                                value=context.read(|s| s.today_shortcut.clone())
                                default=default_today_shortcut(os)
                                status=shortcut(|report| &report.today)
                                limitation=limitation
                                on_change=move |keys: String| context.set(move |s| s.today_shortcut = keys)
                            />
                        </section>

                        <section class="settings__section" aria-labelledby="s-capture">
                            <h2 class="settings__heading" id="s-capture">{i18n.text(key::SETTINGS_CAPTURE_HEADING)}</h2>
                            <Row i18n=i18n label=key::SETTINGS_CAPTURE_POSITION help=position_help>
                                <div class="positions">
                                    <div class="positions__item">
                                        <PositionPicker
                                            i18n=i18n
                                            label=key::SETTINGS_CAPTURE_POSITION_CAPTURE
                                            value=context.read(|s| s.capture_position)
                                            on_change=move |spot| context.set(move |s| s.capture_position = spot)
                                        />
                                        <span class="positions__caption" aria-hidden="true">
                                            {i18n.text(key::SETTINGS_CAPTURE_CAPTION_CAPTURE)}
                                        </span>
                                    </div>
                                    <div class="positions__item">
                                        <PositionPicker
                                            i18n=i18n
                                            label=key::SETTINGS_CAPTURE_POSITION_TODAY
                                            value=context.read(|s| s.today_position)
                                            on_change=move |spot| context.set(move |s| s.today_position = spot)
                                        />
                                        <span class="positions__caption" aria-hidden="true">
                                            {i18n.text(key::SETTINGS_CAPTURE_CAPTION_TODAY)}
                                        </span>
                                    </div>
                                </div>
                            </Row>
                            <Row i18n=i18n label=key::SETTINGS_CAPTURE_SIZE help=key::SETTINGS_CAPTURE_SIZE_HELP>
                                <Segmented
                                    i18n=i18n
                                    label=key::SETTINGS_CAPTURE_SIZE
                                    value=context.read(|s| s.window_size)
                                    options=vec![
                                        (WindowSize::Small, key::SETTINGS_CAPTURE_SIZE_SMALL),
                                        (WindowSize::Medium, key::SETTINGS_CAPTURE_SIZE_MEDIUM),
                                        (WindowSize::Large, key::SETTINGS_CAPTURE_SIZE_LARGE),
                                    ]
                                    on_change=move |size| context.set(move |s| s.window_size = size)
                                />
                            </Row>
                            <Row
                                i18n=i18n
                                label=key::SETTINGS_CAPTURE_TEXT_SIZE
                                help=key::SETTINGS_CAPTURE_TEXT_SIZE_HELP
                            >
                                <Segmented
                                    i18n=i18n
                                    label=key::SETTINGS_CAPTURE_TEXT_SIZE
                                    value=context.read(|s| s.text_size)
                                    options=vec![
                                        (TextSize::Small, key::SETTINGS_CAPTURE_TEXT_SIZE_SMALL),
                                        (TextSize::Medium, key::SETTINGS_CAPTURE_TEXT_SIZE_MEDIUM),
                                        (TextSize::Large, key::SETTINGS_CAPTURE_TEXT_SIZE_LARGE),
                                    ]
                                    on_change=move |size| context.set(move |s| s.text_size = size)
                                />
                            </Row>
                            <Row i18n=i18n label=key::SETTINGS_CAPTURE_ENTER help=key::SETTINGS_CAPTURE_ENTER_HELP>
                                <Segmented
                                    i18n=i18n
                                    label=key::SETTINGS_CAPTURE_ENTER
                                    value=context.read(|s| s.enter_behavior)
                                    options=vec![
                                        (EnterBehavior::Save, key::SETTINGS_CAPTURE_ENTER_SAVE),
                                        (EnterBehavior::Newline, key::SETTINGS_CAPTURE_ENTER_NEWLINE),
                                    ]
                                    on_change=move |enter| context.set(move |s| s.enter_behavior = enter)
                                />
                            </Row>
                            <Row
                                i18n=i18n
                                label=key::SETTINGS_CAPTURE_CLOSE_AFTER_SAVE
                                help=key::SETTINGS_CAPTURE_CLOSE_AFTER_SAVE_HELP
                            >
                                <Switch
                                    label=i18n.label(key::SETTINGS_CAPTURE_CLOSE_AFTER_SAVE)
                                    checked=context.read(|s| s.hide_after_save)
                                    on_change=move |on| context.set(move |s| s.hide_after_save = on)
                                />
                            </Row>
                            <Row
                                i18n=i18n
                                label=key::SETTINGS_CAPTURE_CLOSE_ON_BLUR
                                help=key::SETTINGS_CAPTURE_CLOSE_ON_BLUR_HELP
                            >
                                <Switch
                                    label=i18n.label(key::SETTINGS_CAPTURE_CLOSE_ON_BLUR)
                                    checked=context.read(|s| s.hide_on_blur)
                                    on_change=move |on| context.set(move |s| s.hide_on_blur = on)
                                />
                            </Row>
                            <Row i18n=i18n label=key::SETTINGS_CAPTURE_DRAFTS help=key::SETTINGS_CAPTURE_DRAFTS_HELP>
                                <Segmented
                                    i18n=i18n
                                    label=key::SETTINGS_CAPTURE_DRAFTS
                                    value=context.read(|s| s.draft_retention)
                                    options=vec![
                                        (DraftRetention::Day, key::SETTINGS_CAPTURE_DRAFTS_DAY),
                                        (DraftRetention::Week, key::SETTINGS_CAPTURE_DRAFTS_WEEK),
                                        (DraftRetention::Forever, key::SETTINGS_CAPTURE_DRAFTS_FOREVER),
                                    ]
                                    on_change=move |keep| context.set(move |s| s.draft_retention = keep)
                                />
                            </Row>
                        </section>
                    }
                    .into_any(),
                    Tab::Appearance => view! {
                        <section class="settings__section" aria-labelledby="s-appearance">
                            <h2 class="settings__heading" id="s-appearance">{i18n.text(key::SETTINGS_TABS_APPEARANCE)}</h2>
                            <appearance::AppearanceSection context=context appearance=appearance />
                        </section>
                    }
                    .into_any(),
                    Tab::Ai => view! {
                        <ai::AiSection context=context language=language />
                    }
                    .into_any(),
                    Tab::Devices => view! {
                        <devices::DevicesSection context=context language=language />
                    }
                    .into_any(),
                    Tab::Data => view! {
                        <data::DataSection context=context />
                    }
                    .into_any(),
                }}
            </div>
        </div>
    }
}

/// One global shortcut: the recorder, a way back to the default, and
/// whether the system accepted it.
#[component]
fn ShortcutRow(
    context: Context,
    os: Os,
    label: Key,
    help: Key,
    value: Signal<String>,
    default: &'static str,
    status: Signal<ShortcutStatus>,
    limitation: Signal<Option<String>>,
    #[prop(into)] on_change: Callback<String>,
) -> impl IntoView {
    let i18n = context.i18n;
    view! {
        <div class="row row--stack">
            <div class="row">
                <div class="row__text">
                    <div class="row__label">{i18n.text(label)}</div>
                    <div class="row__help">{i18n.text(help)}</div>
                </div>
                <div class="row__control">
                    <ShortcutRecorder i18n=i18n os=os label=label value=value on_change=on_change />
                    <Button
                        variant=Variant::Quiet
                        disabled=Signal::derive(move || value.with(|value| value == default))
                        on_click=move |()| on_change.run(default.to_owned())
                    >
                        {i18n.text(key::SETTINGS_KEYBOARD_RESTORE_DEFAULT)}
                    </Button>
                </div>
            </div>
            {move || {
                let t = i18n.translator();
                let status = status.get();
                if status.accelerator.is_empty() {
                    view! { <p class="status">{t.text(key::SETTINGS_KEYBOARD_NONE)}</p> }.into_any()
                } else if !status.registered {
                    let reason = status.error.unwrap_or_default();
                    view! {
                        <p class="status selectable" data-state="error" role="alert">
                            {t.format(key::SETTINGS_KEYBOARD_INACTIVE, &[("reason", &reason)])}
                        </p>
                    }
                    .into_any()
                } else if let Some(reason) = limitation.get() {
                    view! {
                        <p class="status selectable" data-state="warn">
                            {t.format(key::SETTINGS_KEYBOARD_LIMITED, &[("reason", &reason)])}
                        </p>
                    }
                    .into_any()
                } else {
                    view! {
                        <p class="status" data-state="ok">{t.text(key::SETTINGS_KEYBOARD_ACTIVE)}</p>
                    }
                    .into_any()
                }
            }}
        </div>
    }
}

/// Which version this is, and the way to a newer one.
#[component]
fn UpdatesSection(context: Context, updates: Updates, version: String) -> impl IntoView {
    let i18n = context.i18n;
    let checking = RwSignal::new(false);
    // Set after a manual check that found nothing newer.
    let is_latest = RwSignal::new(false);
    let check = move |()| {
        checking.set(true);
        spawn_local(async move {
            match updates.check().await {
                Ok(found) => is_latest.set(found.is_none()),
                Err(error) => context.toaster.fail(error),
            }
            checking.set(false);
        });
    };
    let install = move |()| {
        spawn_local(async move {
            if let Err(error) = updates.install().await {
                context.toaster.fail(error);
            }
        });
    };
    let found_version = move || {
        updates
            .found
            .with(|found| found.as_ref().map(|u| u.version.clone()))
    };

    view! {
        <section class="settings__section" aria-labelledby="s-updates">
            <h2 class="settings__heading" id="s-updates">{i18n.text(key::SETTINGS_UPDATES_HEADING)}</h2>
            <div class="row row--stack">
                <div class="row">
                    <div class="row__text">
                        <div class="row__label">
                            {move || i18n.translator().format(
                                key::SETTINGS_UPDATES_VERSION,
                                &[("version", &version)],
                            )}
                        </div>
                        <div class="row__help">{i18n.text(key::SETTINGS_UPDATES_HELP)}</div>
                    </div>
                    <div class="row__control">
                        {move || match found_version() {
                            Some(version) => view! {
                                <Button busy=updates.installing on_click=install>
                                    {move || i18n.translator().format(
                                        key::SETTINGS_UPDATES_UPDATE_TO,
                                        &[("version", &version)],
                                    )}
                                </Button>
                            }
                            .into_any(),
                            None => view! {
                                <Button busy=checking on_click=check>
                                    {i18n.text(key::SETTINGS_UPDATES_CHECK)}
                                </Button>
                            }
                            .into_any(),
                        }}
                    </div>
                </div>
                {move || found_version().map(|version| view! {
                    <p class="status" data-state="ok">
                        {move || i18n.translator().format(
                            key::SETTINGS_UPDATES_READY,
                            &[("version", &version)],
                        )}
                    </p>
                })}
                <Show when=move || is_latest.get() && found_version().is_none()>
                    <p class="status" data-state="ok">{i18n.text(key::SETTINGS_UPDATES_LATEST)}</p>
                </Show>
            </div>
        </section>
    }
}
