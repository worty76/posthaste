//! Optional AI note polishing, with any of several providers.
//!
//! Off unless the user picks a provider in Settings. The request goes from
//! this machine straight to that provider (or to a server the user runs),
//! with the user's own key. Nothing runs or is sent automatically.
//!
//! - `providers`: the table of providers; adding one is usually one entry.
//! - `config`: settings plus a stored key → one checked setup.
//! - `wire`: the request and reply formats, as pure functions.
//! - `secrets`: keys in the system keychain, never in the settings.

pub mod config;
pub mod providers;
pub mod secrets;
pub mod wire;

use std::sync::{Arc, Condvar, Mutex};

use posthaste_core::Settings;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use providers::{key_account, KeyNeed, PROVIDERS};
use secrets::Secrets;

const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// What the test button polishes.
const SAMPLE: &str = "remind me to send teh slides to anna befor thursdays meeting";

pub use posthaste_core::{AiOverview, AiProviderInfo as ProviderInfo};

fn overview(state: &AppState, secrets: &Secrets) -> AppResult<AiOverview> {
    let providers = PROVIDERS
        .iter()
        .map(|p| {
            // Never reads the keychain: see `Secrets::has`.
            let has_key = p.key != KeyNeed::None && secrets.has(state, &key_account(p.id));
            ProviderInfo {
                id: p.id.to_owned(),
                name: p.name.to_owned(),
                key: p.key,
                has_key,
                key_url: p.key_url.map(str::to_owned),
                key_help: p.key_help.to_owned(),
                default_model: p.default_model.to_owned(),
                base_url: p.base_url.to_owned(),
                base_url_editable: p.base_url_editable,
            }
        })
        .collect();
    Ok(AiOverview {
        backend: secrets.backend(),
        providers,
    })
}

fn known(provider: &str) -> AppResult<&'static providers::Provider> {
    providers::find(provider)
        .ok_or_else(|| AppError::new("ai_unknown_provider", "That AI provider is not supported."))
}

/// Moves a key saved by an older version (plain text in the settings) out
/// of the settings: into the keychain, or into the database fallback if the
/// keychain refuses it. The settings are rewritten without it only once it
/// is stored somewhere else, so the key is never lost.
pub fn migrate_legacy_key(state: &AppState, secrets: &Secrets) -> AppResult<()> {
    let settings = state.settings();
    let legacy = settings.ai_api_key.trim().to_string();
    if legacy.is_empty() {
        return Ok(());
    }
    let provider = match providers::find(&settings.ai_provider) {
        Some(p) if p.key != KeyNeed::None => p.id,
        // AI was off (or on a keyless provider): file it where it most
        // likely belongs rather than drop it.
        _ => likely_provider(&legacy),
    };
    let account = key_account(provider);
    // A key already saved for that provider is newer than the old one.
    if secrets.get(state, &account)?.is_none() {
        if let Err(e) = secrets.set(state, &account, &legacy) {
            log::warn!(
                "keychain refused the old AI key, keeping it in the database: {}",
                e.message
            );
            state.with_db(|db| db.secret_set(&account, &legacy))?;
            secrets.note_saved(state, &account)?;
        }
    }
    // The keychain may have kept us waiting on a prompt: take the settings
    // as they are now, so a change made meanwhile is not undone.
    let mut current = state.settings.write().unwrap_or_else(|e| e.into_inner());
    let mut updated = current.clone();
    updated.ai_api_key.clear();
    state.with_db(|db| db.save_settings(&updated))?;
    *current = updated;
    log::info!("moved the AI key out of the settings");
    Ok(())
}

/// Which of the providers that existed before 0.3.2 a bare key belongs to.
fn likely_provider(key: &str) -> &'static str {
    if key.starts_with("sk-ant-") {
        "anthropic"
    } else if key.starts_with("AIza") {
        "gemini"
    } else {
        "openai"
    }
}

/// Settings as the interface may see them: never with an old key in them.
pub fn for_interface(settings: &Settings) -> Settings {
    let mut s = settings.clone();
    s.ai_api_key.clear();
    s
}

/// Settings from the interface never carry the old key; keep any that has
/// not been moved yet so saving them cannot lose it.
pub fn keep_legacy_key(mut from_interface: Settings, current: &Settings) -> Settings {
    from_interface.ai_api_key = current.ai_api_key.clone();
    from_interface
}

/// A key goes in an HTTP header, so it must be plain visible ASCII.
fn check_key(key: &str) -> AppResult<()> {
    let ok = key.is_ascii() && reqwest::header::HeaderValue::from_str(key).is_ok();
    if ok {
        Ok(())
    } else {
        Err(AppError::new(
            "ai_bad_key",
            "That key has characters a key cannot contain. Copy it again from the provider.",
        ))
    }
}

/// Removes every provider key, for "Clear all data". Best effort: a key
/// the keychain will not delete is logged, never a reason to stop.
pub fn forget_all_keys(state: &AppState, secrets: &Secrets) -> usize {
    PROVIDERS
        .iter()
        .filter(|p| match secrets.delete(state, &key_account(p.id)) {
            Ok(()) => false,
            Err(e) => {
                log::warn!("could not remove the {} key: {}", p.name, e.message);
                true
            }
        })
        .count()
}

/// Every provider's keychain account name.
pub fn key_accounts() -> Vec<String> {
    PROVIDERS.iter().map(|p| key_account(p.id)).collect()
}

/// Sets up the keychain off the main thread (it may prompt or wait on
/// D-Bus), moves old keys out of the settings, then hands the result to
/// the commands, which wait for it.
pub fn start(app: &tauri::AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let secrets = Secrets::detect();
        if let Err(e) = migrate_legacy_key(&state, &secrets) {
            log::error!("the old AI key could not be moved: {}", e.message);
        }
        let accounts = key_accounts();
        let accounts: Vec<&str> = accounts.iter().map(String::as_str).collect();
        secrets.adopt_database_keys(&state, &accounts);
        let retired: Vec<String> = posthaste_core::RETIRED_AI_PROVIDERS
            .iter()
            .map(|id| key_account(id))
            .collect();
        let retired: Vec<&str> = retired.iter().map(String::as_str).collect();
        secrets.index_existing(&state, &accounts, &retired);
        app.state::<SecretsCell>().put(secrets);
    });
}

/// Managed state: the key store, once `start` has set it up.
#[derive(Default)]
pub struct SecretsCell {
    slot: Mutex<Option<Arc<Secrets>>>,
    ready: Condvar,
}

impl SecretsCell {
    pub fn put(&self, secrets: Secrets) {
        *self.slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(secrets));
        self.ready.notify_all();
    }

    /// Blocks until the key store is ready; only call off the main thread.
    pub fn wait(&self) -> Arc<Secrets> {
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(s) = slot.as_ref() {
                return s.clone();
            }
            slot = self.ready.wait(slot).unwrap_or_else(|e| e.into_inner());
        }
    }
}

/// Runs keychain work on a blocking thread, never the main one: a keychain
/// may show a prompt or wait on D-Bus.
async fn with_keys<T: Send + 'static>(
    app: &tauri::AppHandle,
    f: impl FnOnce(&AppState, &Secrets) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let secrets = app.state::<SecretsCell>().wait();
        f(&state, &secrets)
    })
    .await
    .map_err(|e| {
        AppError::new(
            "keychain",
            format!("The key store stopped unexpectedly. ({e})"),
        )
    })?
}

/// The HTTP client for provider calls. Never follows redirects: a key in a
/// provider-specific header would otherwise travel to wherever a redirect
/// points.
fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| {
            AppError::new(
                "ai_network",
                format!("The AI client could not start. ({e})"),
            )
        })
}

async fn send(r: &config::Resolved, text: &str) -> AppResult<String> {
    let call = wire::build(r, text);
    let client = client()?;
    let mut req = client.post(&call.url).json(&call.body);
    for (name, value) in &call.headers {
        req = req.header(name, value);
    }
    let name = r.provider.name;
    let response = req.send().await.map_err(|e| {
        let message = if e.is_timeout() {
            format!("{name} took too long to answer. Try again.")
        } else if r.provider.base_url_editable {
            format!(
                "Could not reach {name} at {}. Check that it is running and the address is right.",
                r.base_url
            )
        } else {
            format!("Could not reach {name}. Check your internet connection.")
        };
        AppError::new("ai_network", message)
    })?;

    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response.text().await.map_err(|e| {
        AppError::new(
            "ai_network",
            format!("The answer from {name} was cut off. ({e})"),
        )
    })?;
    if !status.is_success() {
        return Err(AppError::new(
            "ai_provider_error",
            wire::describe_failure(name, status.as_u16(), &body),
        ));
    }
    wire::read_body(r.provider.wire, &body).ok_or_else(|| {
        log::warn!("unreadable answer from {name}: HTTP {status}, {content_type}");
        AppError::new(
            "ai_provider_error",
            wire::describe_unreadable(name, status.as_u16(), &content_type, &body),
        )
    })
}

fn resolve_for(state: &AppState, secrets: &Secrets, provider: &str) -> AppResult<config::Resolved> {
    let settings = state.settings();
    let key = match providers::find(provider) {
        Some(p) if p.key != KeyNeed::None => secrets.get(state, &key_account(p.id))?,
        _ => None,
    };
    config::resolve(&settings, provider, key)
}

/// The providers, whether each has a key, and where keys are kept.
#[tauri::command]
pub async fn ai_providers(app: AppHandle) -> AppResult<AiOverview> {
    with_keys(&app, overview).await
}

#[tauri::command]
pub async fn ai_key_set(app: AppHandle, provider: String, key: String) -> AppResult<AiOverview> {
    let p = known(&provider)?;
    if p.key == KeyNeed::None {
        return Err(AppError::new(
            "ai_no_key",
            format!("{} does not use a key.", p.name),
        ));
    }
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err(AppError::new("ai_no_key", "Paste a key first."));
    }
    check_key(&key)?;
    with_keys(&app, move |state, secrets| {
        secrets.set(state, &key_account(p.id), &key)?;
        overview(state, secrets)
    })
    .await
}

#[tauri::command]
pub async fn ai_key_clear(app: AppHandle, provider: String) -> AppResult<AiOverview> {
    let p = known(&provider)?;
    with_keys(&app, move |state, secrets| {
        secrets.delete(state, &key_account(p.id))?;
        overview(state, secrets)
    })
    .await
}

/// Opens the page where `provider` hands out keys, from the table above;
/// the page never chooses the address.
#[tauri::command]
pub fn ai_key_page(app: AppHandle, provider: String) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let p = known(&provider)?;
    let url = p
        .key_url
        .ok_or_else(|| AppError::new("ai_no_key", format!("{} does not use a key.", p.name)))?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::new("io", format!("The page could not be opened: {e}")))
}

/// Polishes a sample note with `provider`, whether or not it is the one in
/// use, so Settings can show the setup works before switching to it.
#[tauri::command]
pub async fn ai_test(app: AppHandle, provider: String) -> AppResult<String> {
    let r = with_keys(&app, move |state, secrets| {
        resolve_for(state, secrets, &provider)
    })
    .await?;
    send(&r, SAMPLE).await
}

/// Polishes the note text with the provider in use.
#[tauri::command]
pub async fn ai_polish(app: AppHandle, text: String) -> AppResult<String> {
    if text.trim().is_empty() {
        return Err(AppError::new(
            "empty_note",
            "Nothing to polish — the note is empty.",
        ));
    }
    let r = with_keys(&app, |state, secrets| {
        let provider = state.settings().ai_provider;
        resolve_for(state, secrets, &provider)
    })
    .await?;
    send(&r, text.trim()).await
}

#[cfg(test)]
mod tests {
    use super::secrets::tests::{state, FakeKeychain};
    use super::*;
    use posthaste_core::Settings;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    fn broken() -> (Arc<FakeKeychain>, Secrets) {
        let k = FakeKeychain::broken();
        (k.clone(), Secrets::with_keychain(Box::new(k)))
    }

    fn keychain() -> (Arc<FakeKeychain>, Secrets) {
        let k = Arc::new(FakeKeychain::default());
        (k.clone(), Secrets::with_keychain(Box::new(k)))
    }

    /// Settings as an older version left them in memory after loading.
    fn store_raw(state: &AppState, json: &str) {
        let parsed: Settings = serde_json::from_str(json).unwrap();
        *state.settings.write().unwrap() = parsed;
    }

    fn stored_blob(state: &AppState) -> String {
        let s = state.with_db(|db| db.load_settings()).unwrap();
        serde_json::to_string(&s).unwrap()
    }

    #[test]
    fn an_old_plain_text_key_moves_into_the_keychain() {
        let state = state();
        let (keychain, secrets) = keychain();
        store_raw(&state, r#"{"ai_provider":"gemini","ai_api_key":" g-old "}"#);

        migrate_legacy_key(&state, &secrets).unwrap();

        assert_eq!(keychain.has("ai:gemini").as_deref(), Some("g-old"));
        assert_eq!(state.settings().ai_api_key, "");
        assert_eq!(state.settings().ai_provider, "gemini");
        assert!(!stored_blob(&state).contains("g-old"));
        // Running again is harmless.
        migrate_legacy_key(&state, &secrets).unwrap();
        assert_eq!(keychain.has("ai:gemini").as_deref(), Some("g-old"));
    }

    #[test]
    fn a_newer_keychain_key_wins_over_the_old_one() {
        let state = state();
        let (keychain, secrets) = keychain();
        keychain
            .items
            .lock()
            .unwrap()
            .insert("ai:openai".into(), "sk-new".into());
        store_raw(&state, r#"{"ai_provider":"openai","ai_api_key":"sk-old"}"#);
        migrate_legacy_key(&state, &secrets).unwrap();
        assert_eq!(keychain.has("ai:openai").as_deref(), Some("sk-new"));
    }

    #[test]
    fn with_no_keychain_the_old_key_moves_out_of_the_visible_settings() {
        let state = state();
        let (_, secrets) = broken();
        store_raw(
            &state,
            r#"{"ai_provider":"anthropic","ai_api_key":"sk-ant"}"#,
        );
        migrate_legacy_key(&state, &secrets).unwrap();
        assert_eq!(
            state
                .with_db(|db| db.secret_get("ai:anthropic"))
                .unwrap()
                .as_deref(),
            Some("sk-ant")
        );
        assert!(!stored_blob(&state).contains("sk-ant"));
    }

    #[test]
    fn opening_settings_never_reads_the_keychain() {
        let state = state();
        let (k, secrets) = keychain();
        secrets.set(&state, "ai:openai", "sk-1").unwrap();
        secrets.set(&state, "ai:gemini", "g-1").unwrap();
        k.reads.store(0, Ordering::SeqCst);

        let o = overview(&state, &secrets).unwrap();

        assert_eq!(k.reads.load(Ordering::SeqCst), 0, "no keychain read");
        let has: Vec<&str> = o
            .providers
            .iter()
            .filter(|p| p.has_key)
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(has, ["openai", "gemini"]);

        secrets.delete(&state, "ai:openai").unwrap();
        let o = overview(&state, &secrets).unwrap();
        assert!(
            !o.providers
                .iter()
                .find(|p| p.id == "openai")
                .unwrap()
                .has_key
        );
    }

    #[test]
    fn keys_from_an_older_build_are_noticed_once_and_never_read_again() {
        let state = state();
        let (k, secrets) = keychain();
        k.items
            .lock()
            .unwrap()
            .insert("ai:anthropic".into(), "sk-ant".into());
        k.items
            .lock()
            .unwrap()
            .insert("ai:github".into(), "old-github-token".into());
        let accounts = key_accounts();
        let accounts: Vec<&str> = accounts.iter().map(String::as_str).collect();
        assert!(!overview(&state, &secrets)
            .unwrap()
            .providers
            .iter()
            .any(|p| p.has_key));

        secrets.index_existing(&state, &accounts, &["ai:github"]);

        let o = overview(&state, &secrets).unwrap();
        assert!(
            o.providers
                .iter()
                .find(|p| p.id == "anthropic")
                .unwrap()
                .has_key
        );
        assert!(
            !o.providers
                .iter()
                .find(|p| p.id == "openai")
                .unwrap()
                .has_key
        );
        assert_eq!(k.has("ai:github"), None, "retired provider's key removed");
        assert_eq!(k.has("ai:anthropic").as_deref(), Some("sk-ant"));

        let reads = k.reads.load(Ordering::SeqCst);
        secrets.index_existing(&state, &accounts, &["ai:github"]);
        assert_eq!(k.reads.load(Ordering::SeqCst), reads, "indexed only once");
    }

    #[test]
    fn a_key_in_the_database_fallback_shows_as_saved() {
        let state = state();
        let (_, secrets) = broken();
        secrets.set(&state, "ai:openai", "sk-1").unwrap();
        let o = overview(&state, &secrets).unwrap();
        assert!(
            o.providers
                .iter()
                .find(|p| p.id == "openai")
                .unwrap()
                .has_key
        );
        assert_eq!(o.backend, secrets::Backend::Database);
    }

    #[test]
    fn the_overview_says_which_providers_have_keys_without_revealing_them() {
        let state = state();
        let (_, secrets) = keychain();
        secrets.set(&state, "ai:openrouter", "or-key-x").unwrap();
        let o = overview(&state, &secrets).unwrap();
        assert_eq!(o.backend, secrets::Backend::Keychain);
        let router = o.providers.iter().find(|p| p.id == "openrouter").unwrap();
        assert!(router.has_key);
        assert!(
            !o.providers
                .iter()
                .find(|p| p.id == "openai")
                .unwrap()
                .has_key
        );
        assert!(!serde_json::to_string(&o).unwrap().contains("or-key-x"));
        assert_eq!(o.providers.len(), PROVIDERS.len());
    }

    #[test]
    fn polishing_uses_the_key_stored_for_that_provider() {
        let state = state();
        let (_, secrets) = keychain();
        *state.settings.write().unwrap() = Settings {
            ai_provider: "openrouter".into(),
            ..Settings::default()
        };
        let err = resolve_for(&state, &secrets, "openrouter").unwrap_err();
        assert_eq!(err.code, "ai_not_configured");
        secrets.set(&state, "ai:openrouter", "or-1").unwrap();
        let r = resolve_for(&state, &secrets, "openrouter").unwrap();
        assert_eq!(r.key.as_deref(), Some("or-1"));
        assert_eq!(r.base_url, "https://openrouter.ai/api/v1");
    }

    #[test]
    fn clearing_all_data_forgets_every_key() {
        let state = state();
        let (keychain, secrets) = keychain();
        secrets.set(&state, "ai:openai", "a").unwrap();
        secrets.set(&state, "ai:custom", "b").unwrap();
        assert_eq!(forget_all_keys(&state, &secrets), 0);
        assert!(keychain.items.lock().unwrap().is_empty());
    }

    #[test]
    fn a_refused_keychain_write_keeps_the_old_key_safe_in_the_database() {
        let state = state();
        let (keychain, secrets) = keychain();
        keychain.deny_writes.store(true, Ordering::SeqCst);
        store_raw(&state, r#"{"ai_provider":"openai","ai_api_key":"sk-old"}"#);

        migrate_legacy_key(&state, &secrets).unwrap();

        assert_eq!(
            state
                .with_db(|db| db.secret_get("ai:openai"))
                .unwrap()
                .as_deref(),
            Some("sk-old")
        );
        assert_eq!(
            secrets.get(&state, "ai:openai").unwrap().as_deref(),
            Some("sk-old")
        );
        assert!(!stored_blob(&state).contains("sk-old"));
    }

    #[test]
    fn an_old_key_kept_while_ai_was_off_is_filed_under_its_likely_provider() {
        for (key, provider) in [
            ("sk-ant-api03-x", "anthropic"),
            ("AIzaSyX", "gemini"),
            ("sk-proj-x", "openai"),
        ] {
            let state = state();
            let (keychain, secrets) = keychain();
            store_raw(
                &state,
                &format!(r#"{{"ai_provider":"none","ai_api_key":"{key}"}}"#),
            );
            migrate_legacy_key(&state, &secrets).unwrap();
            assert_eq!(
                keychain.has(&format!("ai:{provider}")).as_deref(),
                Some(key),
                "{key}"
            );
            assert_eq!(state.settings().ai_provider, "none");
        }
    }

    #[test]
    fn the_interface_never_receives_an_old_key() {
        let s = Settings {
            ai_api_key: "sk-old".into(),
            ..Settings::default()
        };
        assert_eq!(for_interface(&s).ai_api_key, "");
        assert!(!serde_json::to_string(&for_interface(&s))
            .unwrap()
            .contains("sk-old"));
    }

    #[test]
    fn a_settings_change_from_the_interface_keeps_an_unmoved_old_key() {
        let old = Settings {
            ai_api_key: "sk-old".into(),
            ..Settings::default()
        };
        let from_ui = Settings {
            theme: posthaste_core::Theme::Dark,
            ..Settings::default()
        };
        let merged = keep_legacy_key(from_ui, &old);
        assert_eq!(merged.ai_api_key, "sk-old");
        assert_eq!(merged.theme, posthaste_core::Theme::Dark);
    }

    #[test]
    fn requests_do_not_follow_redirects_with_the_key() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let elsewhere = TcpListener::bind("127.0.0.1:0").unwrap();
        let elsewhere_port = elsewhere.local_addr().unwrap().port();
        elsewhere.set_nonblocking(true).unwrap();
        let provider = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = provider.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut conn, _) = provider.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = conn.read(&mut buf);
            let reply = format!(
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{elsewhere_port}/steal\r\nContent-Length: 0\r\n\r\n"
            );
            let _ = conn.write_all(reply.as_bytes());
        });
        let mut s = Settings {
            ai_provider: "custom".into(),
            ..Settings::default()
        };
        s.ai_base_urls
            .insert("custom".into(), format!("http://127.0.0.1:{port}/v1"));
        s.ai_models.insert("custom".into(), "m".into());
        let r = config::resolve(&s, "custom", Some("sk-secret".into())).unwrap();

        let err = tauri::async_runtime::block_on(send(&r, "note")).unwrap_err();

        assert_eq!(err.code, "ai_provider_error", "{}", err.message);
        assert!(
            elsewhere.accept().is_err(),
            "the redirect target was contacted"
        );
    }

    #[test]
    fn a_key_that_cannot_travel_in_a_header_is_refused_when_saved() {
        assert!(check_key("sk-ok_123").is_ok());
        assert!(check_key("sk-\nbad").is_err());
        assert!(check_key("ключ").is_err());
    }

    #[test]
    fn a_change_made_while_the_key_moves_is_kept() {
        struct ChangesSettingsOnWrite(Arc<FakeKeychain>, std::sync::Weak<AppState>);
        impl secrets::KeyStore for ChangesSettingsOnWrite {
            fn get(&self, a: &str) -> Result<Option<String>, String> {
                self.0.get(a)
            }
            fn set(&self, a: &str, v: &str) -> Result<(), String> {
                // The user changes a setting while a keychain prompt is up.
                if let Some(state) = self.1.upgrade() {
                    state.settings.write().unwrap().theme = posthaste_core::Theme::Dark;
                }
                self.0.set(a, v)
            }
            fn delete(&self, a: &str) -> Result<(), String> {
                self.0.delete(a)
            }
        }
        let state = Arc::new(state());
        let secrets = Secrets::with_keychain(Box::new(ChangesSettingsOnWrite(
            Arc::new(FakeKeychain::default()),
            Arc::downgrade(&state),
        )));
        store_raw(&state, r#"{"ai_provider":"openai","ai_api_key":"sk-old"}"#);
        migrate_legacy_key(&state, &secrets).unwrap();
        assert_eq!(state.settings().theme, posthaste_core::Theme::Dark);
        assert_eq!(state.settings().ai_api_key, "");
    }
}
