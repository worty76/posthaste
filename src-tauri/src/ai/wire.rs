//! The three request/reply formats, as pure functions: what to send, how to
//! read the answer, and how to explain a failure. No network here, so every
//! format is tested without one.

use serde_json::{json, Value};

use super::config::Resolved;
use super::providers::Wire;

pub const SYSTEM_PROMPT: &str = "You tidy up quick notes typed in a hurry. Fix spelling and \
grammar, keep the original meaning and language, and keep it short. Reply with ONLY the \
polished note text — no quotes, no explanation, no extra lines.";

/// One HTTP request, ready to send.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpCall {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

pub fn build(r: &Resolved, text: &str) -> HttpCall {
    let mut headers: Vec<(String, String)> = Vec::new();
    let (url, body) = match r.provider.wire {
        Wire::OpenAi => {
            if let Some(key) = &r.key {
                headers.push(("Authorization".into(), format!("Bearer {key}")));
            }
            (
                format!("{}/chat/completions", r.base_url),
                json!({
                    "model": r.model,
                    "messages": [
                        { "role": "system", "content": SYSTEM_PROMPT },
                        { "role": "user", "content": text }
                    ],
                    "stream": false
                }),
            )
        }
        Wire::Anthropic => {
            if let Some(key) = &r.key {
                headers.push(("x-api-key".into(), key.clone()));
            }
            (
                format!("{}/messages", r.base_url),
                json!({
                    "model": r.model,
                    "max_tokens": 1024,
                    "system": SYSTEM_PROMPT,
                    "messages": [{ "role": "user", "content": text }]
                }),
            )
        }
        Wire::Gemini => {
            if let Some(key) = &r.key {
                headers.push(("x-goog-api-key".into(), key.clone()));
            }
            (
                format!(
                    "{}/models/{}:generateContent",
                    r.base_url,
                    path_segment(&r.model)
                ),
                json!({
                    "systemInstruction": { "parts": [{ "text": SYSTEM_PROMPT }] },
                    "contents": [{ "role": "user", "parts": [{ "text": text }] }]
                }),
            )
        }
    };
    for (name, value) in r.provider.extra_headers {
        headers.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
        headers.push(((*name).into(), (*value).into()));
    }
    HttpCall { url, headers, body }
}

/// A model name is put in the Gemini URL path; anything outside the safe
/// set is percent-encoded so it cannot change the path.
fn path_segment(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The polished text from a successful reply, if there is any.
pub fn read_reply(wire: Wire, value: &Value) -> Option<String> {
    let text = match wire {
        Wire::OpenAi => value["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string),
        Wire::Anthropic => value["content"].as_array().and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b["type"] == "text")
                .and_then(|b| b["text"].as_str())
                .map(str::to_string)
        }),
        Wire::Gemini => value["candidates"][0]["content"]["parts"]
            .as_array()
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p["text"].as_str())
                    .collect::<String>()
            }),
    }?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The polished text from a successful reply body. Accepts plain JSON
/// (with a byte order mark or padding around it) and, for the OpenAI
/// format, a streamed reply (`data: …` lines) from servers that stream even
/// when asked not to.
pub fn read_body(wire: Wire, body: &str) -> Option<String> {
    let body = body.trim_start_matches('\u{feff}').trim();
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        return read_reply(wire, &value);
    }
    if wire != Wire::OpenAi || !body.starts_with("data:") {
        return None;
    }
    let text: String = body
        .lines()
        .filter_map(|line| line.trim().strip_prefix("data:"))
        .map(str::trim)
        .filter(|data| *data != "[DONE]")
        .filter_map(|data| serde_json::from_str::<Value>(data).ok())
        .filter_map(|chunk| {
            let choice = &chunk["choices"][0];
            choice["delta"]["content"]
                .as_str()
                .or_else(|| choice["message"]["content"].as_str())
                .map(str::to_string)
        })
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// A successful reply that held no readable answer, with enough of what
/// came back to tell what went wrong. Provider replies never contain the
/// key, so the snippet is safe to show.
pub fn describe_unreadable(provider: &str, status: u16, content_type: &str, body: &str) -> String {
    let snippet = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if snippet.is_empty() {
        return format!("{provider} sent an empty answer (HTTP {status}).");
    }
    let kind = if content_type.is_empty() {
        String::new()
    } else {
        format!(", {content_type}")
    };
    format!(
        "{provider} sent an answer this app could not read (HTTP {status}{kind}: \"{}\").",
        shorten(&snippet, 100)
    )
}

/// A failed reply, in words the user can act on. Includes the provider's
/// own message when it sends one, shortened.
pub fn describe_failure(provider: &str, status: u16, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v["error"]["message"]
                .as_str()
                .or_else(|| v["message"].as_str())
                .or_else(|| v["error"].as_str())
                .map(str::to_string)
        })
        .map(|m| shorten(m.trim(), 200))
        .filter(|m| !m.is_empty());
    let summary = match status {
        401 | 403 => format!("{provider} did not accept the key. Check it in Settings."),
        404 => format!("{provider} could not find that model or address. Check them in Settings."),
        429 => format!("{provider} is limiting requests right now. Try again in a moment."),
        500..=599 => format!("{provider} had a problem (HTTP {status}). Try again later."),
        _ => format!("{provider} returned an error (HTTP {status})."),
    };
    match detail {
        Some(d) => format!("{summary} ({d})"),
        None => summary,
    }
}

fn shorten(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max_chars).collect();
        format!("{}…", cut.trim_end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::providers::find;

    fn resolved(id: &str, key: Option<&str>, model: &str, base: &str) -> Resolved {
        Resolved {
            provider: find(id).unwrap(),
            base_url: base.into(),
            model: model.into(),
            key: key.map(str::to_string),
        }
    }

    fn header<'a>(call: &'a HttpCall, name: &str) -> Option<&'a str> {
        call.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn openai_format_sends_the_prompt_and_note_with_a_bearer_key() {
        let call = build(
            &resolved(
                "openai",
                Some("sk-1"),
                "gpt-4o-mini",
                "https://api.openai.com/v1",
            ),
            "my rough note",
        );
        assert_eq!(call.url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(header(&call, "Authorization"), Some("Bearer sk-1"));
        assert_eq!(call.body["model"], "gpt-4o-mini");
        assert_eq!(call.body["messages"][0]["content"], SYSTEM_PROMPT);
        assert_eq!(call.body["messages"][1]["content"], "my rough note");
        assert_eq!(call.body["stream"], false);
    }

    #[test]
    fn a_keyless_server_gets_no_auth_header() {
        let call = build(
            &resolved("ollama", None, "llama3.2", "http://localhost:11434/v1"),
            "n",
        );
        assert_eq!(call.url, "http://localhost:11434/v1/chat/completions");
        assert_eq!(header(&call, "Authorization"), None);
    }

    #[test]
    fn anthropic_format_uses_its_own_header_and_system_field() {
        let call = build(
            &resolved(
                "anthropic",
                Some("sk-ant"),
                "claude-haiku-4-5",
                "https://api.anthropic.com/v1",
            ),
            "my rough note",
        );
        assert_eq!(call.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(header(&call, "x-api-key"), Some("sk-ant"));
        assert_eq!(header(&call, "anthropic-version"), Some("2023-06-01"));
        assert_eq!(header(&call, "Authorization"), None);
        assert_eq!(call.body["system"], SYSTEM_PROMPT);
        assert_eq!(call.body["messages"][0]["content"], "my rough note");
    }

    #[test]
    fn gemini_format_puts_the_model_in_the_path_safely() {
        let base = "https://generativelanguage.googleapis.com/v1beta";
        let call = build(
            &resolved("gemini", Some("g-1"), "gemini-2.0-flash", base),
            "note",
        );
        assert_eq!(
            call.url,
            format!("{base}/models/gemini-2.0-flash:generateContent")
        );
        assert_eq!(header(&call, "x-goog-api-key"), Some("g-1"));
        assert_eq!(
            call.body["systemInstruction"]["parts"][0]["text"],
            SYSTEM_PROMPT
        );
        assert_eq!(call.body["contents"][0]["parts"][0]["text"], "note");

        let odd = build(&resolved("gemini", Some("g"), "../x?y", base), "n");
        assert_eq!(odd.url, format!("{base}/models/..%2Fx%3Fy:generateContent"));
    }

    #[test]
    fn replies_are_read_per_format_and_trimmed() {
        let openai = json!({ "choices": [{ "message": { "content": "  Fixed.\n" } }] });
        assert_eq!(read_reply(Wire::OpenAi, &openai).as_deref(), Some("Fixed."));

        let anthropic = json!({ "content": [
            { "type": "thinking", "thinking": "hmm" },
            { "type": "text", "text": "Fixed." }
        ] });
        assert_eq!(
            read_reply(Wire::Anthropic, &anthropic).as_deref(),
            Some("Fixed.")
        );

        let gemini = json!({ "candidates": [{ "content": { "parts": [
            { "text": "Fix" }, { "text": "ed." }
        ] } }] });
        assert_eq!(read_reply(Wire::Gemini, &gemini).as_deref(), Some("Fixed."));
    }

    #[test]
    fn an_empty_or_odd_reply_reads_as_nothing() {
        for (wire, value) in [
            (
                Wire::OpenAi,
                json!({ "choices": [{ "message": { "content": "   " } }] }),
            ),
            (Wire::OpenAi, json!({ "choices": [] })),
            (
                Wire::Anthropic,
                json!({ "content": [{ "type": "tool_use" }] }),
            ),
            (
                Wire::Gemini,
                json!({ "promptFeedback": { "blockReason": "SAFETY" } }),
            ),
        ] {
            assert_eq!(read_reply(wire, &value), None, "{value}");
        }
    }

    #[test]
    fn failures_say_what_to_do_and_include_the_providers_words() {
        let msg = describe_failure(
            "OpenAI",
            401,
            r#"{"error":{"message":"Incorrect API key provided."}}"#,
        );
        assert_eq!(
            msg,
            "OpenAI did not accept the key. Check it in Settings. (Incorrect API key provided.)"
        );
        assert!(
            describe_failure("Ollama", 404, r#"{"error":"model 'x' not found"}"#)
                .contains("model 'x' not found")
        );
        assert!(
            describe_failure("OpenRouter", 403, r#"{"message":"Forbidden"}"#)
                .contains("(Forbidden)")
        );
        assert_eq!(
            describe_failure("Gemini", 502, "<html>bad gateway</html>"),
            "Gemini had a problem (HTTP 502). Try again later."
        );
        let long = format!(r#"{{"error":{{"message":"{}"}}}}"#, "x".repeat(500));
        assert!(describe_failure("Claude", 400, &long).chars().count() < 280);
    }

    #[test]
    fn a_streamed_openai_reply_is_read_too() {
        let body = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"Fix\"}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"ed.\"}}]}\n\n\
                    data: [DONE]\n";
        assert_eq!(read_body(Wire::OpenAi, body).as_deref(), Some("Fixed."));
    }

    #[test]
    fn a_reply_with_a_byte_order_mark_or_padding_is_read() {
        let body = "\u{feff}\n {\"choices\":[{\"message\":{\"content\":\"Fixed.\"}}]}\n";
        assert_eq!(read_body(Wire::OpenAi, body).as_deref(), Some("Fixed."));
    }

    #[test]
    fn something_unreadable_is_described_with_what_came_back() {
        assert_eq!(read_body(Wire::OpenAi, "<html>Sign in</html>"), None);
        let msg = describe_unreadable(
            "OpenRouter",
            200,
            "text/html; charset=utf-8",
            "<html>\n  <title>Sign in</title>\n</html>",
        );
        assert_eq!(
            msg,
            "OpenRouter sent an answer this app could not read (HTTP 200, text/html; charset=utf-8: \"<html> <title>Sign in</title> </html>\")."
        );
        let empty = describe_unreadable("Ollama", 200, "", "");
        assert_eq!(empty, "Ollama sent an empty answer (HTTP 200).");
        let long = describe_unreadable("X", 200, "text/plain", &"y".repeat(400));
        assert!(long.chars().count() < 200, "{long}");
    }
}
