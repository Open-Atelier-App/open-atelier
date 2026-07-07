use tauri::{AppHandle, Emitter};
use crate::commands::cred_store::store_get;
use crate::error::{AtelierError, ErrorCode, Result};
use crate::models::ChatToken;
use super::{openai, anthropic, google, ollama, openai_compatible};

const SERVICE_NAME: &str = "com.openatelier.app";

/// Result of a streaming chat call: the full assembled text plus whatever
/// token usage the provider's API surfaced (not all providers report this,
/// so both fields are optional).
#[derive(Debug, Clone, Default)]
pub struct StreamResult {
    pub content: String,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    /// Why the provider stopped generating, in whatever vocabulary that
    /// provider uses ("length"/"stop" for OpenAI-shaped APIs, "max_tokens"/
    /// "end_turn" for Anthropic, "MAX_TOKENS"/"STOP" for Gemini, ollama's
    /// `done_reason`). Not normalized across providers — callers that care
    /// use `is_truncated` below, which just checks for the token-limit case
    /// case-insensitively regardless of which provider's spelling it is.
    pub finish_reason: Option<String>,
}

/// True if `finish_reason` indicates the provider stopped because it hit
/// its output token limit, not because the model naturally finished. This
/// is the single most useful piece of context for diagnosing a truncated,
/// malformed trigger (e.g. "Unterminated trigger: missing ]<<<") after the
/// fact — it tells you the model didn't make a syntax mistake, it just ran
/// out of room mid-write.
pub fn is_truncated(finish_reason: &Option<String>) -> bool {
    finish_reason.as_deref()
        .map(|r| {
            let r = r.to_lowercase();
            r.contains("length") || r.contains("max_token")
        })
        .unwrap_or(false)
}

/// Fetch a stored API key/value for `provider` from the local encrypted
/// credential store (see `cred_store`) — Atelier never uses the OS keychain.
fn get_key(provider: &str) -> Result<String> {
    match store_get(SERVICE_NAME, provider)? {
        Some((value, _backend)) => Ok(value),
        None => Err(AtelierError::new(
            ErrorCode::ProviderUnauthorized,
            format!("No API key configured for {provider}. Add one in Settings."),
        )),
    }
}

/// An empty-content assistant turn (e.g. a response that was 100% triggers,
/// nothing else) carries no information for the model to see again, and
/// some providers (Mistral's OpenAI-compatible endpoint, at least) reject it
/// outright with a 400 rather than silently accepting it, since we never
/// send `tool_calls` either. Dropping it here is a last line of defense in
/// case some other history-building path lets one through.
fn build_messages(history: &[(String, String)]) -> Vec<serde_json::Value> {
    history.iter()
        .filter(|(role, content)| role != "assistant" || !content.trim().is_empty())
        .map(|(role, content)| serde_json::json!({ "role": role, "content": content }))
        .collect()
}

pub async fn stream_chat(
    app: &AppHandle,
    message_id: i64,
    provider: &str,
    model: &str,
    history: Vec<(String, String)>,
) -> Result<StreamResult> {
    let messages = build_messages(&history);

    match provider {
        "openai" => {
            let key = get_key("openai")?;
            openai::stream(app, message_id, &key, model, messages, false).await
        }
        "openai-codex" => {
            let key = get_key("openai")?; // same key, different endpoint
            openai::stream(app, message_id, &key, model, messages, true).await
        }
        "anthropic" => {
            let key = get_key("anthropic")?;
            anthropic::stream(app, message_id, &key, model, messages).await
        }
        "google" => {
            let key = get_key("google")?;
            google::stream(app, message_id, &key, model, messages).await
        }
        "ollama" => {
            let base_url = get_key("ollama").unwrap_or_else(|_| "http://localhost:11434".to_string());
            ollama::stream(app, message_id, &base_url, model, messages).await
        }
        other => {
            if let Some(base_url) = openai_compatible::base_url_for(other) {
                let key = get_key(other)?;
                // OpenRouter asks integrators to identify their app via these
                // headers; harmless no-ops for the other OpenAI-compatible vendors.
                let extra_headers: &[(&str, &str)] = if other == "openrouter" {
                    &[("HTTP-Referer", "https://github.com/acleefr/open-atelier"), ("X-Title", "Open Atelier")]
                } else {
                    &[]
                };
                openai_compatible::stream(app, message_id, base_url, &key, model, messages, extra_headers).await
            } else {
                Err(AtelierError::new(ErrorCode::Unsupported, format!("Unknown provider: {other}")))
            }
        }
    }
}

pub fn emit_token(app: &AppHandle, message_id: i64, delta: &str) {
    if message_id < 0 { return; } // sentinel for background calls (auto-title)
    let _ = app.emit("chat://token", ChatToken {
        message_id,
        delta: delta.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_empty_content_assistant_turns() {
        let history = vec![
            ("system".to_string(), "context".to_string()),
            ("user".to_string(), "read that file".to_string()),
            ("assistant".to_string(), "".to_string()),
            ("system".to_string(), "RESULT ...".to_string()),
        ];
        let messages = build_messages(&history);
        let roles: Vec<&str> = messages.iter().map(|m| m["role"].as_str().unwrap()).collect();
        assert_eq!(roles, vec!["system", "user", "system"]);
    }

    #[test]
    fn drops_whitespace_only_assistant_turns() {
        let history = vec![
            ("user".to_string(), "hi".to_string()),
            ("assistant".to_string(), "   \n  ".to_string()),
        ];
        let messages = build_messages(&history);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
    }

    #[test]
    fn keeps_non_empty_assistant_turns() {
        let history = vec![
            ("user".to_string(), "hi".to_string()),
            ("assistant".to_string(), "hello there".to_string()),
        ];
        let messages = build_messages(&history);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1]["content"], "hello there");
    }

    #[test]
    fn keeps_empty_content_for_non_assistant_roles() {
        // Only assistant turns are the ones providers reject on empty
        // content (no tool_calls to fall back on); an empty user/system
        // message is unusual but not the bug being guarded against here.
        let history = vec![("user".to_string(), "".to_string())];
        let messages = build_messages(&history);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn is_truncated_recognizes_openai_shaped_length() {
        assert!(is_truncated(&Some("length".to_string())));
    }

    #[test]
    fn is_truncated_recognizes_anthropic_max_tokens() {
        assert!(is_truncated(&Some("max_tokens".to_string())));
    }

    #[test]
    fn is_truncated_recognizes_gemini_uppercase_max_tokens() {
        assert!(is_truncated(&Some("MAX_TOKENS".to_string())));
    }

    #[test]
    fn is_truncated_false_for_normal_stop() {
        assert!(!is_truncated(&Some("stop".to_string())));
        assert!(!is_truncated(&Some("end_turn".to_string())));
        assert!(!is_truncated(&None));
    }
}
