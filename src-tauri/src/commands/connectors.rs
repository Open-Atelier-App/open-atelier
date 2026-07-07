use crate::error::{AtelierError, Result};
use crate::connectors::{github, github_oauth, notion, slack, google_drive, google_oauth};
use crate::commands::settings::{cred_keyring_name, SERVICE_NAME};
use crate::commands::cred_store::profile_store_set;

/// Validates a GitHub personal access token by fetching the authenticated
/// user, so enabling the connector in Settings gives immediate, real
/// feedback rather than silently accepting whatever was pasted. The token
/// itself is stored via the existing per-profile credential commands
/// (provider "github", cred_type "api_key") — this only tests it.
#[tauri::command]
pub async fn connector_github_test(token: String) -> Result<String> {
    github::test_connection(&token).await.map_err(AtelierError::internal)
}

/// Validates a Notion integration token by fetching the integration's own
/// bot user. Same "test only, storage is elsewhere" split as GitHub above
/// (provider "notion", cred_type "api_key").
#[tauri::command]
pub async fn connector_notion_test(token: String) -> Result<String> {
    notion::test_connection(&token).await.map_err(AtelierError::internal)
}

/// Validates a Slack bot token via auth.test (provider "slack", cred_type
/// "api_key").
#[tauri::command]
pub async fn connector_slack_test(token: String) -> Result<String> {
    slack::test_connection(&token).await.map_err(AtelierError::internal)
}

/// Validates a Google Drive API key (provider "google_drive", cred_type
/// "api_key") — see connectors::google_drive for why this can only ever
/// test reachability/validity, not a real user identity.
#[tauri::command]
pub async fn connector_google_drive_test(api_key: String) -> Result<String> {
    google_drive::test_connection(&api_key).await.map_err(AtelierError::internal)
}

// ── Native OAuth ("Connect" button) flows ────────────────────────────────
//
// GitHub (Device Flow) and Google Drive (installed-app PKCE) are the two
// connectors with a real "Connect" button instead of paste-a-token: both
// can be done without a client secret that would need a backend server to
// keep confidential, unlike Notion/Slack's OAuth (which do need one) — see
// the connectors module docs for the full reasoning.

/// Starts a GitHub Device Flow sign-in: requests a user code and opens the
/// browser to GitHub's verification page. The frontend shows the code (in
/// case the browser didn't open automatically) and then calls
/// `connector_github_oauth_finish` to wait for the user to approve it.
#[tauri::command]
pub async fn connector_github_oauth_start() -> Result<github_oauth::DeviceFlowStart> {
    github_oauth::start("repo").await.map_err(AtelierError::internal)
}

/// Polls until the device code is approved (or expires/is denied), then
/// verifies and stores the resulting token exactly like a pasted personal
/// access token (provider "github", cred_type "api_key") — a Device Flow
/// token works identically to a PAT for the GitHub REST API, so GITHUB_READ
/// and the "Test connection" button need no changes to use it.
#[tauri::command]
pub async fn connector_github_oauth_finish(device_code: String, interval: u64, expires_in: u64, profile_id: i64) -> Result<String> {
    let token = github_oauth::poll(&device_code, interval, expires_in).await.map_err(AtelierError::internal)?;
    let username = github::test_connection(&token).await.map_err(AtelierError::internal)?;
    let name = cred_keyring_name("github", "api_key");
    profile_store_set(profile_id, SERVICE_NAME, &name, &token)?;
    Ok(username)
}

/// Runs the full Google Drive OAuth flow (opens the browser, waits for the
/// redirect, exchanges the code) and stores the resulting access token
/// (cred_type "bearer_token") and refresh token (cred_type
/// "oauth_refresh_token", if Google returned one) under provider
/// "google_drive" — separate from the plain-API-key path's "api_key" slot,
/// so both can coexist and GDRIVE_READ can prefer OAuth when present (see
/// run_turn's GDRIVE_READ handling in commands::chat).
#[tauri::command]
pub async fn connector_google_drive_oauth_connect(profile_id: i64) -> Result<String> {
    let result = google_oauth::connect().await.map_err(AtelierError::internal)?;
    let token_name = cred_keyring_name("google_drive", "bearer_token");
    profile_store_set(profile_id, SERVICE_NAME, &token_name, &result.access_token)?;
    if let Some(refresh_token) = &result.refresh_token {
        let refresh_name = cred_keyring_name("google_drive", "oauth_refresh_token");
        profile_store_set(profile_id, SERVICE_NAME, &refresh_name, refresh_token)?;
    }
    Ok(result.email)
}
