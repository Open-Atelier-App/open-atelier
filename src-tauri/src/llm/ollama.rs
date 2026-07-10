use super::router::{emit_token, StreamResult};
use super::sse::{friendly_stream_error, LineBuffer};
use crate::error::{AtelierError, ErrorCode, Result};
use futures_util::StreamExt;
use tauri::AppHandle;

pub async fn stream(
    app: &AppHandle,
    message_id: i64,
    base_url: &str,
    model: &str,
    messages: Vec<serde_json::Value>,
) -> Result<StreamResult> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true
    });

    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            AtelierError::new(
                ErrorCode::ProviderUnavailable,
                format!("Cannot reach Ollama at {url}: {e}"),
            )
        })?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(AtelierError::new(
            ErrorCode::ProviderError,
            format!("Ollama error {status}: {body}"),
        ));
    }

    let mut full_content = String::new();
    let mut input_tokens: Option<i64> = None;
    let mut output_tokens: Option<i64> = None;
    let mut finish_reason: Option<String> = None;
    let mut stream = resp.bytes_stream();
    let mut line_buf = LineBuffer::new();

    // Labeled break: a bare `break` here only exits the inner `for`, not
    // this `while` — the same bug found (and fixed) in openai.rs and
    // openai_compatible.rs, where it left the stream waiting on more chunks
    // after the provider had already said it was done.
    'stream: while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            AtelierError::new(
                ErrorCode::ProviderUnavailable,
                friendly_stream_error(&e.to_string()),
            )
        })?;
        for line in line_buf.push_chunk(&chunk) {
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(delta) = v["message"]["content"].as_str() {
                    emit_token(app, message_id, delta);
                    full_content.push_str(delta);
                }
                if v["done"].as_bool().unwrap_or(false) {
                    // Ollama's final chunk includes prompt_eval_count / eval_count,
                    // which map to input/output token counts respectively.
                    if let Some(n) = v["prompt_eval_count"].as_i64() {
                        input_tokens = Some(n);
                    }
                    if let Some(n) = v["eval_count"].as_i64() {
                        output_tokens = Some(n);
                    }
                    if let Some(r) = v["done_reason"].as_str() {
                        finish_reason = Some(r.to_string());
                    }
                    break 'stream;
                }
            }
        }
    }

    Ok(StreamResult {
        content: full_content,
        input_tokens,
        output_tokens,
        finish_reason,
    })
}
