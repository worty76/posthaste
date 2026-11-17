//! AI polish: pick one provider, then set up only that one. Keys go to the
//! system keychain through their own commands; the page never holds a saved
//! key and never puts one in the settings.

use leptos::prelude::*;
use leptos::task::spawn_local;
use posthaste_i18n::key;
use posthaste_model::{AiOverview, AiProviderInfo, CommandError, KeyNeed, KeyStore};

use super::{Context, Row};
use crate::browser::bind_text;
use crate::components::{Button, Variant};
use crate::ipc::{self, api};

/// The provider id that means polishing is off.
const OFF: &str = "none";

/// How the last test of a provider went.
#[derive(Debug, Clone, PartialEq)]
enum Outcome {
    Running,
    /// The polished sample the provider returned.
    Worked(String),
    Failed(CommandError),
}

#[component]
pub(super) fn AiSection(context: Context, language: Signal<String>) -> impl IntoView {
    let i18n = context.i18n;
    let chosen = context.read(|s| s.ai_provider.clone());
    let overview = RwSignal::new(None::<AiOverview>);
    let failed = RwSignal::new(false);
    // A result belongs to the provider it ran against.
    let test = RwSignal::new(None::<(String, Outcome)>);

    // Loaded again whenever the provider changes, so a key removed elsewhere
    // ("Clear all data") never still shows as saved.
    let request = StoredValue::new(0u32);
    Effect::new(move |_| {
        chosen.with(|_| ());
        let id = request.get_value().wrapping_add(1);
        request.set_value(id);
        spawn_local(async move {
            let answer = api::ai_providers().await;
            if request.get_value() != id {
                return;
            }
            match answer {
                Ok(found) => {
                    overview.set(Some(found));
                    failed.set(false);
                }
                Err(error) => {
                    failed.set(true);
                    context.toaster.fail(error);
                }
            }
        });
    });

    let provider = Memo::new(move |_| {
        let id = chosen.get();
        overview.with(|overview| {
            overview
                .as_ref()
                .and_then(|overview| overview.providers.iter().find(|p| p.id == id).cloned())
        })
    });
    // The setting names a provider this build does not have (or could not
    // list): it stays selected, marked, instead of silently becoming "Off".
    let unknown = move || {
        let id = chosen.get();
        let missing = overview.with(Option::is_some) && provider.with(Option::is_none);
        (id != OFF && (missing || failed.get())).then_some(id)
    };

    view! {
        <section class="settings__section" aria-labelledby="s-ai">
            <h2 class="settings__heading" id="s-ai">{i18n.text(key::AI_HEADING)}</h2>
            <Row i18n=i18n label=key::AI_PROVIDER help=key::AI_PROVIDER_HELP>
                <select
                    class="field select"
                    aria-label=i18n.text(key::AI_PROVIDER)
                    disabled=move || overview.with(Option::is_none) && !failed.get()
                    on:change=move |event| {
                        let id = event_target_value(&event);
                        context.set(move |s| s.ai_provider = id);
                    }
                >
                    <option value=OFF selected=move || chosen.with(|id| id == OFF)>
                        {i18n.text(key::AI_OFF)}
                    </option>
                    {move || unknown().map(|id| view! {
                        <option value=id.clone() disabled selected>
                            {i18n.translator().format(key::AI_UNAVAILABLE, &[("name", &id)])}
                        </option>
                    })}
                    {move || overview.with(|overview| {
                        overview.iter().flat_map(|overview| &overview.providers).map(|p| {
                            let id = p.id.clone();
                            view! {
                                <option
                                    value=p.id.clone()
                                    selected=move || chosen.with(|chosen| *chosen == id)
                                >
                                    {p.name.clone()}
                                </option>
                            }
                        }).collect_view()
                    })}
                </select>
            </Row>

            {move || {
                // Read so the fields are rebuilt in the new language.
                language.with(|_| ());
                let provider = provider.get()?;
                let backend = overview.with(|overview| overview.as_ref().map(|o| o.backend))?;
                Some(view! {
                    <Setup
                        context=context
                        provider=provider
                        backend=backend
                        overview=overview
                        test=test
                    />
                })
            }}
        </section>
    }
}

/// Everything one provider needs: its key, its address, its model, a test.
#[component]
fn Setup(
    context: Context,
    provider: AiProviderInfo,
    backend: KeyStore,
    overview: RwSignal<Option<AiOverview>>,
    test: RwSignal<Option<(String, Outcome)>>,
) -> impl IntoView {
    let i18n = context.i18n;
    let t = i18n.translator();
    let id = StoredValue::new(provider.id.clone());
    let name = provider.name.clone();
    // What was typed stays in the field while each change is being saved.
    let stored = move |read: fn(
        &posthaste_model::Settings,
    ) -> &std::collections::BTreeMap<String, String>| {
        context.settings.settings.with_untracked(|settings| {
            settings
                .as_ref()
                .and_then(|settings| read(settings).get(&id.get_value()).cloned())
                .unwrap_or_default()
        })
    };
    let address = RwSignal::new(stored(|s| &s.ai_base_urls));
    let model = RwSignal::new(stored(|s| &s.ai_models));

    let outcome = Memo::new(move |_| {
        test.with(|test| match test {
            Some((tested, outcome)) if *tested == id.get_value() => Some(outcome.clone()),
            _ => None,
        })
    });
    let running = move || outcome.with(|outcome| *outcome == Some(Outcome::Running));
    let run_test = move |()| {
        let id = id.get_value();
        test.set(Some((id.clone(), Outcome::Running)));
        spawn_local(async move {
            let outcome = match api::ai_test(&id).await {
                Ok(text) => Outcome::Worked(text),
                Err(error) => Outcome::Failed(error),
            };
            test.set(Some((id, outcome)));
        });
    };
    let open_key_page = move |_| {
        spawn_local(async move {
            if let Err(error) = api::ai_key_page(&id.get_value()).await {
                context.toaster.fail(error);
            }
        });
    };
    let model_help = if provider.default_model.is_empty() {
        key::AI_MODEL_HELP_REQUIRED
    } else {
        key::AI_MODEL_HELP_DEFAULT
    };
    let or = move |value: &str, fallback| {
        if value.is_empty() {
            t.text(fallback).to_owned()
        } else {
            value.to_owned()
        }
    };

    view! {
        <p class="ai__about">
            {provider.key_help.clone()}
            {(provider.key != KeyNeed::None).then(|| format!(
                " {}",
                t.format(key::AI_SENDS, &[("name", &name)]),
            ))}
            {provider.key_url.is_some().then(|| view! {
                " "
                <button type="button" class="link" on:click=open_key_page>
                    {t.text(key::AI_GET_KEY)}
                </button>
            })}
        </p>

        {(provider.key != KeyNeed::None).then(|| view! {
            <KeyRow context=context provider=provider.clone() backend=backend overview=overview />
        })}

        {provider.base_url_editable.then({
            let name = name.clone();
            move || view! {
            <Row i18n=i18n label=key::AI_ADDRESS help=key::AI_ADDRESS_HELP>
                <input
                    class="field"
                    aria-label=t.format(key::AI_ADDRESS_FIELD, &[("name", &name)])
                    placeholder=or(&provider.base_url, key::AI_ADDRESS_PLACEHOLDER)
                    spellcheck="false"
                    autocomplete="off"
                    use:bind_text=Signal::derive(move || address.get())
                    on:input=move |event| {
                        let value = event_target_value(&event);
                        address.set(value.clone());
                        context.set(move |s| {
                            s.ai_base_urls.insert(id.get_value(), value);
                        });
                    }
                />
            </Row>
        }})}

        <Row i18n=i18n label=key::AI_MODEL help=model_help>
            <input
                class="field"
                aria-label=t.format(key::AI_MODEL_FIELD, &[("name", &name)])
                placeholder=or(&provider.default_model, key::AI_MODEL_PLACEHOLDER)
                spellcheck="false"
                autocomplete="off"
                use:bind_text=Signal::derive(move || model.get())
                on:input=move |event| {
                    let value = event_target_value(&event);
                    model.set(value.clone());
                    context.set(move |s| {
                        s.ai_models.insert(id.get_value(), value);
                    });
                }
            />
        </Row>

        <Row i18n=i18n label=key::AI_CHECK help=key::AI_CHECK_HELP>
            <Button variant=Variant::Quiet disabled=Signal::derive(running) on_click=run_test>
                {move || i18n.translator().text(if running() { key::AI_TESTING } else { key::AI_TEST })}
            </Button>
        </Row>
        {move || match outcome.get() {
            Some(Outcome::Worked(text)) => view! {
                <p class="status ai__result selectable" data-state="ok" role="status">
                    {i18n.translator().format(key::AI_WORKS, &[("text", &text)])}
                </p>
            }
            .into_any(),
            Some(Outcome::Failed(error)) => view! {
                <p class="status ai__result selectable" data-state="error" role="alert">
                    {ipc::message(&error, i18n.translator())}
                </p>
            }
            .into_any(),
            Some(Outcome::Running) | None => ().into_any(),
        }}
    }
}

/// The provider's API key: saved (replace or remove it), or a field to
/// paste one into. A saved key is never shown.
#[component]
fn KeyRow(
    context: Context,
    provider: AiProviderInfo,
    backend: KeyStore,
    overview: RwSignal<Option<AiOverview>>,
) -> impl IntoView {
    let i18n = context.i18n;
    let draft = RwSignal::new(String::new());
    let replacing = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let id = StoredValue::new(provider.id.clone());
    let has_key = provider.has_key;
    let optional = provider.key == KeyNeed::Optional;

    // Saves or removes the key; the answer says what is stored now.
    let apply = move |key: Option<String>| {
        busy.set(true);
        spawn_local(async move {
            let id = id.get_value();
            let answer = match key {
                Some(key) => api::ai_key_set(&id, &key).await,
                None => api::ai_key_clear(&id).await,
            };
            busy.set(false);
            match answer {
                // This row is rebuilt from the new overview.
                Ok(now) => overview.set(Some(now)),
                Err(error) => context.toaster.fail(error),
            }
        });
    };
    let save = move || {
        let key = draft.with_untracked(|draft| draft.trim().to_owned());
        if !key.is_empty() {
            apply(Some(key));
        }
    };
    let label = move || {
        view! {
            {i18n.text(key::AI_KEY)}
            {optional.then(|| view! {
                " "
                <span class="row__optional">{i18n.text(key::AI_OPTIONAL)}</span>
            })}
        }
    };
    let where_kept = match backend {
        KeyStore::Keychain => key::AI_KEY_IN_KEYCHAIN,
        KeyStore::Database => key::AI_KEY_IN_DATABASE,
    };
    let name = provider.name;

    view! {
        <div class="row">
            <div class="row__text">
                <div class="row__label">{label()}</div>
                <Show when=move || has_key && !replacing.get()>
                    <div class="row__help">{i18n.text(where_kept)}</div>
                </Show>
            </div>
            <div class="row__control">
                <Show
                    when=move || has_key && !replacing.get()
                    fallback={
                        let name = name.clone();
                        move || {
                            let name = name.clone();
                            view! {
                                <input
                                    class="field"
                                    type="password"
                                    aria-label=move || i18n.translator().format(
                                        key::AI_KEY_FIELD,
                                        &[("name", &name)],
                                    )
                                    placeholder=i18n.text(key::AI_KEY_PLACEHOLDER)
                                    spellcheck="false"
                                    autocomplete="off"
                                    use:bind_text=Signal::derive(move || draft.get())
                                    on:input=move |event| draft.set(event_target_value(&event))
                                    on:keydown=move |event| {
                                        if event.key() == "Enter" && !event.is_composing() {
                                            event.prevent_default();
                                            save();
                                        }
                                    }
                                />
                                <Button
                                    disabled=Signal::derive(move || {
                                        busy.get() || draft.with(|draft| draft.trim().is_empty())
                                    })
                                    on_click=move |()| save()
                                >
                                    {i18n.text(key::AI_SAVE_KEY)}
                                </Button>
                                <Show when=move || replacing.get()>
                                    <Button
                                        variant=Variant::Quiet
                                        disabled=busy
                                        on_click=move |()| replacing.set(false)
                                    >
                                        {i18n.text(key::COMMON_CANCEL)}
                                    </Button>
                                </Show>
                            }
                        }
                    }
                >
                    <Button variant=Variant::Quiet disabled=busy on_click=move |()| replacing.set(true)>
                        {i18n.text(key::AI_REPLACE)}
                    </Button>
                    <Button variant=Variant::Quiet disabled=busy on_click=move |()| apply(None)>
                        {i18n.text(key::AI_REMOVE)}
                    </Button>
                </Show>
            </div>
        </div>
    }
}
