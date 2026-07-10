//! Google Drive OAuth "installed app" flow with PKCE and a local loopback
//! redirect — Google's own recommended pattern for desktop apps (see
//! developers.google.com/identity/protocols/oauth2/native-app). PKCE means
//! the token exchange doesn't depend on keeping a client secret
//! confidential the way a traditional server-side OAuth client would: the
//! `code_verifier` generated fresh for each sign-in attempt is what proves
//! this specific app instance made the original request, not a shared
//! secret baked into the binary.
//!
//! Unlike the API-key-only path in google_drive.rs (which can only read
//! files shared as "Anyone with the link"), a completed OAuth flow gives a
//! real user-scoped access token that can read the signed-in user's own
//! private Drive files.

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

// Register a real OAuth client at https://console.cloud.google.com/apis/credentials
// with application type "Desktop app". Google's own installed-app guidance
// treats a Desktop app's client secret as non-confidential (it can't be
// kept secret in a distributed binary either way) — see the link above —
// but it's still a per-project value, not a real secret to protect.
//
// Read at compile time from ATELIER_GOOGLE_OAUTH_CLIENT_ID/_SECRET so CI can
// embed the real values (e.g. via repo/org-level build variables) without
// anyone needing to edit this file; falls back to the placeholder below
// when unset, which is rejected the same way a real but wrong ID would be.
const CLIENT_ID: &str = match option_env!("ATELIER_GOOGLE_OAUTH_CLIENT_ID") {
    Some(id) => id,
    None => "REPLACE_WITH_REAL_GOOGLE_OAUTH_CLIENT_ID.apps.googleusercontent.com",
};
const CLIENT_SECRET: &str = match option_env!("ATELIER_GOOGLE_OAUTH_CLIENT_SECRET") {
    Some(secret) => secret,
    None => "REPLACE_WITH_REAL_GOOGLE_OAUTH_CLIENT_SECRET",
};

const SCOPE: &str = "https://www.googleapis.com/auth/drive.readonly openid email";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

fn code_verifier() -> String {
    // 32 bytes of CSPRNG entropy (two v4 UUIDs' random bits) -> 43 base64url
    // chars once encoded, right at PKCE's minimum length.
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

fn code_challenge(verifier: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(verifier.as_bytes());
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, digest)
}

/// Waits for exactly one HTTP request on the loopback listener (the OAuth
/// redirect), extracts `code`/`state`/`error` from its query string, and
/// responds with a minimal human-readable page so the browser tab doesn't
/// look broken. Only ever handles a single request — this listener exists
/// for one sign-in attempt, not as a standing local server.
async fn capture_redirect(listener: TcpListener, expected_state: &str) -> Result<String, String> {
    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|e| format!("Redirect listener failed: {e}"))?;
    let mut buf = [0u8; 8192];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("Failed to read redirect: {e}"))?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let first_line = request.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("");
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");

    let params: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();

    let (body, result) = if let Some(err) = params.get("error") {
        (
            "Authorization failed — you can close this tab and return to Open Atelier.",
            Err(format!("Google returned an error: {err}")),
        )
    } else if let Some(code) = params.get("code") {
        if params.get("state").map(String::as_str) != Some(expected_state) {
            (
                "Something went wrong — you can close this tab.",
                Err("State mismatch — possible CSRF, aborting".to_string()),
            )
        } else {
            (
                "Connected — you can close this tab and return to Open Atelier.",
                Ok(code.clone()),
            )
        }
    } else {
        (
            "Something went wrong — you can close this tab.",
            Err("No authorization code in redirect".to_string()),
        )
    };

    let html = format!(
        "<html><body style=\"font-family: sans-serif; padding: 2rem;\">{body}</body></html>"
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len(),
    );
    let _ = stream.write_all(response.as_bytes()).await;

    result
}

struct Tokens {
    access_token: String,
    refresh_token: Option<String>,
}

async fn exchange_code(code: &str, verifier: &str, redirect_uri: &str) -> Result<Tokens, String> {
    let resp = client()
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| format!("Could not reach Google: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Google rejected the code exchange (status {})",
            resp.status()
        ));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Unexpected response from Google: {e}"))?;
    let access_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Google response had no access_token".to_string())?
        .to_string();
    let refresh_token = body
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok(Tokens {
        access_token,
        refresh_token,
    })
}

/// Exchanges a refresh token for a new access token — called on-demand
/// when a Drive API call comes back 401, not on a background timer.
pub async fn refresh_access_token(refresh_token: &str) -> Result<String, String> {
    let resp = client()
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| format!("Could not reach Google: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Google rejected the refresh token (status {}) — reconnect Google Drive in Settings",
            resp.status()
        ));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Unexpected response from Google: {e}"))?;
    body.get("access_token")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| "Google refresh response had no access_token".to_string())
}

async fn fetch_email(access_token: &str) -> Result<String, String> {
    let resp = client()
        .get("https://openidconnect.googleapis.com/v1/userinfo")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("Could not reach Google: {e}"))?;
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Unexpected response from Google: {e}"))?;
    Ok(body
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("your Google account")
        .to_string())
}

pub struct ConnectResult {
    pub email: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
}

/// Runs the full PKCE + loopback-redirect flow end to end: binds an
/// ephemeral local port, opens the browser to Google's consent screen,
/// waits (up to 5 minutes) for the redirect, and exchanges the resulting
/// code for tokens. Storing the tokens is the caller's job (see
/// commands::connectors::connector_google_drive_oauth_connect) — this
/// only runs the OAuth dance itself.
pub async fn connect() -> Result<ConnectResult, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Could not start local redirect listener: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let verifier = code_verifier();
    let challenge = code_challenge(&verifier);
    let state = uuid::Uuid::new_v4().to_string();

    let auth_url = url::Url::parse_with_params(
        "https://accounts.google.com/o/oauth2/v2/auth",
        &[
            ("client_id", CLIENT_ID),
            ("redirect_uri", &redirect_uri),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("state", &state),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ],
    )
    .map_err(|e| format!("Failed to build authorize URL: {e}"))?;

    let _ = open::that(auth_url.as_str());

    let code = tokio::time::timeout(Duration::from_secs(300), capture_redirect(listener, &state))
        .await
        .map_err(|_| "Timed out waiting for Google sign-in".to_string())??;

    let tokens = exchange_code(&code, &verifier, &redirect_uri).await?;
    let email = fetch_email(&tokens.access_token)
        .await
        .unwrap_or_else(|_| "your Google account".to_string());

    Ok(ConnectResult {
        email,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_verifier_and_challenge_are_pkce_shaped() {
        let verifier = code_verifier();
        assert!(verifier.len() >= 43 && verifier.len() <= 128);
        assert!(verifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));

        let challenge = code_challenge(&verifier);
        assert!(!challenge.is_empty());
        assert_ne!(challenge, verifier);

        // Deterministic for the same input, different for different input.
        assert_eq!(code_challenge(&verifier), challenge);
        assert_ne!(code_challenge(&code_verifier()), challenge);
    }
}
