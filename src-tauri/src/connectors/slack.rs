//! Minimal, read-only Slack Web API client for the SLACK_READ trigger and
//! the "Test connection" button in Settings > Connectors. The user pastes
//! a bot token (xoxb-...) from a Slack app with the `channels:history` and
//! `channels:read` scopes; the bot must also be invited into each channel
//! it should read (`/invite @your-app` in that channel).

const MAX_MESSAGES: u32 = 50;
const MAX_CONTENT_CHARS: usize = 200_000;

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

/// Validates a token via auth.test, returning the workspace (team) name on
/// success. Slack's Web API always returns HTTP 200 with an `ok` field
/// rather than using status codes for auth failures, so that field (not
/// the status code) is what actually indicates success here.
pub async fn test_connection(token: &str) -> Result<String, String> {
    let resp = client()
        .get("https://slack.com/api/auth.test")
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach Slack: {e}"))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Unexpected response from Slack: {e}"))?;

    if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(format!("Slack rejected this token: {err}"));
    }

    body.get("team")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Slack response had no team name".to_string())
}

/// Fetches the most recent messages (plain text only) from a channel via
/// conversations.history, oldest-first. The bot must already be a member
/// of the channel, or Slack returns the `not_in_channel` error.
pub async fn fetch_channel_history(channel_id: &str, token: &str) -> Result<String, String> {
    let resp = client()
        .get("https://slack.com/api/conversations.history")
        .header("Authorization", format!("Bearer {token}"))
        .query(&[
            ("channel", channel_id),
            ("limit", &MAX_MESSAGES.to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("Could not reach Slack: {e}"))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Unexpected response from Slack: {e}"))?;

    if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(format!(
            "Slack returned an error for channel {channel_id}: {err}"
        ));
    }

    let messages = body
        .get("messages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Slack response had no messages".to_string())?;
    if messages.is_empty() {
        return Err(format!("No messages found in channel {channel_id}"));
    }

    let mut lines: Vec<String> = messages
        .iter()
        .filter_map(|m| {
            let user = m.get("user").and_then(|v| v.as_str()).unwrap_or("unknown");
            let text = m.get("text").and_then(|v| v.as_str())?;
            Some(format!("{user}: {text}"))
        })
        .collect();
    lines.reverse(); // Slack returns newest-first; show chronological order instead

    let joined = lines.join("\n");
    if joined.len() > MAX_CONTENT_CHARS {
        return Err(format!(
            "Channel {channel_id} history is too large to read here ({} chars)",
            joined.len()
        ));
    }
    Ok(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_rejects_a_bogus_token() {
        match test_connection("xoxb-not-a-real-token").await {
            Ok(_) => panic!("expected a bogus token to be rejected"),
            Err(e) => assert!(
                e.contains("Slack rejected this token") || e.contains("Could not reach Slack"),
                "unexpected error shape: {e}",
            ),
        }
    }

    #[tokio::test]
    async fn fetch_channel_history_rejects_a_bogus_token() {
        match fetch_channel_history("C0000000000", "xoxb-not-a-real-token").await {
            Ok(_) => panic!("expected a bogus token to be rejected"),
            Err(e) => assert!(
                e.contains("Slack returned an error") || e.contains("Could not reach Slack"),
                "unexpected error shape: {e}",
            ),
        }
    }
}
