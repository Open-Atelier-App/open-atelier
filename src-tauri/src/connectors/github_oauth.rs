//! GitHub OAuth Device Flow — the "Connect with GitHub" button in Settings
//! > Connectors. Device Flow needs no client secret for the token exchange
//! (unlike the redirect-based "Authorization Code" flow), which is what
//! makes it safe to ship a bare client ID inside a distributed desktop app:
//! there's no confidential value here to protect.
//!
//! Flow: `start` requests a device+user code pair and opens the browser to
//! GitHub's verification page; the user types in the short code shown in
//! the UI. Meanwhile `poll` repeatedly asks GitHub whether that code has
//! been approved yet, until it succeeds, is denied, or expires.

use serde::{Deserialize, Serialize};
use std::time::Duration;

// Register a real GitHub OAuth App at https://github.com/settings/developers
// with Device Flow enabled, then replace this with its client ID (a public
// identifier, not a secret — safe to commit). Left as a placeholder here
// since this repo doesn't have one of its own; GitHub will reject requests
// with a clear "incorrect_client_credentials"-style error until it's set.
const CLIENT_ID: &str = "REPLACE_WITH_REAL_GITHUB_OAUTH_CLIENT_ID";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct DeviceFlowStart {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Requests a device/user code pair and best-effort opens the browser to
/// the verification page — the frontend also shows the URL directly in
/// case the auto-open didn't work (e.g. no default browser configured).
///
/// GitHub's device-code endpoint frequently answers a bad `client_id` with
/// HTTP 200 and an OAuth-style `{"error": "...", "error_description": "..."}`
/// body rather than a non-2xx status, so the HTTP status alone isn't a
/// reliable success signal here — the body has to be inspected either way.
pub async fn start(scope: &str) -> Result<DeviceFlowStart, String> {
    let resp = client()
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", CLIENT_ID), ("scope", scope)])
        .send()
        .await
        .map_err(|e| format!("Could not reach GitHub: {e}"))?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("Could not read GitHub's response: {e}"))?;

    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| {
        format!(
            "GitHub returned an unexpected response (status {status}) instead of a device code — \
             is CLIENT_ID in github_oauth.rs a real, registered GitHub OAuth App with Device Flow enabled?"
        )
    })?;

    if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
        let desc = value.get("error_description").and_then(|v| v.as_str()).unwrap_or(err);
        return Err(format!("GitHub rejected the device flow request: {desc}"));
    }
    if !status.is_success() {
        return Err(format!("GitHub rejected the device flow request (status {status})"));
    }

    let body: DeviceCodeResponse = serde_json::from_value(value)
        .map_err(|e| format!("Unexpected response shape from GitHub: {e}"))?;

    let _ = open::that(&body.verification_uri);

    Ok(DeviceFlowStart {
        user_code: body.user_code,
        verification_uri: body.verification_uri,
        device_code: body.device_code,
        expires_in: body.expires_in,
        interval: body.interval,
    })
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    interval: Option<u64>,
}

/// Polls GitHub at the server-specified interval until the user finishes
/// entering the code on github.com (or it expires / is denied).
pub async fn poll(device_code: &str, mut interval_secs: u64, expires_in: u64) -> Result<String, String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(expires_in);
    loop {
        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
        if tokio::time::Instant::now() >= deadline {
            return Err("Code expired before authorization completed — try connecting again".to_string());
        }

        let resp = client()
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", CLIENT_ID),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|e| format!("Could not reach GitHub: {e}"))?;

        let body: TokenResponse = resp.json().await
            .map_err(|e| format!("Unexpected response from GitHub: {e}"))?;

        if let Some(token) = body.access_token {
            return Ok(token);
        }
        match body.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => interval_secs = body.interval.unwrap_or(interval_secs + 5),
            Some("expired_token") => return Err("Code expired before authorization completed — try connecting again".to_string()),
            Some("access_denied") => return Err("Authorization was denied".to_string()),
            Some(other) => return Err(format!("GitHub returned an error: {other}")),
            None => return Err("Unexpected response from GitHub".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn start_rejects_a_placeholder_client_id() {
        // Documents the expected failure mode until CLIENT_ID is replaced
        // with a real registered OAuth App — GitHub answers a bad client_id
        // with either a non-2xx status, an error-shaped 200 body, or (for a
        // string this malformed) a non-JSON response entirely, rather than
        // hanging or silently succeeding either way.
        match start("repo").await {
            Ok(_) => panic!("expected the placeholder client id to be rejected"),
            Err(e) => assert!(
                e.contains("GitHub rejected") || e.contains("Could not reach GitHub")
                    || e.contains("unexpected response") || e.contains("Unexpected response"),
                "unexpected error shape: {e}",
            ),
        }
    }
}
