use futures_util::StreamExt;
use tauri::AppHandle;
use crate::error::{AtelierError, ErrorCode, Result};
use super::router::{emit_token, StreamResult};
use super::sse::LineBuffer;

/// Generic client for the many vendors that expose an OpenAI-shaped
/// `/chat/completions` endpoint (Groq, OpenRouter, Mistral, Together AI,
/// DeepSeek, ...). Only the base URL and an optional extra header differ
/// between them, so one implementation covers all of them instead of a
/// near-duplicate module per vendor.
pub async fn stream(
    app: &AppHandle,
    message_id: i64,
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: Vec<serde_json::Value>,
    extra_headers: &[(&str, &str)],
) -> Result<StreamResult> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_default();

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let mut req = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json");
    for (k, v) in extra_headers {
        req = req.header(*k, *v);
    }

    let resp = req
        .json(&body)
        .send().await
        .map_err(|e| AtelierError::new(ErrorCode::ProviderUnavailable, e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(match status {
            401 | 403 => AtelierError::new(ErrorCode::ProviderUnauthorized, format!("Invalid API key for {base_url}")),
            429 => AtelierError::new(ErrorCode::ProviderRateLimited, "Rate limit exceeded"),
            _ => AtelierError::new(ErrorCode::ProviderError, format!("Provider error {status}: {text}")),
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
                if data == "[DONE]" { break; }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                        emit_token(app, message_id, delta);
                        full_content.push_str(delta);
                    }
                    if let Some(r) = v["choices"][0]["finish_reason"].as_str() {
                        finish_reason = Some(r.to_string());
                    }
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

    Ok(StreamResult { content: full_content, input_tokens, output_tokens, finish_reason })
}

/// Base URL for each OpenAI-compatible vendor Atelier supports. Adding a
/// new OpenAI-compatible provider only needs an entry here plus a router
/// match arm and frontend model list entries — no new request/response
/// parsing code.
pub fn base_url_for(provider: &str) -> Option<&'static str> {
    match provider {
        "groq" => Some("https://api.groq.com/openai/v1"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "mistral" => Some("https://api.mistral.ai/v1"),
        "together" => Some("https://api.together.xyz/v1"),
        "deepseek" => Some("https://api.deepseek.com/v1"),
        _ => None,
    }
}
