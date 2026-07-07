/// Embedding generation for semantic search.
/// Supports OpenAI text-embedding-3-small (1536-dim, truncated to 768)
/// and Ollama nomic-embed-text (768-dim).
///
/// Falls back gracefully: if no embedder configured, BM25-only search continues.

use crate::error::{AtelierError, ErrorCode, Result};

pub const EMBED_DIM: usize = 768;

pub async fn embed_texts(
    texts: &[&str],
    provider: &str,
    api_key: &str,
) -> Result<Vec<Vec<f32>>> {
    match provider {
        "openai" => embed_openai(texts, api_key).await,
        "ollama" => embed_ollama(texts, api_key).await,
        _ => Err(AtelierError::new(ErrorCode::Unsupported, format!("Unknown embedding provider: {provider}"))),
    }
}

async fn embed_openai(texts: &[&str], api_key: &str) -> Result<Vec<Vec<f32>>> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": "text-embedding-3-small",
        "input": texts,
        "dimensions": EMBED_DIM
    });

    let resp = client
        .post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send().await
        .map_err(|e| AtelierError::new(ErrorCode::EmbeddingFailed, e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(AtelierError::new(ErrorCode::EmbeddingFailed, format!("OpenAI embed {status}: {body}")));
    }

    let json: serde_json::Value = resp.json().await
        .map_err(|e| AtelierError::new(ErrorCode::EmbeddingFailed, e.to_string()))?;

    let embeddings = json["data"].as_array()
        .ok_or_else(|| AtelierError::new(ErrorCode::EmbeddingFailed, "no data in response"))?
        .iter()
        .map(|item| {
            item["embedding"].as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect::<Vec<f32>>()
        })
        .collect();

    Ok(embeddings)
}

async fn embed_ollama(texts: &[&str], base_url: &str) -> Result<Vec<Vec<f32>>> {
    let client = reqwest::Client::new();
    let base = if base_url.starts_with("http") { base_url } else { "http://localhost:11434" };
    let url = format!("{}/api/embed", base.trim_end_matches('/'));

    let mut all = Vec::new();
    for text in texts {
        let body = serde_json::json!({
            "model": "nomic-embed-text",
            "input": text
        });
        let resp = client
            .post(&url)
            .json(&body)
            .send().await
            .map_err(|e| AtelierError::new(ErrorCode::EmbeddingFailed, format!("Ollama embed: {e}")))?;

        if !resp.status().is_success() {
            return Err(AtelierError::new(ErrorCode::EmbeddingFailed, "Ollama embedding failed"));
        }

        let json: serde_json::Value = resp.json().await
            .map_err(|e| AtelierError::new(ErrorCode::EmbeddingFailed, e.to_string()))?;

        let vec: Vec<f32> = json["embeddings"][0].as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();

        // Truncate or pad to EMBED_DIM
        let mut padded = vec;
        padded.resize(EMBED_DIM, 0.0);
        all.push(padded);
    }
    Ok(all)
}
