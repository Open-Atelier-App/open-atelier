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
    use_responses_api: bool,
) -> Result<StreamResult> {
    // A connection that never completes (dead proxy, hung TLS handshake, etc.)
    // would otherwise leave the chat message stuck "streaming" forever with no
    // error ever surfaced, since reqwest has no timeout by default.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_default();
    let mut full_content = String::new();
    let mut input_tokens: Option<i64> = None;
    let mut output_tokens: Option<i64> = None;
    let mut finish_reason: Option<String> = None;

    if use_responses_api {
        // Responses API for codex-mini-latest
        let prompt = messages.iter()
            .map(|m| format!("{}: {}", m["role"].as_str().unwrap_or(""), m["content"].as_str().unwrap_or("")))
            .collect::<Vec<_>>().join("\n");

        let body = serde_json::json!({
            "model": model,
            "input": prompt,
            "stream": true
        });

        let resp = client
            .post("https://api.openai.com/v1/responses")
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await
            .map_err(|e| AtelierError::new(ErrorCode::ProviderUnavailable, e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(map_openai_error(status.as_u16(), &body));
        }

        let mut stream = resp.bytes_stream();
        let mut line_buf = LineBuffer::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AtelierError::new(ErrorCode::ProviderUnavailable, e.to_string()))?;
            for line in line_buf.push_chunk(&chunk) {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" { break; }
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(delta) = v["delta"]["text"].as_str() {
                            emit_token(app, message_id, delta);
                            full_content.push_str(delta);
                        }
                        // Responses API streaming emits a "response.completed" event
                        // with usage on the final response object, when present.
                        if let Some(n) = v["response"]["usage"]["input_tokens"].as_i64() {
                            input_tokens = Some(n);
                        }
                        if let Some(n) = v["response"]["usage"]["output_tokens"].as_i64() {
                            output_tokens = Some(n);
                        }
                    }
                }
            }
        }
    } else {
        // Chat completions API
        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true }
        });

        let resp = client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await
            .map_err(|e| AtelierError::new(ErrorCode::ProviderUnavailable, e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(map_openai_error(status.as_u16(), &body));
        }

        let mut stream = resp.bytes_stream();
        let mut line_buf = LineBuffer::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AtelierError::new(ErrorCode::ProviderUnavailable, e.to_string()))?;
            for line in line_buf.push_chunk(&chunk) {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" { break; }
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                            emit_token(app, message_id, delta);
                            full_content.push_str(delta);
                        }
                        if let Some(r) = v["choices"][0]["finish_reason"].as_str() {
                            finish_reason = Some(r.to_string());
                        }
                        // With stream_options.include_usage=true, the final chunk
                        // carries a top-level "usage" object (and an empty choices array).
                        if let Some(n) = v["usage"]["prompt_tokens"].as_i64() {
                            input_tokens = Some(n);
                        }
                        if let Some(n) = v["usage"]["completion_tokens"].as_i64() {
                            output_tokens = Some(n);
                        }
                    }
                }
            }
        }
    }

    Ok(StreamResult { content: full_content, input_tokens, output_tokens, finish_reason })
}

fn map_openai_error(status: u16, body: &str) -> AtelierError {
    match status {
        401 => AtelierError::new(ErrorCode::ProviderUnauthorized, "Invalid OpenAI API key"),
        429 => AtelierError::new(ErrorCode::ProviderRateLimited, "OpenAI rate limit exceeded"),
        _ => AtelierError::new(ErrorCode::ProviderError, format!("OpenAI error {status}: {body}")),
    }
}
