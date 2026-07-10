use futures_util::StreamExt;
use tauri::AppHandle;
use crate::error::{AtelierError, ErrorCode, Result};
use super::router::{emit_token, StreamResult};
use super::sse::{LineBuffer, friendly_stream_error};

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

    // Gemini uses systemInstruction for system messages, and converts
    // the rest to user/model roles in contents.
    let system_parts: Vec<serde_json::Value> = messages.iter()
        .filter(|m| m["role"] == "system")
        .map(|m| serde_json::json!({ "text": m["content"] }))
        .collect();

    let contents: Vec<serde_json::Value> = messages.iter()
        .filter(|m| m["role"] != "system")
        .map(|m| {
            let role = if m["role"] == "assistant" { "model" } else { "user" };
            serde_json::json!({
                "role": role,
                "parts": [{ "text": m["content"] }]
            })
        })
        .collect();

    let mut body = serde_json::json!({
        "contents": contents,
        "generationConfig": {
            "temperature": 0.7,
            "maxOutputTokens": 8192
        }
    });

    if !system_parts.is_empty() {
        body["systemInstruction"] = serde_json::json!({ "parts": system_parts });
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?key={api_key}&alt=sse"
    );

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send().await
        .map_err(|e| AtelierError::new(ErrorCode::ProviderUnavailable, e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(match status {
            401 | 403 => AtelierError::new(ErrorCode::ProviderUnauthorized, "Invalid Google API key"),
            429 => AtelierError::new(ErrorCode::ProviderRateLimited, "Gemini rate limit exceeded"),
            _ => AtelierError::new(ErrorCode::ProviderError, format!("Gemini error {status}: {body}")),
        });
    }

    let mut full_content = String::new();
    let mut input_tokens: Option<i64> = None;
    let mut output_tokens: Option<i64> = None;
    let mut finish_reason: Option<String> = None;
    let mut stream = resp.bytes_stream();
    let mut line_buf = LineBuffer::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AtelierError::new(ErrorCode::ProviderUnavailable, friendly_stream_error(&e.to_string())))?;
        for line in line_buf.push_chunk(&chunk) {
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(text) = v["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                        emit_token(app, message_id, text);
                        full_content.push_str(text);
                    }
                    if let Some(r) = v["candidates"][0]["finishReason"].as_str() {
                        finish_reason = Some(r.to_string());
                    }
                    // usageMetadata appears on each chunk once available (it
                    // accumulates as generation proceeds), so just keep the
                    // latest values seen.
                    if let Some(n) = v["usageMetadata"]["promptTokenCount"].as_i64() {
                        input_tokens = Some(n);
                    }
                    if let Some(n) = v["usageMetadata"]["candidatesTokenCount"].as_i64() {
                        output_tokens = Some(n);
                    }
                }
            }
        }
    }

    Ok(StreamResult { content: full_content, input_tokens, output_tokens, finish_reason })
}
