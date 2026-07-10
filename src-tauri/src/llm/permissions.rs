use crate::error::{AtelierError, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionLevel {
    pub label: String,
    pub description: String,
    pub allowed_triggers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPermission {
    pub display_name: String,
    pub available_levels: Vec<String>,
    pub default_level: String,
    pub api_mapping: HashMap<String, serde_json::Value>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    pub version: u32,
    pub levels: HashMap<String, PermissionLevel>,
    pub providers: HashMap<String, ProviderPermission>,
}

pub(crate) static CONFIG: Lazy<RwLock<PermissionConfig>> =
    Lazy::new(|| RwLock::new(default_config()));

static ACTIVE_LEVELS: Lazy<RwLock<HashMap<String, String>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

const BUNDLED_CONFIG: &str = include_str!("../../resources/config/llm-permissions.json");

fn default_config() -> PermissionConfig {
    load_config_from_str(BUNDLED_CONFIG).unwrap_or_else(|_| PermissionConfig {
        version: 1,
        levels: {
            let mut m = HashMap::new();
            m.insert(
                "chat_only".into(),
                PermissionLevel {
                    label: "Chat Only".into(),
                    description: "LLM can only send messages. No file operations.".into(),
                    allowed_triggers: vec!["MESSAGE".into()],
                },
            );
            m
        },
        providers: HashMap::new(),
    })
}

pub fn load_config_from_str(json: &str) -> Result<PermissionConfig> {
    let config: PermissionConfig = serde_json::from_str(json).map_err(|e| {
        AtelierError::internal(format!("Failed to parse llm-permissions.json: {e}"))
    })?;

    for (provider_id, provider) in &config.providers {
        for level_id in &provider.available_levels {
            if !config.levels.contains_key(level_id) {
                log::warn!(
                    "Provider '{provider_id}' references unknown level '{level_id}' — will be ignored"
                );
            }
        }
        if !config.levels.contains_key(&provider.default_level) {
            log::warn!(
                "Provider '{provider_id}' has unknown default_level '{}' — falling back to chat_only",
                provider.default_level
            );
        }
    }

    Ok(config)
}

/// Loads the permission config from the bundled resource directory (a real
/// file on disk so a packaged app's admin can edit it post-install without
/// recompiling). Running the debug binary directly rather than through
/// `tauri dev`/`tauri build` leaves `resource_dir()` pointing at a
/// `target/debug/` path that never got the resources copied into it — in
/// that case (or any other read/parse failure) this falls back to the exact
/// same JSON compiled into the binary via `include_str!`, so the app still
/// ends up with a fully working, correct config rather than silently
/// relying on `CONFIG`'s Lazy-static default having already run first.
pub fn load_config(resource_path: &Path) -> Result<()> {
    let json_path = resource_path.join("config").join("llm-permissions.json");
    let json = std::fs::read_to_string(&json_path).unwrap_or_else(|e| {
        log::warn!(
            "Could not read llm-permissions.json at {} ({e}); using the bundled default",
            json_path.display()
        );
        BUNDLED_CONFIG.to_string()
    });

    let config = load_config_from_str(&json)?;

    let mut guard = CONFIG
        .write()
        .map_err(|_| AtelierError::internal("config lock poisoned"))?;
    *guard = config;
    Ok(())
}

pub fn get_config() -> Result<PermissionConfig> {
    let guard = CONFIG
        .read()
        .map_err(|_| AtelierError::internal("config lock poisoned"))?;
    Ok(guard.clone())
}

pub fn get_level_for_provider(provider: &str) -> Result<String> {
    let active = ACTIVE_LEVELS
        .read()
        .map_err(|_| AtelierError::internal("lock"))?;
    if let Some(level) = active.get(provider) {
        return Ok(level.clone());
    }

    let config = get_config()?;
    if let Some(p) = config.providers.get(provider) {
        Ok(p.default_level.clone())
    } else {
        Ok("chat_only".into())
    }
}

pub fn set_level_for_provider(provider: &str, level: &str) -> Result<Vec<String>> {
    let config = get_config()?;

    let level_def = config
        .levels
        .get(level)
        .ok_or_else(|| AtelierError::internal(format!("Unknown permission level: {level}")))?;

    if let Some(p) = config.providers.get(provider) {
        if !p.available_levels.contains(&level.to_string()) {
            return Err(AtelierError::internal(format!(
                "Level '{level}' not available for provider '{provider}'"
            )));
        }
    }

    let mut active = ACTIVE_LEVELS
        .write()
        .map_err(|_| AtelierError::internal("lock"))?;
    active.insert(provider.to_string(), level.to_string());

    Ok(level_def.allowed_triggers.clone())
}

pub fn allowed_triggers_for_level(level: &str) -> Result<Vec<String>> {
    let config = get_config()?;
    if let Some(l) = config.levels.get(level) {
        Ok(l.allowed_triggers.clone())
    } else {
        Ok(vec!["MESSAGE".into()])
    }
}

pub fn level_label(level: &str) -> Result<String> {
    let config = get_config()?;
    if let Some(l) = config.levels.get(level) {
        Ok(l.label.clone())
    } else {
        Ok("Chat Only".into())
    }
}

// ── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_permission_config() -> Result<PermissionConfig> {
    get_config()
}

#[tauri::command]
pub fn get_permission_level(provider: String) -> Result<String> {
    get_level_for_provider(&provider)
}

#[tauri::command]
pub fn set_permission_level(provider: String, level: String) -> Result<Vec<String>> {
    set_level_for_provider(&provider, &level)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_JSON: &str = r#"{
        "version": 1,
        "levels": {
            "chat_only": {
                "label": "Chat Only",
                "description": "Messages only.",
                "allowed_triggers": ["MESSAGE"]
            },
            "full_access": {
                "label": "Full Access",
                "description": "All triggers.",
                "allowed_triggers": ["MESSAGE", "CREATE", "DELETE", "WRITE"]
            }
        },
        "providers": {
            "openai": {
                "display_name": "OpenAI",
                "available_levels": ["chat_only", "full_access"],
                "default_level": "full_access",
                "api_mapping": {},
                "notes": null
            }
        }
    }"#;

    #[test]
    fn parses_valid_config() {
        let config = load_config_from_str(VALID_JSON).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.levels.len(), 2);
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers["openai"].default_level, "full_access");
    }

    #[test]
    fn rejects_malformed_json() {
        let result = load_config_from_str("{ not valid json }}}");
        assert!(result.is_err());
    }

    #[test]
    fn allowed_triggers_returns_correct_list() {
        // Load the test config into the global state
        let config = load_config_from_str(VALID_JSON).unwrap();
        let mut guard = CONFIG.write().unwrap();
        *guard = config;
        drop(guard);

        let triggers = allowed_triggers_for_level("full_access").unwrap();
        assert_eq!(triggers, vec!["MESSAGE", "CREATE", "DELETE", "WRITE"]);
    }

    #[test]
    fn chat_only_fallback_for_unknown_level() {
        let triggers = allowed_triggers_for_level("nonexistent").unwrap();
        assert_eq!(triggers, vec!["MESSAGE"]);
    }

    #[test]
    fn set_and_get_level() {
        let config = load_config_from_str(VALID_JSON).unwrap();
        let mut guard = CONFIG.write().unwrap();
        *guard = config;
        drop(guard);

        let triggers = set_level_for_provider("openai", "chat_only").unwrap();
        assert_eq!(triggers, vec!["MESSAGE"]);
        assert_eq!(get_level_for_provider("openai").unwrap(), "chat_only");
    }

    #[test]
    fn load_config_falls_back_to_bundled_default_when_file_is_missing() {
        // A dev binary run outside `tauri dev`/`tauri build` resolves
        // resource_dir() to a path with no resources copied into it; this
        // must not leave CONFIG in a broken or stale state.
        let missing = std::env::temp_dir().join("atelier_test_no_such_resource_dir_xyz");
        let result = load_config(&missing);
        assert!(result.is_ok());

        let config = get_config().unwrap();
        assert_eq!(config.version, 1);
        assert!(
            config.providers.contains_key("openai"),
            "should have loaded the real bundled providers, not the minimal hardcoded fallback"
        );
    }
}
