//! Turns settings plus a stored key into one ready-to-send provider setup,
//! refusing anything incomplete before a request leaves the machine.

use posthaste_core::Settings;

use super::providers::{self, KeyNeed, Provider};
use crate::error::AppError;

/// Everything one polish request needs.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub provider: &'static Provider,
    /// No trailing slash.
    pub base_url: String,
    pub model: String,
    pub key: Option<String>,
}

fn not_configured(message: impl Into<String>) -> AppError {
    AppError::new("ai_not_configured", message)
}

/// The setup for `provider_id`, using what the user chose for it in
/// Settings and the key stored for it (if any).
pub fn resolve(
    settings: &Settings,
    provider_id: &str,
    key: Option<String>,
) -> Result<Resolved, AppError> {
    let provider = providers::find(provider_id)
        .ok_or_else(|| not_configured("AI polish is not set up. Pick a provider in Settings."))?;
    let name = provider.name;

    let key = key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
    let key = match provider.key {
        KeyNeed::Required if key.is_none() => {
            return Err(not_configured(format!(
                "{name} needs an API key. Add yours in Settings."
            )))
        }
        KeyNeed::None => None,
        _ => key,
    };

    let base_url = chosen(&settings.ai_base_urls, provider.id)
        .filter(|_| provider.base_url_editable)
        .unwrap_or(provider.base_url)
        .trim_end_matches('/')
        .to_string();
    if base_url.is_empty() {
        return Err(not_configured(format!(
            "{name} needs the server address. Add it in Settings."
        )));
    }
    if !(base_url.starts_with("https://") || base_url.starts_with("http://")) {
        return Err(not_configured(format!(
            "The {name} address must start with http:// or https://."
        )));
    }

    let model = chosen(&settings.ai_models, provider.id)
        .unwrap_or(provider.default_model)
        .to_string();
    if model.is_empty() {
        return Err(not_configured(format!(
            "{name} needs a model name. Add it in Settings."
        )));
    }

    Ok(Resolved {
        provider,
        base_url,
        model,
        key,
    })
}

/// A non-empty, trimmed choice for this provider.
fn chosen<'a>(map: &'a std::collections::BTreeMap<String, String>, id: &str) -> Option<&'a str> {
    map.get(id).map(|v| v.trim()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(provider: &str) -> Settings {
        Settings {
            ai_provider: provider.into(),
            ..Settings::default()
        }
    }

    fn key(k: &str) -> Option<String> {
        Some(k.into())
    }

    #[test]
    fn off_or_unknown_is_not_configured() {
        for id in ["none", "", "copilot"] {
            let err = resolve(&with(id), id, key("k")).unwrap_err();
            assert_eq!(err.code, "ai_not_configured", "{id}");
        }
    }

    #[test]
    fn a_provider_that_needs_a_key_refuses_to_run_without_one() {
        for missing in [None, key("   ")] {
            let err = resolve(&with("openai"), "openai", missing).unwrap_err();
            assert_eq!(err.code, "ai_not_configured");
            assert!(
                err.message.contains("OpenAI needs an API key"),
                "{}",
                err.message
            );
        }
    }

    #[test]
    fn defaults_apply_until_the_user_picks_a_model() {
        let r = resolve(&with("openai"), "openai", key(" sk-1 ")).unwrap();
        assert_eq!(r.model, "gpt-4o-mini");
        assert_eq!(r.base_url, "https://api.openai.com/v1");
        assert_eq!(r.key.as_deref(), Some("sk-1"));

        let mut s = with("openai");
        s.ai_models.insert("openai".into(), " gpt-4.1-nano ".into());
        s.ai_models.insert("gemini".into(), "not-this-one".into());
        assert_eq!(
            resolve(&s, "openai", key("k")).unwrap().model,
            "gpt-4.1-nano"
        );
    }

    #[test]
    fn a_fixed_address_cannot_be_redirected_from_settings() {
        // A stray setting must not send an OpenAI key to another server.
        let mut s = with("openai");
        s.ai_base_urls
            .insert("openai".into(), "https://example.com/v1".into());
        assert_eq!(
            resolve(&s, "openai", key("k")).unwrap().base_url,
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn ollama_needs_no_key_and_can_move() {
        let r = resolve(&with("ollama"), "ollama", None).unwrap();
        assert_eq!(r.base_url, "http://localhost:11434/v1");
        assert_eq!(r.key, None);

        let mut s = with("ollama");
        s.ai_base_urls
            .insert("ollama".into(), "http://192.168.1.5:11434/v1/".into());
        // A key stored by mistake is never sent to a keyless provider.
        let r = resolve(&s, "ollama", key("stray")).unwrap();
        assert_eq!(r.base_url, "http://192.168.1.5:11434/v1");
        assert_eq!(r.key, None);
    }

    #[test]
    fn a_custom_server_needs_an_address_and_a_model_but_not_a_key() {
        let err = resolve(&with("custom"), "custom", None).unwrap_err();
        assert!(err.message.contains("server address"), "{}", err.message);

        let mut s = with("custom");
        s.ai_base_urls
            .insert("custom".into(), "http://localhost:1234/v1".into());
        let err = resolve(&s, "custom", None).unwrap_err();
        assert!(err.message.contains("model name"), "{}", err.message);

        s.ai_models.insert("custom".into(), "qwen2.5".into());
        let r = resolve(&s, "custom", None).unwrap();
        assert_eq!((r.model.as_str(), r.key), ("qwen2.5", None));
        assert_eq!(
            resolve(&s, "custom", key("k")).unwrap().key.as_deref(),
            Some("k")
        );
    }

    #[test]
    fn an_address_without_a_scheme_is_refused() {
        let mut s = with("custom");
        s.ai_base_urls
            .insert("custom".into(), "localhost:1234".into());
        s.ai_models.insert("custom".into(), "m".into());
        let err = resolve(&s, "custom", None).unwrap_err();
        assert!(err.message.contains("http://"), "{}", err.message);
    }
}
