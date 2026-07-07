use tauri::State;
use crate::db::{Db, now_ms};
use crate::error::{AtelierError, Result};
use crate::models::KeyStatus;
use super::cred_store::{
    store_set, store_get, store_delete, store_backend,
    profile_store_set, profile_store_get, profile_store_delete, profile_store_backend,
    migrate_global_to_profile,
};

pub(crate) const SERVICE_NAME: &str = "com.openatelier.app";

/// Credential types supported beyond a plain API key.
/// Stored in keychain under "{SERVICE_NAME}/{provider}/{cred_type}".
/// Supported cred_type values:
///   "api_key"      — classic secret key (default)
///   "bearer_token" — OAuth2 / JWT bearer
///   "azure"        — Azure OpenAI: value is JSON {"endpoint":"...","key":"...","deployment":"..."}
///   "ollama_url"   — Ollama base URL (no secret, stored for convenience)
pub(crate) fn cred_keyring_name(provider: &str, cred_type: &str) -> String {
    format!("{SERVICE_NAME}/{provider}/{cred_type}")
}

#[tauri::command]
pub fn cred_save(provider: String, cred_type: String, value: String) -> Result<()> {
    let name = cred_keyring_name(&provider, &cred_type);
    store_set(SERVICE_NAME, &name, &value)?;
    Ok(())
}

#[tauri::command]
pub fn cred_delete(provider: String, cred_type: String) -> Result<()> {
    let name = cred_keyring_name(&provider, &cred_type);
    store_delete(SERVICE_NAME, &name)
}

#[tauri::command]
pub fn cred_get(provider: String, cred_type: String) -> Result<Option<String>> {
    let name = cred_keyring_name(&provider, &cred_type);
    Ok(store_get(SERVICE_NAME, &name)?.map(|(v, _backend)| v))
}

/// Like `cred_get`, but also reports which backend the credential is stored
/// in (e.g. so the frontend can show a "stored in local fallback, not OS
/// keychain" hint).
#[tauri::command]
pub fn cred_get_with_backend(provider: String, cred_type: String) -> Result<Option<(String, String)>> {
    let name = cred_keyring_name(&provider, &cred_type);
    Ok(store_get(SERVICE_NAME, &name)?.map(|(v, backend)| (v, backend.as_str().to_string())))
}

/// Returns only the last 3 characters of a credential, masked.
#[tauri::command]
pub fn cred_get_masked(provider: String, cred_type: String, profile_id: Option<i64>) -> Result<Option<String>> {
    let name = cred_keyring_name(&provider, &cred_type);
    let raw = if let Some(pid) = profile_id {
        profile_store_get(pid, SERVICE_NAME, &name)?.map(|(v, _)| v)
    } else {
        store_get(SERVICE_NAME, &name)?.map(|(v, _)| v)
    };
    Ok(raw.map(|v| {
        if v.len() <= 3 {
            v
        } else {
            let suffix = &v[v.len() - 3..];
            format!("{}{suffix}", "\u{2022}".repeat(8))
        }
    }))
}

// ── Per-profile credential commands ─────────────────────────────────────

#[tauri::command]
pub fn cred_save_profile(provider: String, cred_type: String, value: String, profile_id: i64) -> Result<()> {
    let name = cred_keyring_name(&provider, &cred_type);
    profile_store_set(profile_id, SERVICE_NAME, &name, &value)?;
    Ok(())
}

#[tauri::command]
pub fn cred_delete_profile(provider: String, cred_type: String, profile_id: i64) -> Result<()> {
    let name = cred_keyring_name(&provider, &cred_type);
    profile_store_delete(profile_id, SERVICE_NAME, &name)
}

#[tauri::command]
pub fn cred_get_profile(provider: String, cred_type: String, profile_id: i64) -> Result<Option<String>> {
    let name = cred_keyring_name(&provider, &cred_type);
    Ok(profile_store_get(profile_id, SERVICE_NAME, &name)?.map(|(v, _)| v))
}

#[tauri::command]
pub fn cred_get_with_backend_profile(provider: String, cred_type: String, profile_id: i64) -> Result<Option<(String, String)>> {
    let name = cred_keyring_name(&provider, &cred_type);
    Ok(profile_store_get(profile_id, SERVICE_NAME, &name)?.map(|(v, backend)| (v, backend.as_str().to_string())))
}

#[tauri::command]
pub fn key_list_status_profile(profile_id: i64) -> Result<Vec<KeyStatus>> {
    let providers = [
        "openai", "openai-codex", "anthropic", "google", "ollama",
        "groq", "openrouter", "mistral", "together", "deepseek", "xai",
    ];
    let statuses = providers.iter().map(|p| {
        let cred_name = cred_keyring_name(p, "api_key");
        // Check: profile-scoped cred format, then profile-scoped legacy, then global cred format, then global legacy
        let backend = profile_store_backend(profile_id, SERVICE_NAME, &cred_name)
            .or_else(|| profile_store_backend(profile_id, SERVICE_NAME, p))
            .or_else(|| store_backend(SERVICE_NAME, &cred_name))
            .or_else(|| store_backend(SERVICE_NAME, p));
        match backend {
            Some(b) => KeyStatus { provider: p.to_string(), exists: true, backend: b.as_str().to_string() },
            None => KeyStatus { provider: p.to_string(), exists: false, backend: "none".to_string() },
        }
    }).collect();
    Ok(statuses)
}

#[tauri::command]
pub fn cred_migrate_to_profile(profile_id: i64) -> Result<()> {
    migrate_global_to_profile(profile_id)
}

// ── Legacy global key commands ─────────────────────────────────────────

#[tauri::command]
pub fn key_save(provider: String, key: String, db: State<Db>) -> Result<()> {
    let _ = db; // db not needed for credential ops
    store_set(SERVICE_NAME, &provider, &key)?;
    Ok(())
}

#[tauri::command]
pub fn key_delete(provider: String) -> Result<()> {
    store_delete(SERVICE_NAME, &provider)
}

#[tauri::command]
pub fn key_get(provider: String) -> Result<Option<String>> {
    Ok(store_get(SERVICE_NAME, &provider)?.map(|(v, _backend)| v))
}

/// Internal helper: test a key value against a provider's API.
async fn test_provider_key(provider: &str, key: &str) -> Result<bool> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AtelierError::internal(e.to_string()))?;

    let ok = match provider {
        "openai" | "openai-codex" => {
            let resp = client
                .get("https://api.openai.com/v1/models")
                .header("Authorization", format!("Bearer {key}"))
                .send().await;
            resp.map(|r| r.status().is_success()).unwrap_or(false)
        }
        "anthropic" => {
            let resp = client
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .send().await;
            resp.map(|r| r.status().is_success()).unwrap_or(false)
        }
        "google" => {
            let resp = client
                .get(format!("https://generativelanguage.googleapis.com/v1beta/models?key={key}"))
                .send().await;
            resp.map(|r| r.status().is_success()).unwrap_or(false)
        }
        "ollama" => {
            let base = if key.starts_with("http") { key.to_string() } else { "http://localhost:11434".to_string() };
            let resp = client.get(format!("{base}/api/tags")).send().await;
            resp.map(|r| r.status().is_success()).unwrap_or(false)
        }
        "groq" => {
            let resp = client
                .get("https://api.groq.com/openai/v1/models")
                .header("Authorization", format!("Bearer {key}"))
                .send().await;
            resp.map(|r| r.status().is_success()).unwrap_or(false)
        }
        "mistral" => {
            let resp = client
                .get("https://api.mistral.ai/v1/models")
                .header("Authorization", format!("Bearer {key}"))
                .send().await;
            resp.map(|r| r.status().is_success()).unwrap_or(false)
        }
        "xai" => {
            let resp = client
                .get("https://api.x.ai/v1/models")
                .header("Authorization", format!("Bearer {key}"))
                .send().await;
            resp.map(|r| r.status().is_success()).unwrap_or(false)
        }
        "deepseek" => {
            let resp = client
                .get("https://api.deepseek.com/models")
                .header("Authorization", format!("Bearer {key}"))
                .send().await;
            resp.map(|r| r.status().is_success()).unwrap_or(false)
        }
        "together" => {
            let resp = client
                .get("https://api.together.xyz/v1/models")
                .header("Authorization", format!("Bearer {key}"))
                .send().await;
            resp.map(|r| r.status().is_success()).unwrap_or(false)
        }
        "openrouter" => {
            let resp = client
                .get("https://openrouter.ai/api/v1/models")
                .header("Authorization", format!("Bearer {key}"))
                .send().await;
            resp.map(|r| r.status().is_success()).unwrap_or(false)
        }
        "openai-azure" => {
            // Azure: key is JSON {"endpoint":"...","key":"...","deployment":"..."}
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(key) {
                let endpoint = parsed["endpoint"].as_str().unwrap_or("");
                let api_key = parsed["key"].as_str().unwrap_or("");
                if endpoint.is_empty() || api_key.is_empty() {
                    false
                } else {
                    let url = format!("{endpoint}/openai/models?api-version=2024-06-01");
                    let resp = client
                        .get(&url)
                        .header("api-key", api_key)
                        .send().await;
                    resp.map(|r| r.status().is_success()).unwrap_or(false)
                }
            } else {
                false
            }
        }
        _ => false,
    };
    Ok(ok)
}

#[tauri::command]
pub async fn key_test(provider: String) -> Result<bool> {
    // Try profile-scoped credential first (cred_keyring_name format), then legacy global
    let cred_name = cred_keyring_name(&provider, "api_key");
    let key = match store_get(SERVICE_NAME, &cred_name) {
        Ok(Some((k, _))) => k,
        _ => match store_get(SERVICE_NAME, &provider) {
            Ok(Some((k, _))) => k,
            _ => return Ok(false),
        },
    };
    test_provider_key(&provider, &key).await
}

#[tauri::command]
pub async fn key_test_with_value(provider: String, value: String) -> Result<bool> {
    test_provider_key(&provider, &value).await
}

#[tauri::command]
pub async fn key_test_profile(provider: String, profile_id: i64) -> Result<bool> {
    let cred_name = cred_keyring_name(&provider, "api_key");
    let key = match profile_store_get(profile_id, SERVICE_NAME, &cred_name) {
        Ok(Some((k, _))) => k,
        _ => match store_get(SERVICE_NAME, &cred_name) {
            Ok(Some((k, _))) => k,
            _ => match store_get(SERVICE_NAME, &provider) {
                Ok(Some((k, _))) => k,
                _ => return Ok(false),
            },
        },
    };
    test_provider_key(&provider, &key).await
}

#[tauri::command]
pub fn key_list_status() -> Result<Vec<KeyStatus>> {
    let providers = [
        "openai", "openai-codex", "anthropic", "google", "ollama",
        "groq", "openrouter", "mistral", "together", "deepseek", "xai",
    ];
    let statuses = providers.iter().map(|p| {
        let cred_name = cred_keyring_name(p, "api_key");
        let backend = store_backend(SERVICE_NAME, &cred_name)
            .or_else(|| store_backend(SERVICE_NAME, p));
        match backend {
            Some(b) => KeyStatus { provider: p.to_string(), exists: true, backend: b.as_str().to_string() },
            None => KeyStatus { provider: p.to_string(), exists: false, backend: "none".to_string() },
        }
    }).collect();
    Ok(statuses)
}

#[tauri::command]
pub fn settings_get(key: String, db: State<Db>) -> Result<serde_json::Value> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    // Settings stored in a simple key-value table (created lazily)
    db.execute_batch("CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)").ok();
    let result: rusqlite::Result<String> = db.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [&key],
        |r| r.get(0),
    );
    match result {
        Ok(v) => Ok(serde_json::from_str(&v).unwrap_or(serde_json::Value::Null)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(serde_json::Value::Null),
        Err(e) => Err(e.into()),
    }
}

#[tauri::command]
pub fn factory_reset(db: State<Db>) -> Result<()> {
    // Drop all data from all tables (order matters for FK constraints).
    let conn = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    conn.execute_batch("
        DELETE FROM tool_calls;
        DELETE FROM citations;
        DELETE FROM saved_documents;
        DELETE FROM chunk_embeddings;
        DELETE FROM chunks;
        DELETE FROM files;
        DELETE FROM messages;
        DELETE FROM conversations;
        DELETE FROM workspaces;
        DELETE FROM window_state;
        DELETE FROM profiles;
    ").map_err(|e| AtelierError::db(format!("Reset failed: {e}")))?;

    // Clear app_settings if table exists.
    conn.execute_batch("DELETE FROM app_settings;").ok();

    // Delete credential files.
    if let Some(data_dir) = dirs::data_dir() {
        let app_dir = data_dir.join("com.openatelier.app");
        let _ = std::fs::remove_file(app_dir.join("credentials.enc.json"));
        let _ = std::fs::remove_file(app_dir.join(".cred_master_key"));
        let _ = std::fs::remove_dir_all(app_dir.join("credentials"));
    }

    Ok(())
}

#[tauri::command]
pub fn settings_set(key: String, value: serde_json::Value, db: State<Db>) -> Result<()> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)").ok();
    let v = serde_json::to_string(&value)?;
    db.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key, v],
    )?;
    Ok(())
}
