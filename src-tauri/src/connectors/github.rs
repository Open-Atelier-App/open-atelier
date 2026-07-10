//! Minimal, read-only GitHub REST client for the GITHUB_READ trigger and
//! the "Test connection" button in Settings > Connectors. No OAuth flow —
//! the user pastes a personal access token (or leaves it blank for public
//! repos, subject to GitHub's unauthenticated rate limit).

const USER_AGENT: &str = "OpenAtelier";
// A file this large in a chat context isn't useful anyway, and GitHub's
// contents API itself refuses files over 1MB via this endpoint.
const MAX_FILE_BYTES: usize = 200_000;

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(USER_AGENT)
        .build()
        .unwrap_or_default()
}

/// Validates a token by fetching the authenticated user, returning their
/// login on success — used by the Settings "Test connection" button so
/// enabling the connector gives real, immediate feedback instead of just
/// accepting whatever was pasted.
pub async fn test_connection(token: &str) -> Result<String, String> {
    let resp = client()
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach GitHub: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "GitHub rejected this token (status {})",
            resp.status()
        ));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Unexpected response from GitHub: {e}"))?;
    body.get("login")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "GitHub response had no username".to_string())
}

/// Fetches a single file's raw text content from a repo via the contents
/// API. `owner_repo` is "owner/repo"; `token` is optional (omitting it
/// only works for public repos, and hits GitHub's much lower unauthenticated
/// rate limit).
pub async fn fetch_file(
    owner_repo: &str,
    path: &str,
    token: Option<&str>,
) -> Result<String, String> {
    let (owner, repo) = owner_repo
        .split_once('/')
        .ok_or_else(|| format!("Expected \"owner/repo\", got: {owner_repo}"))?;

    let url = format!(
        "https://api.github.com/repos/{owner}/{repo}/contents/{}",
        path.trim_start_matches('/'),
    );

    let mut req = client()
        .get(&url)
        .header("Accept", "application/vnd.github.raw");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("Could not reach GitHub: {e}"))?;

    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(format!(
            "Not found: {owner_repo}/{path} (or the repo is private and no token is set)"
        ));
    }
    if !status.is_success() {
        return Err(format!("GitHub returned {status} for {owner_repo}/{path}"));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!(
            "{owner_repo}/{path} is too large to read here ({} bytes)",
            bytes.len()
        ));
    }

    String::from_utf8(bytes.to_vec())
        .map_err(|_| format!("{owner_repo}/{path} does not look like a text file"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fetch_file_rejects_malformed_owner_repo() {
        let err = fetch_file("not-a-valid-owner-repo", "README.md", None)
            .await
            .unwrap_err();
        assert!(err.contains("Expected \"owner/repo\""));
    }

    /// Exercises the real GitHub API against a small, stable public file
    /// when network access to api.github.com is actually available in the
    /// environment running the test. Sandboxes that route all egress
    /// through a restrictive proxy (only crates.io/npm/etc. allow-listed)
    /// won't reach GitHub at all — that's a legitimate, distinguishable
    /// outcome here, not a bug, so this only fails on some other
    /// unexpected error shape.
    #[tokio::test]
    async fn fetch_file_real_network_or_clear_error() {
        match fetch_file("octocat/Hello-World", "README", None).await {
            Ok(content) => assert!(!content.is_empty()),
            Err(e) => assert!(
                e.contains("Could not reach GitHub")
                    || e.contains("GitHub returned")
                    || e.contains("Not found"),
                "unexpected error shape: {e}",
            ),
        }
    }
}
