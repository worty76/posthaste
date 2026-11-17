//! The AI providers Posthaste can polish notes with.
//!
//! A provider is data, not code: which wire format it speaks, where it
//! lives, how it wants its key and which small model it uses by default.
//! Most providers speak the OpenAI chat format, so adding one is usually a
//! single entry in [`PROVIDERS`]. A provider with a format of its own adds a
//! [`Wire`] variant and its two functions in `wire.rs`.

/// The request and reply format a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    /// `POST {base}/chat/completions`, OpenAI's chat format. Also spoken by
    /// OpenRouter, Ollama and most self-hosted servers.
    OpenAi,
    /// `POST {base}/messages`, Anthropic's Messages API.
    Anthropic,
    /// `POST {base}/models/{model}:generateContent`, Google's Gemini API.
    Gemini,
}

pub use posthaste_core::KeyNeed;

#[derive(Debug, Clone, Copy)]
pub struct Provider {
    /// Stored in settings; never change an id once shipped.
    pub id: &'static str,
    pub name: &'static str,
    pub wire: Wire,
    /// Where requests go. Empty when the user must supply it.
    pub base_url: &'static str,
    /// Whether Settings lets the user point it somewhere else.
    pub base_url_editable: bool,
    pub key: KeyNeed,
    /// Where to get a key, shown as a link in Settings.
    pub key_url: Option<&'static str>,
    /// What kind of key it takes, in a few words.
    pub key_help: &'static str,
    /// A small, fast model. Empty when the user must name one.
    pub default_model: &'static str,
    /// Sent with every request, after the auth header.
    pub extra_headers: &'static [(&'static str, &'static str)],
}

/// Every provider, in the order Settings lists them.
pub const PROVIDERS: &[Provider] = &[
    Provider {
        id: "anthropic",
        name: "Claude",
        wire: Wire::Anthropic,
        base_url: "https://api.anthropic.com/v1",
        base_url_editable: false,
        key: KeyNeed::Required,
        key_url: Some("https://console.anthropic.com/settings/keys"),
        key_help: "An Anthropic API key.",
        default_model: "claude-haiku-4-5",
        extra_headers: &[("anthropic-version", "2023-06-01")],
    },
    Provider {
        id: "openai",
        name: "OpenAI",
        wire: Wire::OpenAi,
        base_url: "https://api.openai.com/v1",
        base_url_editable: false,
        key: KeyNeed::Required,
        key_url: Some("https://platform.openai.com/api-keys"),
        key_help: "An OpenAI API key.",
        default_model: "gpt-4o-mini",
        extra_headers: &[],
    },
    Provider {
        id: "gemini",
        name: "Gemini",
        wire: Wire::Gemini,
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        base_url_editable: false,
        key: KeyNeed::Required,
        key_url: Some("https://aistudio.google.com/app/apikey"),
        key_help: "A Google AI Studio API key.",
        default_model: "gemini-2.0-flash",
        extra_headers: &[],
    },
    Provider {
        id: "openrouter",
        name: "OpenRouter",
        wire: Wire::OpenAi,
        base_url: "https://openrouter.ai/api/v1",
        base_url_editable: false,
        key: KeyNeed::Required,
        key_url: Some("https://openrouter.ai/keys"),
        key_help: "An OpenRouter API key. One key reaches many models.",
        default_model: "openai/gpt-4o-mini",
        extra_headers: &[("X-Title", "Posthaste")],
    },
    Provider {
        id: "ollama",
        name: "Ollama",
        wire: Wire::OpenAi,
        base_url: "http://localhost:11434/v1",
        base_url_editable: true,
        key: KeyNeed::None,
        key_url: None,
        key_help: "Runs on this computer. Notes never leave it.",
        default_model: "llama3.2",
        extra_headers: &[],
    },
    Provider {
        id: "custom",
        name: "OpenAI-compatible",
        wire: Wire::OpenAi,
        base_url: "",
        base_url_editable: true,
        key: KeyNeed::Optional,
        key_url: None,
        key_help: "Any server that speaks the OpenAI chat format: LM Studio, vLLM, DeepSeek, Groq and others.",
        default_model: "",
        extra_headers: &[],
    },
];

pub fn find(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// The name of a provider's keychain entry.
pub fn key_account(id: &str) -> String {
    format!("ai:{id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_none_is_not_a_provider() {
        let mut ids: Vec<_> = PROVIDERS.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PROVIDERS.len());
        assert!(find("none").is_none());
        assert!(find("nope").is_none());
    }

    #[test]
    fn the_ids_stored_before_0_3_2_still_resolve() {
        for id in ["anthropic", "openai", "gemini"] {
            assert!(find(id).is_some(), "{id}");
        }
    }

    #[test]
    fn every_provider_is_usable_as_shipped_or_says_what_it_needs() {
        for p in PROVIDERS {
            if p.base_url.is_empty() || p.default_model.is_empty() {
                assert!(p.base_url_editable, "{} needs an editable address", p.id);
            } else {
                assert!(p.base_url.starts_with("http"), "{}", p.id);
            }
            if p.key == KeyNeed::Required {
                assert!(
                    p.key_url.is_some(),
                    "{} should link to where keys come from",
                    p.id
                );
            }
            assert!(!p.key_help.is_empty(), "{}", p.id);
        }
    }
}
