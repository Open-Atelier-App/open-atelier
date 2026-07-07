use futures_util::StreamExt;
use tauri::AppHandle;
use crate::error::{AtelierError, ErrorCode, Result};
use super::router::{emit_token, StreamResult};
use super::sse::LineBuffer;

pub async fn stream(
    app: &AppHandle,
    message_id: i64,
    api_key: &str,
    model: &str,
    messages: Vec<serde_json::Value>,
) -> Result<StreamResult> {
    // A connection that never completes (dead proxy, hung TLS handshake, etc.)
    // would otherwise leave the chat message stuck "streaming" forever with no
    // error ever surfaced, since reqwest has no timeout by default.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_default();

    let system = build_system_field(&messages);

    let chat_messages: Vec<_> = messages.iter()
        .filter(|m| m["role"] != "system")
        .collect();

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": 8192,
        "messages": chat_messages,
        "stream": true
    });
    if let Some(sys) = system {
        body["system"] = serde_json::json!(sys);
    }

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send().await
        .map_err(|e| AtelierError::new(ErrorCode::ProviderUnavailable, e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(match status {
            401 => AtelierError::new(ErrorCode::ProviderUnauthorized, "Invalid Anthropic API key"),
            429 => AtelierError::new(ErrorCode::ProviderRateLimited, "Anthropic rate limit exceeded"),
            _ => AtelierError::new(ErrorCode::ProviderError, format!("Anthropic error {status}: {body}")),
        });
    }

    let mut full_content = String::new();
    let mut input_tokens: Option<i64> = None;
    let mut output_tokens: Option<i64> = None;
    let mut finish_reason: Option<String> = None;
    let mut stream = resp.bytes_stream();
    let mut line_buf = LineBuffer::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AtelierError::new(ErrorCode::ProviderUnavailable, e.to_string()))?;
        for line in line_buf.push_chunk(&chunk) {
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                    let event_type = v["type"].as_str().unwrap_or("");
                    if event_type == "content_block_delta" {
                        if let Some(delta) = v["delta"]["text"].as_str() {
                            emit_token(app, message_id, delta);
                            full_content.push_str(delta);
                        }
                    } else if event_type == "message_start" {
                        if let Some(n) = v["message"]["usage"]["input_tokens"].as_i64() {
                            input_tokens = Some(n);
                        }
                    } else if event_type == "message_delta" {
                        if let Some(n) = v["usage"]["output_tokens"].as_i64() {
                            output_tokens = Some(n);
                        }
                        if let Some(r) = v["delta"]["stop_reason"].as_str() {
                            finish_reason = Some(r.to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(StreamResult { content: full_content, input_tokens, output_tokens, finish_reason })
}

/// Anthropic only accepts a single top-level `system` field, not interleaved
/// system turns. Trigger RESULT/CONTENT feedback is stored as additional
/// 'system' messages mid-conversation (see chat.rs), so taking only the
/// first system message would silently drop every trigger result after turn
/// one. Concatenate all of them instead.
fn build_system_field(messages: &[serde_json::Value]) -> Option<String> {
    let system_parts: Vec<&str> = messages.iter()
        .filter(|m| m["role"] == "system")
        .filter_map(|m| m["content"].as_str())
        .collect();

    if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_system_messages_returns_none() {
        let messages = vec![
            serde_json::json!({ "role": "user", "content": "hi" }),
        ];
        assert_eq!(build_system_field(&messages), None);
    }

    #[test]
    fn single_system_message_passes_through() {
        let messages = vec![
            serde_json::json!({ "role": "system", "content": "You are Atelier." }),
            serde_json::json!({ "role": "user", "content": "hi" }),
        ];
        assert_eq!(build_system_field(&messages), Some("You are Atelier.".to_string()));
    }

    #[test]
    fn multiple_system_messages_are_concatenated_not_dropped() {
        // Regression test: trigger RESULT/CONTENT feedback is stored as
        // additional role='system' messages in the conversation history.
        // Only forwarding the first one to Anthropic silently discarded
        // every subsequent trigger result, so the model never learned
        // whether its CREATE/WRITE actually succeeded.
        let messages = vec![
            serde_json::json!({ "role": "system", "content": "You are Atelier." }),
            serde_json::json!({ "role": "user", "content": "create a file" }),
            serde_json::json!({ "role": "assistant", "content": ">>>[CREATE \"a.ts\"]<<<" }),
            serde_json::json!({ "role": "system", "content": ">>>[RESULT \"CREATE\" \"OK\" \"\"]<<<" }),
            serde_json::json!({ "role": "user", "content": "now write to it" }),
        ];
        let system = build_system_field(&messages).unwrap();
        assert!(system.contains("You are Atelier."));
        assert!(system.contains("RESULT \"CREATE\" \"OK\""));
    }
}
