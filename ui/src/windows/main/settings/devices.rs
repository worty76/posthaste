//! Devices section in Settings: experimental peer-to-peer sync over iroh.

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::{SyncDevice, SyncStatus};

use super::{Context, Row};
use crate::browser::{bind_text, copy_text};
use crate::components::{Button, Dialog, Switch, Variant};
use crate::ipc::{api, events};
use crate::state::Message;
use crate::time;

#[component]
pub(super) fn DevicesSection(context: Context, language: Signal<String>) -> impl IntoView {
    let i18n = context.i18n;
    let status = RwSignal::new(SyncStatus::default());
    let toggling = RwSignal::new(false);
    let offer_code = RwSignal::new(None::<String>);
    let offering = RwSignal::new(false);
    let join_code = RwSignal::new(String::new());
    let joining = RwSignal::new(false);
    let syncing = RwSignal::new(false);
    let forget_target = RwSignal::new(None::<SyncDevice>);
    let forgetting = RwSignal::new(false);

    let device_name = RwSignal::new(context.settings.settings.with_untracked(|s| {
        s.as_ref()
            .map(|s| s.sync_device_name.clone())
            .unwrap_or_default()
    }));
    let relay_url = RwSignal::new(context.settings.settings.with_untracked(|s| {
        s.as_ref()
            .map(|s| s.sync_relay_url.clone())
            .unwrap_or_default()
    }));

    Effect::new(move || {
        context.settings.settings.with(|s| {
            if let Some(s) = s {
                if device_name.get_untracked() != s.sync_device_name {
                    device_name.set(s.sync_device_name.clone());
                }
                if relay_url.get_untracked() != s.sync_relay_url {
                    relay_url.set(s.sync_relay_url.clone());
                }
            }
        });
    });

    let refresh = move || {
        spawn_local(async move {
            if let Ok(next) = api::sync_status().await {
                status.set(next);
            }
        });
    };
    refresh();

    events::on::<events::SyncChanged>(move |()| {
        refresh();
    });

    let on_toggle = move |on: bool| {
        toggling.set(true);
        spawn_local(async move {
            match api::sync_set_enabled(on).await {
                Ok(next) => {
                    status.set(next);
                    if !on {
                        offer_code.set(None);
                    }
                }
                Err(error) => context.toaster.fail(error),
            }
            toggling.set(false);
        });
    };

    let on_pair_offer = move |()| {
        offering.set(true);
        spawn_local(async move {
            match api::sync_pair_offer().await {
                Ok(code) => offer_code.set(Some(code)),
                Err(error) => context.toaster.fail(error),
            }
            offering.set(false);
        });
    };

    let on_copy_offer = move |()| {
        if let Some(code) = offer_code.get_untracked() {
            spawn_local(async move {
                let _ = copy_text(&code).await;
                context
                    .toaster
                    .info(Message::Plain(key::SETTINGS_DEVICES_COPIED));
            });
        }
    };

    let on_pair_join = move |()| {
        let code = join_code.get_untracked();
        if code.trim().is_empty() {
            return;
        }
        joining.set(true);
        spawn_local(async move {
            match api::sync_pair_join(&code).await {
                Ok(device) => {
                    join_code.set(String::new());
                    context.toaster.info(Message::with(
                        key::SETTINGS_DEVICES_PAIRED,
                        "name",
                        device.name,
                    ));
                    refresh();
                }
                Err(error) => context.toaster.fail(error),
            }
            joining.set(false);
        });
    };

    let on_sync_now = move |()| {
        syncing.set(true);
        spawn_local(async move {
            match api::sync_now().await {
                Ok(()) => {
                    context
                        .toaster
                        .info(Message::Plain(key::SETTINGS_DEVICES_SYNCED));
                    refresh();
                }
                Err(error) => context.toaster.fail(error),
            }
            syncing.set(false);
        });
    };

    let confirm_forget = move |()| {
        let Some(target) = forget_target.get_untracked() else {
            return;
        };
        forgetting.set(true);
        spawn_local(async move {
            match api::sync_forget(&target.id).await {
                Ok(()) => {
                    forget_target.set(None);
                    context.toaster.info(Message::with(
                        key::SETTINGS_DEVICES_FORGOTTEN,
                        "name",
                        target.name,
                    ));
                    refresh();
                }
                Err(error) => context.toaster.fail(error),
            }
            forgetting.set(false);
        });
    };

    let save_device_name = move || {
        let next = device_name.get_untracked();
        context.set(move |s| s.sync_device_name = next);
    };

    let save_relay_url = move || {
        let next = relay_url.get_untracked();
        context.set(move |s| s.sync_relay_url = next);
    };

    let enabled = Signal::derive(move || status.with(|s| s.enabled));
    let forget_open = Signal::derive(move || forget_target.with(|t| t.is_some()));
    let forget_title = Signal::derive(move || {
        let name = forget_target
            .with(|t| t.as_ref().map(|d| d.name.clone()))
            .unwrap_or_default();
        i18n.translator()
            .format(key::SETTINGS_DEVICES_FORGET_TITLE, &[("name", &name)])
    });

    view! {
        <section class="settings__section" aria-labelledby="s-devices">
            <h2 class="settings__heading settings__heading--tagged" id="s-devices">
                <span>{i18n.text(key::SETTINGS_DEVICES_HEADING)}</span>
                <span class="badge">{i18n.text(key::SETTINGS_DEVICES_EXPERIMENTAL)}</span>
            </h2>

            <Row
                i18n=i18n
                label=key::SETTINGS_DEVICES_ENABLE
                help=key::SETTINGS_DEVICES_ENABLE_HELP
            >
                <Switch
                    label=i18n.label(key::SETTINGS_DEVICES_ENABLE)
                    checked=enabled
                    on_change=on_toggle
                />
            </Row>

            <Show when=move || enabled.get()>
                <Row i18n=i18n label=key::SETTINGS_DEVICES_THIS_COMPUTER>
                    <input
                        class="field"
                        aria-label=i18n.text(key::SETTINGS_DEVICES_THIS_COMPUTER)
                        placeholder=i18n.text(key::SETTINGS_DEVICES_THIS_COMPUTER_PLACEHOLDER)
                        use:bind_text=Signal::derive(move || device_name.get())
                        on:input=move |ev| device_name.set(event_target_value(&ev))
                        on:blur=move |_| save_device_name()
                    />
                </Row>

                <div class="row row--stack">
                    <div class="row__control">
                        <Button busy=offering on_click=on_pair_offer>
                            {i18n.text(key::SETTINGS_DEVICES_PAIR_ACTION)}
                        </Button>
                        <Button variant=Variant::Quiet busy=syncing on_click=on_sync_now>
                            {i18n.text(key::SETTINGS_DEVICES_SYNC_NOW)}
                        </Button>
                    </div>

                    {move || offer_code.get().map(|code| view! {
                        <div class="devices__offer">
                            <div class="path selectable">{code}</div>
                            <div class="row__control">
                                <Button variant=Variant::Quiet on_click=on_copy_offer>
                                    {i18n.text(key::SETTINGS_DEVICES_COPY)}
                                </Button>
                            </div>
                            <div class="row__help">{i18n.text(key::SETTINGS_DEVICES_OFFER_HELP)}</div>
                        </div>
                    })}
                </div>

                <div class="row row--stack">
                    <div class="row__label">{i18n.text(key::SETTINGS_DEVICES_HAVE_CODE)}</div>
                    <div class="row__control">
                        <input
                            class="field"
                            aria-label=i18n.text(key::SETTINGS_DEVICES_HAVE_CODE)
                            placeholder=i18n.text(key::SETTINGS_DEVICES_HAVE_CODE_PLACEHOLDER)
                            autocomplete="off"
                            spellcheck="false"
                            use:bind_text=Signal::derive(move || join_code.get())
                            on:input=move |ev| join_code.set(event_target_value(&ev))
                        />
                        <Button
                            busy=joining
                            disabled=Signal::derive(move || join_code.with(|c| c.trim().is_empty()))
                            on_click=on_pair_join
                        >
                            {i18n.text(key::SETTINGS_DEVICES_PAIR_SUBMIT)}
                        </Button>
                    </div>
                </div>

                <div class="row row--stack">
                    {move || {
                        let devices = status.with(|s| s.devices.clone());
                        let locale = time::locale_for(&language.get());
                        if devices.is_empty() {
                            view! {
                                <div class="row__help">{i18n.text(key::SETTINGS_DEVICES_EMPTY)}</div>
                            }
                            .into_any()
                        } else {
                            view! {
                                <ul class="devices__list">
                                    {devices.into_iter().map(|dev| {
                                        let dev_for_forget = dev.clone();
                                        let sync_line = match dev.last_sync_at {
                                            Some(ms) => {
                                                let formatted = time::format_time(ms, &locale);
                                                i18n.translator().format(
                                                    key::SETTINGS_DEVICES_LAST_SYNCED,
                                                    &[("time", &formatted)],
                                                )
                                            }
                                            None => i18n
                                                .translator()
                                                .text(key::SETTINGS_DEVICES_NOT_SYNCED)
                                                .to_owned(),
                                        };
                                        let state_attr = if dev.online { "ok" } else { "idle" };
                                        view! {
                                            <li class="devices__item">
                                                <div class="devices__meta">
                                                    <div class="status" data-state=state_attr>
                                                        <span class="row__label">{dev.name}</span>
                                                    </div>
                                                    <div class="row__help">{sync_line}</div>
                                                </div>
                                                <Button
                                                    variant=Variant::Quiet
                                                    on_click=move |()| forget_target.set(Some(dev_for_forget.clone()))
                                                >
                                                    {i18n.text(key::SETTINGS_DEVICES_FORGET)}
                                                </Button>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            }
                            .into_any()
                        }
                    }}

                    {move || status.with(|s| s.last_error.clone()).map(|err| view! {
                        <p class="status selectable" data-state="error" role="alert">
                            {err}
                        </p>
                    })}
                </div>

                <details class="devices__advanced">
                    <summary class="row__help">{i18n.text(key::SETTINGS_DEVICES_ADVANCED)}</summary>
                    <Row
                        i18n=i18n
                        label=key::SETTINGS_DEVICES_RELAY
                        help=key::SETTINGS_DEVICES_RELAY_HELP
                    >
                        <input
                            class="field"
                            aria-label=i18n.text(key::SETTINGS_DEVICES_RELAY)
                            autocomplete="off"
                            spellcheck="false"
                            use:bind_text=Signal::derive(move || relay_url.get())
                            on:input=move |ev| relay_url.set(event_target_value(&ev))
                            on:blur=move |_| save_relay_url()
                        />
                    </Row>
                </details>
            </Show>

            <Dialog
                open=forget_open
                title=forget_title
                on_close=move |()| forget_target.set(None)
                actions=Box::new(move || view! {
                    <Button on_click=move |()| forget_target.set(None)>
                        {i18n.text(key::COMMON_CANCEL)}
                    </Button>
                    <Button variant=Variant::Danger busy=forgetting on_click=confirm_forget>
                        {i18n.text(key::SETTINGS_DEVICES_FORGET)}
                    </Button>
                }.into_any())
            >
                <p class="dialog__text">
                    {move || {
                        let name = forget_target
                            .with(|t| t.as_ref().map(|d| d.name.clone()))
                            .unwrap_or_default();
                        i18n.translator()
                            .format(key::SETTINGS_DEVICES_FORGET_TEXT, &[("name", &name)])
                    }}
                </p>
            </Dialog>
        </section>
    }
}
