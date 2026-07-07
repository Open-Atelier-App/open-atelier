//! Minimal, read-only Notion API client for the NOTION_READ trigger and
//! the "Test connection" button in Settings > Connectors. The user pastes
//! an internal integration token from notion.so/my-integrations — Notion
//! has no "browse everything" scope, so the integration must also be
//! explicitly shared onto each page it should read (via that page's
//! "Connections" menu in Notion).

const NOTION_VERSION: &str = "2022-06-28";
const MAX_CONTENT_CHARS: usize = 200_000;

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

/// Validates a token by fetching the integration's own bot user, returning
/// its display name on success.
pub async fn test_connection(token: &str) -> Result<String, String> {
    let resp = client()
        .get("https://api.notion.com/v1/users/me")
        .header("Authorization", format!("Bearer {token}"))
        .header("Notion-Version", NOTION_VERSION)
        .send()
        .await
        .map_err(|e| format!("Could not reach Notion: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Notion rejected this token (status {})", resp.status()));
    }

    let body: serde_json::Value = resp.json().await
        .map_err(|e| format!("Unexpected response from Notion: {e}"))?;
    body.get("name").and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Notion response had no integration name".to_string())
}

/// Fetches a page's readable text by concatenating the plain_text of each
/// top-level block's rich_text. Doesn't recurse into nested block children
/// (toggles, nested lists) — good enough for a first read of typical pages
/// (paragraphs, headings, bullets), not a full page renderer.
pub async fn fetch_page(page_id: &str, token: &str) -> Result<String, String> {
    let url = format!("https://api.notion.com/v1/blocks/{page_id}/children?page_size=100");
    let resp = client()
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Notion-Version", NOTION_VERSION)
        .send()
        .await
        .map_err(|e| format!("Could not reach Notion: {e}"))?;

    let status = resp.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(format!("Page not found, or the integration hasn't been shared onto it: {page_id}"));
    }
    if !status.is_success() {
        return Err(format!("Notion returned {status} for page {page_id}"));
    }

    let body: serde_json::Value = resp.json().await
        .map_err(|e| format!("Unexpected response from Notion: {e}"))?;
    let results = body.get("results").and_then(|v| v.as_array())
        .ok_or_else(|| "Notion response had no results".to_string())?;

    let mut lines = Vec::new();
    for block in results {
        let Some(block_type) = block.get("type").and_then(|v| v.as_str()) else { continue };
        let Some(rich_text) = block.get(block_type).and_then(|b| b.get("rich_text")).and_then(|v| v.as_array()) else { continue };
        let text: String = rich_text.iter()
            .filter_map(|rt| rt.get("plain_text").and_then(|v| v.as_str()))
            .collect();
        if !text.is_empty() {
            lines.push(text);
        }
    }

    if lines.is_empty() {
        return Err(format!("No readable text blocks found on page {page_id} (nested blocks aren't supported yet)"));
    }
    let joined = lines.join("\n\n");
    if joined.len() > MAX_CONTENT_CHARS {
        return Err(format!("Page {page_id} is too large to read here ({} chars)", joined.len()));
    }
    Ok(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_rejects_a_bogus_token() {
        match test_connection("not-a-real-token").await {
            Ok(_) => panic!("expected a bogus token to be rejected"),
            Err(e) => assert!(
                e.contains("Notion rejected this token") || e.contains("Could not reach Notion"),
                "unexpected error shape: {e}",
            ),
        }
    }

    #[tokio::test]
    async fn fetch_page_rejects_a_bogus_token() {
        match fetch_page("00000000-0000-0000-0000-000000000000", "not-a-real-token").await {
            Ok(_) => panic!("expected a bogus token to be rejected"),
            Err(e) => assert!(
                e.contains("Notion returned") || e.contains("Could not reach Notion"),
                "unexpected error shape: {e}",
            ),
        }
    }
}
