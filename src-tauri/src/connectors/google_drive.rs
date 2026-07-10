//! Read-only Google Drive client for the GDRIVE_READ trigger, with two
//! auth paths:
//! - A plain Google Cloud API key (`fetch_file`) — no user identity behind
//!   it, so it can only read files shared as "Anyone with the link".
//! - An OAuth access token from the native "Connect with Google" flow (see
//!   google_oauth.rs; `fetch_file_oauth`) — a real user-scoped token that
//!   can read the signed-in user's own private files too.
//!
//! `run_turn` (see `commands::chat`) prefers OAuth when connected, falling
//! back to the API-key path otherwise.

const MAX_FILE_BYTES: usize = 200_000;

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

/// There's no "whoami" endpoint for a bare API key (it carries no user
/// identity), so this exercises the same auth path as a real read by
/// requesting a deliberately-nonexistent file id, then distinguishes "bad
/// key" (400) from "key is fine, this file just doesn't exist" (404).
pub async fn test_connection(api_key: &str) -> Result<String, String> {
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/__atelier-connection-test__?fields=id&key={api_key}"
    );
    let resp = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Could not reach Google Drive: {e}"))?;

    match resp.status() {
        reqwest::StatusCode::NOT_FOUND => Ok("Key looks valid — Drive API is reachable".to_string()),
        reqwest::StatusCode::BAD_REQUEST => Err("Google rejected this API key".to_string()),
        reqwest::StatusCode::FORBIDDEN => Err("Key is valid but the Drive API isn't enabled for it (or it's restricted) — check the Google Cloud Console".to_string()),
        other => Err(format!("Unexpected response from Google Drive (status {other})")),
    }
}

/// Fetches a file's raw content by ID via `alt=media`. Only works for
/// regular (non-Google-native) files — Docs/Sheets/Slides require the
/// separate export endpoint with a target MIME type, which isn't
/// supported here; such files come back as a clear, distinct error.
pub async fn fetch_file(file_id: &str, api_key: &str) -> Result<String, String> {
    let url =
        format!("https://www.googleapis.com/drive/v3/files/{file_id}?alt=media&key={api_key}");
    let resp = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Could not reach Google Drive: {e}"))?;

    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(format!(
            "File not found, or not shared as \"Anyone with the link\": {file_id}"
        ));
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        let body = resp.text().await.unwrap_or_default();
        if body.contains("fileNotDownloadable") || body.contains("Export") {
            return Err(format!(
                "{file_id} looks like a native Google Doc/Sheet/Slide — GDRIVE_READ only supports \
                 regular files (txt, csv, md, etc.), not native Docs formats. Download or export it \
                 to a plain file first, then share that."
            ));
        }
        return Err(format!("Access denied for {file_id} — check the API key and that the file is shared as \"Anyone with the link\""));
    }
    if !status.is_success() {
        return Err(format!("Google Drive returned {status} for {file_id}"));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!(
            "{file_id} is too large to read here ({} bytes)",
            bytes.len()
        ));
    }
    String::from_utf8(bytes.to_vec())
        .map_err(|_| format!("{file_id} does not look like a text file"))
}

/// Distinguishes "the access token itself is stale" (worth refreshing and
/// retrying once) from every other failure (not worth retrying).
pub enum OAuthFetchError {
    AuthExpired,
    Other(String),
}

/// Same as `fetch_file`, but authenticated as the signed-in user via a
/// Bearer access token from the native OAuth flow instead of a bare API
/// key — this is what actually lets GDRIVE_READ reach the user's own
/// private files, not just link-shared ones.
pub async fn fetch_file_oauth(
    file_id: &str,
    access_token: &str,
) -> Result<String, OAuthFetchError> {
    let url = format!("https://www.googleapis.com/drive/v3/files/{file_id}?alt=media");
    let resp = client()
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| OAuthFetchError::Other(format!("Could not reach Google Drive: {e}")))?;

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(OAuthFetchError::AuthExpired);
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(OAuthFetchError::Other(format!(
            "File not found, or not accessible to this account: {file_id}"
        )));
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        let body = resp.text().await.unwrap_or_default();
        if body.contains("fileNotDownloadable") || body.contains("Export") {
            return Err(OAuthFetchError::Other(format!(
                "{file_id} looks like a native Google Doc/Sheet/Slide — GDRIVE_READ only supports \
                 regular files (txt, csv, md, etc.), not native Docs formats. Download or export it \
                 to a plain file first, then share that."
            )));
        }
        return Err(OAuthFetchError::Other(format!(
            "Access denied for {file_id}"
        )));
    }
    if !status.is_success() {
        return Err(OAuthFetchError::Other(format!(
            "Google Drive returned {status} for {file_id}"
        )));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| OAuthFetchError::Other(format!("Failed to read response: {e}")))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(OAuthFetchError::Other(format!(
            "{file_id} is too large to read here ({} bytes)",
            bytes.len()
        )));
    }
    String::from_utf8(bytes.to_vec())
        .map_err(|_| OAuthFetchError::Other(format!("{file_id} does not look like a text file")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_rejects_a_malformed_key() {
        // A key this short/garbled is always rejected by Google's API key
        // validation itself (status 400), regardless of network reachability.
        match test_connection("not-a-real-key").await {
            Ok(_) => panic!("expected a malformed key to be rejected"),
            Err(e) => assert!(
                e.contains("Google rejected this API key")
                    || e.contains("Could not reach Google Drive"),
                "unexpected error shape: {e}",
            ),
        }
    }

    #[tokio::test]
    async fn fetch_file_rejects_a_malformed_key() {
        match fetch_file("some-file-id", "not-a-real-key").await {
            Ok(_) => panic!("expected a malformed key to be rejected"),
            Err(e) => assert!(
                e.contains("Access denied")
                    || e.contains("not found")
                    || e.contains("Could not reach Google Drive")
                    || e.contains("Google Drive returned"),
                "unexpected error shape: {e}",
            ),
        }
    }
}
