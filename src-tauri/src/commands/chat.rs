use std::path::Path;
use tauri::{State, AppHandle, Emitter, Manager};
use crate::db::{Db, now_ms};
use crate::error::{AtelierError, Result};
use crate::models::{Conversation, Message, Citation, ToolCall, ConversationWithMessages, ChatDone, ChatContinuation};
use crate::llm;
use crate::triggers::{parser, executor, formatter, snapshot::SnapshotStore};

/// Caps automatic READ/LIST -> continue turns per user message, so a model
/// stuck in a read-loop (or a bug that always emits a READ) can't run away.
const MAX_AUTO_CONTINUATIONS: u32 = 4;

/// Builds a short "Created [x](atelier-file:x), edited [y](atelier-file:y)."
/// fallback confirmation for a turn that ended with only triggers and no
/// MESSAGE — see the `display_content` override in run_turn below for why
/// this exists. Paths use the same fake `atelier-file:` link scheme the
/// frontend's markdown renderer already special-cases to open the file
/// viewer instead of a real URL (see MarkdownContent in MessageBubble.tsx)
/// — this text is otherwise indistinguishable from prose, so a plain
/// filename mention wouldn't be clickable the way the action log's
/// "Created" rows already are.
fn synthesize_confirmation(actions: &[(String, Option<String>)]) -> String {
    fn verb(action: &str) -> &'static str {
        match action {
            "CREATE" | "CREATE_DOCX" | "CREATE_XLSX" | "CREATE_PPTX" | "CREATE_MP3" => "created",
            "WRITE" | "INSERT" | "APPEND" => "edited",
            "DELETE" => "deleted",
            "RENAME" => "renamed to",
            "EXPORT_PDF" => "exported",
            "PLAN" => "planned",
            _ => "updated",
        }
    }

    let parts: Vec<String> = actions.iter()
        .map(|(action, path)| match path {
            // PLAN's "path" slot actually carries the plan's title, not a
            // real file — linking it would send the user to open a
            // nonexistent file named after the title.
            Some(p) if !p.is_empty() && action == "PLAN" => format!("{} {p}", verb(action)),
            // The destination is wrapped in angle brackets: a bare markdown
            // link destination can't contain spaces (per CommonMark), and
            // real file names very often do.
            Some(p) if !p.is_empty() => format!("{} [{p}](<atelier-file:{p}>)", verb(action)),
            _ => verb(action).to_string(),
        })
        .collect();

    if parts.is_empty() {
        return String::new();
    }

    let mut sentence = parts.join(", ");
    if let Some(first) = sentence.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    sentence.push('.');
    sentence
}

/// Derives a title locally (no LLM) from the user's first message, used
/// when auto-title's LLM call fails outright after all retries — a
/// conversation should never be permanently stuck showing "New
/// conversation" just because a provider had a bad moment. Takes the
/// first handful of words, capped to a UI-label-sized length.
fn fallback_title(content: &str) -> String {
    const MAX_WORDS: usize = 8;
    const MAX_CHARS: usize = 60;

    let words: Vec<&str> = content.split_whitespace().take(MAX_WORDS).collect();
    if words.is_empty() {
        return "New conversation".to_string();
    }

    let mut title: String = words.join(" ");
    if let Some(first) = title.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    if title.chars().count() > MAX_CHARS {
        title = title.chars().take(MAX_CHARS).collect::<String>().trim_end().to_string();
        title.push('…');
    }
    title
}

/// Shared plumbing for the four connector-backed READ triggers
/// (GITHUB_READ, NOTION_READ, SLACK_READ, GDRIVE_READ). Each of these hits
/// an external service, not the local workspace filesystem, so — like
/// PLAN — they're handled directly in run_turn rather than routed through
/// triggers::executor, which is synchronous and workspace-filesystem-only.
/// This covers the permission check, per-profile credential lookup, and
/// turning the connector's `Result<String, String>` into the same
/// trigger_feedback/executed-event shape triggers::executor produces for
/// file triggers, so each connector only needs to supply its own fetch
/// call and a human-readable "target" string for logging/feedback.
#[allow(clippy::too_many_arguments)]
async fn handle_connector_read_trigger<F, Fut>(
    action: &str,
    target: String,
    cred_provider: &str,
    connector_label: &str,
    perm_level: &str,
    conv_id: i64,
    app: &AppHandle,
    profile_id: Option<i64>,
    trigger_feedback: &mut Vec<String>,
    any_trigger_failed: &mut bool,
    has_content_feedback: &mut bool,
    fetch: F,
) where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = std::result::Result<String, String>>,
{
    let allowed = llm::permissions::allowed_triggers_for_level(perm_level)
        .map(|a| a.iter().any(|x| x == action))
        .unwrap_or(false);
    if !allowed {
        let level_label = llm::permissions::level_label(perm_level).unwrap_or_default();
        let detail = format!("Permission denied: {level_label} cannot {action}");
        trigger_feedback.push(formatter::format_result(action, "FAIL", &detail));
        *any_trigger_failed = true;
        return;
    }

    let token = profile_id.and_then(|pid| {
        let name = crate::commands::settings::cred_keyring_name(cred_provider, "api_key");
        crate::commands::cred_store::profile_store_get(pid, crate::commands::settings::SERVICE_NAME, &name)
            .ok().flatten().map(|(v, _)| v)
    });
    let Some(token) = token else {
        let detail = format!("{connector_label} connector is not enabled — add a token in Settings > Connectors first");
        log::warn!("Trigger executed (conversation {conv_id}): {action} FAIL {target} - {detail}");
        trigger_feedback.push(formatter::format_result(action, "FAIL", &detail));
        *any_trigger_failed = true;
        let result = executor::TriggerResult { action: action.into(), status: "FAIL".into(), detail, file_path: Some(target) };
        let _ = app.emit("trigger://executed", &result);
        return;
    };

    match fetch(token).await {
        Ok(content) => {
            log::info!("Trigger executed (conversation {conv_id}): {action} OK {target} ({} chars)", content.len());
            trigger_feedback.push(formatter::format_result(action, "OK", &format!("{} characters read", content.len())));
            trigger_feedback.push(formatter::format_content(&target, &content));
            *has_content_feedback = true;
            let result = executor::TriggerResult { action: action.into(), status: "OK".into(), detail: format!("{} characters read", content.len()), file_path: Some(target) };
            let _ = app.emit("trigger://executed", &result);
        }
        Err(e) => {
            log::warn!("Trigger executed (conversation {conv_id}): {action} FAIL {target} - {e}");
            trigger_feedback.push(formatter::format_result(action, "FAIL", &e));
            *any_trigger_failed = true;
            let result = executor::TriggerResult { action: action.into(), status: "FAIL".into(), detail: e, file_path: Some(target) };
            let _ = app.emit("trigger://executed", &result);
        }
    }
}

/// GDRIVE_READ's own handler rather than a call into
/// handle_connector_read_trigger above: it needs to try two different
/// credentials in order — a native-OAuth access token first (refreshing it
/// once via the stored refresh token if it's gone stale, then retrying),
/// falling back to a plain API key — which doesn't fit that helper's
/// single-token-lookup shape.
#[allow(clippy::too_many_arguments)]
async fn handle_gdrive_read(
    file_id: &str,
    perm_level: &str,
    conv_id: i64,
    app: &AppHandle,
    profile_id: Option<i64>,
    trigger_feedback: &mut Vec<String>,
    any_trigger_failed: &mut bool,
    has_content_feedback: &mut bool,
) {
    const ACTION: &str = "GDRIVE_READ";
    let allowed = llm::permissions::allowed_triggers_for_level(perm_level)
        .map(|a| a.iter().any(|x| x == ACTION))
        .unwrap_or(false);
    if !allowed {
        let level_label = llm::permissions::level_label(perm_level).unwrap_or_default();
        let detail = format!("Permission denied: {level_label} cannot {ACTION}");
        trigger_feedback.push(formatter::format_result(ACTION, "FAIL", &detail));
        *any_trigger_failed = true;
        return;
    }

    let cred = |cred_type: &str| -> Option<String> {
        profile_id.and_then(|pid| {
            let name = crate::commands::settings::cred_keyring_name("google_drive", cred_type);
            crate::commands::cred_store::profile_store_get(pid, crate::commands::settings::SERVICE_NAME, &name)
                .ok().flatten().map(|(v, _)| v)
        })
    };

    use crate::connectors::google_drive::OAuthFetchError;
    let outcome: std::result::Result<String, String> = if let Some(access_token) = cred("bearer_token") {
        match crate::connectors::google_drive::fetch_file_oauth(file_id, &access_token).await {
            Ok(content) => Ok(content),
            Err(OAuthFetchError::Other(e)) => Err(e),
            Err(OAuthFetchError::AuthExpired) => match cred("oauth_refresh_token") {
                Some(refresh_token) => match crate::connectors::google_oauth::refresh_access_token(&refresh_token).await {
                    Ok(new_access_token) => {
                        if let Some(pid) = profile_id {
                            let name = crate::commands::settings::cred_keyring_name("google_drive", "bearer_token");
                            let _ = crate::commands::cred_store::profile_store_set(pid, crate::commands::settings::SERVICE_NAME, &name, &new_access_token);
                        }
                        match crate::connectors::google_drive::fetch_file_oauth(file_id, &new_access_token).await {
                            Ok(content) => Ok(content),
                            Err(OAuthFetchError::Other(e)) => Err(e),
                            Err(OAuthFetchError::AuthExpired) =>
                                Err("Google Drive sign-in expired — reconnect it in Settings > Connectors".to_string()),
                        }
                    }
                    Err(e) => Err(e),
                },
                None => Err("Google Drive sign-in expired — reconnect it in Settings > Connectors".to_string()),
            },
        }
    } else if let Some(api_key) = cred("api_key") {
        crate::connectors::google_drive::fetch_file(file_id, &api_key).await
    } else {
        Err("Google Drive connector is not enabled — connect it in Settings > Connectors first".to_string())
    };

    match outcome {
        Ok(content) => {
            log::info!("Trigger executed (conversation {conv_id}): {ACTION} OK {file_id} ({} chars)", content.len());
            trigger_feedback.push(formatter::format_result(ACTION, "OK", &format!("{} characters read", content.len())));
            trigger_feedback.push(formatter::format_content(file_id, &content));
            *has_content_feedback = true;
            let result = executor::TriggerResult { action: ACTION.into(), status: "OK".into(), detail: format!("{} characters read", content.len()), file_path: Some(file_id.to_string()) };
            let _ = app.emit("trigger://executed", &result);
        }
        Err(e) => {
            log::warn!("Trigger executed (conversation {conv_id}): {ACTION} FAIL {file_id} - {e}");
            trigger_feedback.push(formatter::format_result(ACTION, "FAIL", &e));
            *any_trigger_failed = true;
            let result = executor::TriggerResult { action: ACTION.into(), status: "FAIL".into(), detail: e, file_path: Some(file_id.to_string()) };
            let _ = app.emit("trigger://executed", &result);
        }
    }
}

/// Builds the "## Related project context" block shared between a
/// sub-project and its parent — one level of the hierarchy only. Reads
/// each related workspace's own `context.md` (see the LLM Functions
/// protocol's project-context-file section) if it has one; workspaces
/// without one contribute nothing rather than an empty heading.
fn build_related_context(conn: &rusqlite::Connection, workspace_id: i64) -> String {
    let mut blocks: Vec<String> = Vec::new();

    let parent: Option<(String, String)> = conn.query_row(
        "SELECT p.name, p.path FROM workspaces w JOIN workspaces p ON p.id = w.parent_workspace_id WHERE w.id = ?1",
        [workspace_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).ok();
    if let Some((parent_name, parent_path)) = parent {
        if let Some(ctx) = llm::skills::read_context_md(&parent_path) {
            blocks.push(format!("### Parent project \"{parent_name}\"\n{ctx}"));
        }
    }

    let children: Vec<(String, String)> = conn.prepare(
        "SELECT name, path FROM workspaces WHERE parent_workspace_id = ?1"
    ).and_then(|mut stmt| {
        stmt.query_map([workspace_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()
    }).unwrap_or_default();
    for (child_name, child_path) in &children {
        if let Some(ctx) = llm::skills::read_context_md(child_path) {
            blocks.push(format!("### Sub-project \"{child_name}\"\n{ctx}"));
        }
    }

    if blocks.is_empty() { String::new() } else { format!("\n\n## Related project context\n{}", blocks.join("\n\n")) }
}

const CONVERSATION_COLUMNS: &str = "id, workspace_id, title, created_at, updated_at, provider, model, summary, compressed_memory, compressed_at, group_id";

fn row_to_conversation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        title: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        provider: row.get(5)?,
        model: row.get(6)?,
        summary: row.get(7)?,
        compressed_memory: row.get(8)?,
        compressed_at: row.get(9)?,
        group_id: row.get(10)?,
    })
}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        created_at: row.get(4)?,
        token_count: row.get(5)?,
        input_tokens: row.get(6)?,
        output_tokens: row.get(7)?,
        error: row.get(8)?,
        status: row.get(9)?,
        provider: row.get(10)?,
        model: row.get(11)?,
        display_override: row.get(12)?,
    })
}

#[tauri::command]
pub fn conversation_list(workspace_id: i64, db: State<Db>) -> Result<Vec<Conversation>> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let mut stmt = db.prepare(
        &format!("SELECT {CONVERSATION_COLUMNS} FROM conversations WHERE workspace_id = ?1 ORDER BY updated_at DESC")
    )?;
    let rows = stmt.query_map([workspace_id], row_to_conversation)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[tauri::command]
pub fn conversation_create(workspace_id: i64, title: Option<String>, db: State<Db>) -> Result<Conversation> {
    let now = now_ms();
    let title = title.unwrap_or_else(|| "New conversation".to_string());
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute(
        "INSERT INTO conversations (workspace_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![workspace_id, title, now, now],
    )?;
    let id = db.last_insert_rowid();
    let conv = db.query_row(
        &format!("SELECT {CONVERSATION_COLUMNS} FROM conversations WHERE id = ?1"),
        [id], row_to_conversation,
    )?;
    Ok(conv)
}

#[tauri::command]
pub fn conversation_rename(id: i64, title: String, db: State<Db>) -> Result<Conversation> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute("UPDATE conversations SET title = ?1 WHERE id = ?2", rusqlite::params![title, id])?;
    let conv = db.query_row(
        &format!("SELECT {CONVERSATION_COLUMNS} FROM conversations WHERE id = ?1"),
        [id], row_to_conversation,
    )?;
    Ok(conv)
}

#[tauri::command]
pub fn conversation_delete(id: i64, db: State<Db>) -> Result<()> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute("DELETE FROM conversations WHERE id = ?1", [id])?;
    Ok(())
}

/// Deletes a conversation like `conversation_delete`, but first writes its
/// full transcript to `.archives/chat-<timestamp>.md` inside the project —
/// archiving a chat means "done with this thread, but keep what it said"
/// rather than losing it outright.
#[tauri::command]
pub fn conversation_archive(id: i64, db: State<Db>) -> Result<()> {
    let (conversation, messages, ws_path): (Conversation, Vec<Message>, String) = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        let conversation = db.query_row(
            &format!("SELECT {CONVERSATION_COLUMNS} FROM conversations WHERE id = ?1"),
            [id], row_to_conversation,
        ).map_err(|_| AtelierError::not_found("Conversation not found"))?;

        let mut stmt = db.prepare(
            "SELECT id, conversation_id, role, content, created_at, token_count, input_tokens, output_tokens, error, status, provider, model, display_override
             FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC"
        )?;
        let messages: Vec<Message> = stmt.query_map([id], row_to_message)?
            .collect::<rusqlite::Result<_>>()?;

        let ws_path: String = db.query_row(
            "SELECT path FROM workspaces WHERE id = ?1",
            [conversation.workspace_id],
            |r| r.get(0),
        ).map_err(|_| AtelierError::not_found("Workspace not found"))?;

        (conversation, messages, ws_path)
    };

    let created = chrono::DateTime::from_timestamp_millis(conversation.created_at)
        .unwrap_or_else(chrono::Utc::now);

    let mut transcript = String::new();
    transcript.push_str(&format!("# {}\n\n", conversation.title));
    transcript.push_str(&format!("- **Archived:** {}\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
    transcript.push_str(&format!("- **Started:** {}\n", created.format("%Y-%m-%d %H:%M:%S UTC")));
    if let Some(provider) = &conversation.provider {
        transcript.push_str(&format!("- **Model:** {provider} / {}\n", conversation.model.as_deref().unwrap_or("?")));
    }
    transcript.push_str(&format!("- **Messages:** {}\n\n---\n\n", messages.len()));

    for m in &messages {
        if m.role == "system" || m.content.trim().is_empty() { continue; }
        let who = match m.role.as_str() { "user" => "User", "assistant" => "Assistant", other => other };
        transcript.push_str(&format!("## {who}\n\n{}\n\n", m.content));
    }

    let archives_dir = Path::new(&ws_path).join(".archives");
    std::fs::create_dir_all(&archives_dir)?;

    // Slugify the title for a readable filename, falling back to the
    // conversation id if it's empty/all-punctuation once stripped.
    let slug: String = conversation.title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() { format!("conversation-{id}") } else { slug };
    let filename = format!("chat-{}-{slug}.md", chrono::Utc::now().format("%Y-%m-%d-%H%M%S"));

    std::fs::write(archives_dir.join(filename), transcript)?;

    conversation_delete(id, db)
}

#[tauri::command]
pub fn conversation_get(id: i64, db: State<Db>) -> Result<ConversationWithMessages> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let conversation = db.query_row(
        &format!("SELECT {CONVERSATION_COLUMNS} FROM conversations WHERE id = ?1"),
        [id], row_to_conversation,
    ).map_err(|_| AtelierError::not_found("Conversation not found"))?;

    let mut stmt = db.prepare(
        "SELECT id, conversation_id, role, content, created_at, token_count, input_tokens, output_tokens, error, status, provider, model, display_override
         FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC"
    )?;
    let messages: Vec<Message> = stmt.query_map([id], row_to_message)?
        .collect::<rusqlite::Result<_>>()?;

    Ok(ConversationWithMessages { conversation, messages })
}

/// Creates a new conversation in the same workspace containing a copy of
/// every message up to and including `up_to_message_id`, so the user can
/// branch off from a specific point (typically an assistant reply) and
/// continue in a different direction without disturbing the original
/// thread. The fork is a one-time copy, not a live link back to the
/// source — editing one afterward never affects the other.
#[tauri::command]
pub fn conversation_fork(id: i64, up_to_message_id: i64, db: State<Db>) -> Result<Conversation> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    fork_impl(&db, id, up_to_message_id)
}

fn fork_impl(db: &rusqlite::Connection, id: i64, up_to_message_id: i64) -> Result<Conversation> {
    let (workspace_id, title, provider, model): (i64, String, Option<String>, Option<String>) = db.query_row(
        "SELECT workspace_id, title, provider, model FROM conversations WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).map_err(|_| AtelierError::not_found("Conversation not found"))?;

    let cutoff: i64 = db.query_row(
        "SELECT created_at FROM messages WHERE id = ?1 AND conversation_id = ?2",
        rusqlite::params![up_to_message_id, id],
        |r| r.get(0),
    ).map_err(|_| AtelierError::not_found("Message not found in this conversation"))?;

    let now = now_ms();
    let fork_title = format!("{title} (fork)");
    db.execute(
        "INSERT INTO conversations (workspace_id, title, created_at, updated_at, provider, model) VALUES (?1, ?2, ?3, ?3, ?4, ?5)",
        rusqlite::params![workspace_id, fork_title, now, provider, model],
    )?;
    let new_id = db.last_insert_rowid();

    db.execute(
        "INSERT INTO messages (conversation_id, role, content, created_at, token_count, input_tokens, output_tokens, error, status, provider, model, display_override)
         SELECT ?1, role, content, created_at, token_count, input_tokens, output_tokens, error, status, provider, model, display_override
         FROM messages WHERE conversation_id = ?2 AND created_at <= ?3 ORDER BY created_at ASC",
        rusqlite::params![new_id, id, cutoff],
    )?;

    let conv = db.query_row(
        &format!("SELECT {CONVERSATION_COLUMNS} FROM conversations WHERE id = ?1"),
        [new_id], row_to_conversation,
    )?;
    Ok(conv)
}

/// Compresses a conversation: summarizes everything so far into a memory
/// block, and marks the compression point (`compressed_at`) so future
/// turns (see run_turn) send only that memory plus messages created after
/// it — not the full transcript. Lets the user keep chatting in the same
/// thread, optionally with a different provider/model, without paying for
/// (or re-sending) the whole prior history on every turn.
#[tauri::command]
pub async fn conversation_compress(
    id: i64,
    provider: String,
    model: String,
    app: AppHandle,
    db: State<'_, Db>,
) -> Result<Conversation> {
    let (transcript, profile_id): (String, Option<i64>) = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        let mut stmt = db.prepare(
            "SELECT role, content FROM messages WHERE conversation_id = ?1 AND role IN ('user','assistant') AND content != '' ORDER BY created_at ASC"
        )?;
        let turns: Vec<(String, String)> = stmt.query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?.collect::<rusqlite::Result<_>>()?;
        if turns.is_empty() {
            return Err(AtelierError::internal("Nothing to compress yet"));
        }
        let profile_id: Option<i64> = db.query_row(
            "SELECT p.id FROM conversations c JOIN workspaces w ON w.id = c.workspace_id JOIN profiles p ON p.id = w.profile_id WHERE c.id = ?1",
            [id],
            |r| r.get(0),
        ).ok();
        (turns.iter().map(|(role, content)| format!("{role}: {content}")).collect::<Vec<_>>().join("\n\n"), profile_id)
    };

    let prompt = format!(
        "Write a thorough memory of this conversation so it can continue seamlessly \
         without the original messages — key facts, decisions made, files created or \
         discussed, and any open threads or next steps. Be specific and complete, but \
         don't pad it with filler. Reply with the memory only, no preamble.\n\n{transcript}"
    );

    const RETRY_DELAYS_MS: [u64; 2] = [1000, 3000];
    let mut result = None;
    let mut last_err = None;
    for delay_ms in RETRY_DELAYS_MS {
        match llm::stream_chat(&app, -1, &provider, &model, vec![("user".to_string(), prompt.clone())], profile_id).await {
            Ok(r) => { result = Some(r); break; }
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }
        }
    }
    let memory = match result {
        Some(r) => r.content.trim().to_string(),
        None => return Err(last_err.unwrap_or_else(|| AtelierError::internal("Compression failed"))),
    };
    if memory.is_empty() {
        return Err(AtelierError::internal("LLM returned an empty memory"));
    }

    let now = now_ms();
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    // Persist provider/model onto the conversation itself, not just use
    // them for this summarization call — this is what actually lets the
    // user switch models going forward: the composer's model lock (see
    // ChatInput's isLockedSession) reads the conversation's own
    // provider/model once it re-locks on the next message, so if this
    // isn't saved here, that next message silently reverts to whatever
    // model the conversation started with.
    db.execute(
        "UPDATE conversations SET compressed_memory = ?1, compressed_at = ?2, provider = ?3, model = ?4 WHERE id = ?5",
        rusqlite::params![memory, now, provider, model, id],
    )?;
    let conv = db.query_row(
        &format!("SELECT {CONVERSATION_COLUMNS} FROM conversations WHERE id = ?1"),
        [id], row_to_conversation,
    )?;
    Ok(conv)
}

/// Returns the assistant message id. Streaming happens via events; caller polls for completion.
#[tauri::command]
pub async fn ask(
    conversation_id: i64,
    content: String,
    provider: String,
    model: String,
    db: State<'_, Db>,
    app: AppHandle,
) -> Result<Message> {
    run_turn(conversation_id, content, provider, model, db.inner().clone(), app, None).await
}

/// Shared by `ask` (a normal user turn) and plan step execution (see
/// commands::plan::plan_execute_next) — a plan step is just a turn whose
/// "user message" is a synthesized step instruction instead of something
/// the user typed. `plan_task_id` is `Some` only for the latter, so this
/// function can mark that task done/failed once the turn (including any
/// auto-continuations) settles.
pub(crate) async fn run_turn(
    conversation_id: i64,
    content: String,
    provider: String,
    model: String,
    db: Db,
    app: AppHandle,
    plan_task_id: Option<i64>,
) -> Result<Message> {
    let now = now_ms();

    // Check if this is the first user message in this conversation (for auto-title)
    let is_first: bool = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            [conversation_id],
            |r| r.get(0),
        ).unwrap_or(1);
        count == 0
    };

    // Full user input, so a diagnostic report shows exactly what was asked —
    // not just the model's side of the exchange.
    log::info!("User input (conversation {conversation_id}): {content}");

    // Insert user message
    {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        db.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at, status) VALUES (?1, 'user', ?2, ?3, 'complete')",
            rusqlite::params![conversation_id, content, now],
        )?;
        db.execute(
            "UPDATE conversations SET updated_at = ?1, provider = ?2, model = ?3 WHERE id = ?4",
            rusqlite::params![now, provider, model, conversation_id],
        )?;
    }

    // Insert streaming placeholder
    let assistant_msg_id = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        db.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at, status, provider, model) VALUES (?1, 'assistant', '', ?2, 'streaming', ?3, ?4)",
            rusqlite::params![conversation_id, now + 1, provider, model],
        )?;
        db.last_insert_rowid()
    };

    // Fetch conversation history (all messages except the blank assistant
    // one). If the conversation was compressed (see conversation_compress),
    // only messages created after the compression point are real history —
    // everything before it is represented instead by a single memory
    // block prepended below, which is what lets the user keep chatting in
    // the same thread (optionally with a different provider/model)
    // without resending the full original transcript on every turn.
    let compressed: Option<(String, i64)> = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        db.query_row(
            "SELECT compressed_memory, compressed_at FROM conversations WHERE id = ?1 AND compressed_memory IS NOT NULL",
            [conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).ok()
    };

    let mut history: Vec<(String, String)> = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        match compressed {
            Some((_, compressed_at)) => {
                let mut stmt = db.prepare(
                    "SELECT role, content FROM messages WHERE conversation_id = ?1 AND id != ?2 AND status != 'streaming' AND content != '' AND created_at > ?3 ORDER BY created_at ASC"
                )?;
                let rows: Vec<(String, String)> = stmt.query_map(rusqlite::params![conversation_id, assistant_msg_id, compressed_at], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?.collect::<rusqlite::Result<_>>()?;
                rows
            }
            None => {
                let mut stmt = db.prepare(
                    "SELECT role, content FROM messages WHERE conversation_id = ?1 AND id != ?2 AND status != 'streaming' AND content != '' ORDER BY created_at ASC"
                )?;
                let rows: Vec<(String, String)> = stmt.query_map(rusqlite::params![conversation_id, assistant_msg_id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?.collect::<rusqlite::Result<_>>()?;
                rows
            }
        }
    };

    if let Some((memory, _)) = &compressed {
        history.insert(0, ("system".to_string(), format!(
            "## Prior session memory\nThis conversation was compressed to continue more efficiently (possibly with a different model). Here is a summary of everything before this point:\n\n{memory}"
        )));
    }

    // Resolve workspace context for system prompt and trigger execution
    let ws_context: Option<(String, String, String, i64, i64)> = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        db.query_row(
            "SELECT w.path, w.name, p.root_path, w.id, p.id FROM conversations c
             JOIN workspaces w ON w.id = c.workspace_id
             JOIN profiles p ON p.id = w.profile_id
             WHERE c.id = ?1",
            [conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        ).ok()
    };

    // Sub-projects share context in both directions: a child project sees
    // its parent's context.md, and a parent sees each child's — one level
    // only, not a general ancestor/descendant tree walk.
    let related_context: String = if let Some((_, _, _, ws_id, _)) = ws_context {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        build_related_context(&db, ws_id)
    } else {
        String::new()
    };

    // Prepend Atelier's workspace context + skills + LLM Functions protocol
    if let Some((ref workspace_path, ref workspace_name, ref profile_root, _, _)) = ws_context {
        let mut context = llm::skills::build_context(profile_root, workspace_name, workspace_path, &provider, is_first);
        context.push_str(&related_context);
        history.insert(0, ("system".to_string(), context));
    }

    let provider_clone = provider.clone();
    let model_clone = model.clone();
    let content_clone = content.clone();
    let workspace_path_clone = ws_context.as_ref().map(|(p, _, _, _, _)| p.clone());
    let profile_id_clone = ws_context.as_ref().map(|(_, _, _, _, pid)| *pid);
    let workspace_id_clone = ws_context.as_ref().map(|(_, _, _, wsid, _)| *wsid);

    // Stream in background task
    let app_clone = app.clone();
    let db_clone = db.clone();
    let conv_id = conversation_id;
    let snapshot_store = app.state::<std::sync::Arc<SnapshotStore>>();
    let snapshot_store = snapshot_store.inner().clone();

    tokio::spawn(async move {
        let mut current_msg_id = assistant_msg_id;
        let mut current_history = history;
        let mut continuations: u32 = 0;
        let mut any_trigger_failed = false;

        loop {
            let result = llm::stream_chat(
                &app_clone,
                current_msg_id,
                &provider_clone,
                &model_clone,
                current_history.clone(),
                profile_id_clone,
            ).await;

            let stream_result = match result {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Provider call failed for {provider_clone}/{model_clone} (message {current_msg_id}): {} (code: {:?})", e.message, e.code);
                    if let Ok(db) = db_clone.lock() {
                        let _ = db.execute(
                            "UPDATE messages SET status = 'error', error = ?1 WHERE id = ?2",
                            rusqlite::params![e.message, current_msg_id],
                        );
                    }
                    let _ = app_clone.emit("chat://error", serde_json::json!({
                        "message_id": current_msg_id,
                        "error": e
                    }));
                    // The auto-title block further down only runs once a
                    // response actually completes — a conversation whose
                    // very first message errors (bad key, provider outage,
                    // rate limit) would otherwise be stuck on "New
                    // conversation" forever, with no later message ever
                    // getting a chance to retry titling it.
                    if is_first {
                        let title = fallback_title(&content_clone);
                        if let Ok(db) = db_clone.lock() {
                            let _ = db.execute(
                                "UPDATE conversations SET title = ?1 WHERE id = ?2",
                                rusqlite::params![title, conv_id],
                            );
                        }
                        let _ = app_clone.emit("conversation://titled", serde_json::json!({
                            "id": conv_id,
                            "title": title
                        }));
                    }
                    break;
                }
            };

            // Full raw model output, before trigger-stripping — the chat
            // bubble only ever shows `display_content` (triggers stripped,
            // sometimes replaced by a synthesized confirmation), so this is
            // the only place the model's complete, unedited response is
            // captured anywhere.
            log::info!(
                "LLM response ({provider_clone}/{model_clone}, message {current_msg_id}, finish_reason={:?}):\n{}",
                stream_result.finish_reason, stream_result.content,
            );

            // Parse triggers from the complete response
            let parse_result = parser::parse(&stream_result.content);

            // Parse errors (malformed/unterminated triggers) are otherwise
            // only visible in the UI's action log, which doesn't end up in
            // "Copy Diagnostic Report". Log them, and flag the single most
            // useful piece of context for figuring out why: whether the
            // provider stopped because it hit its own output token limit
            // (a truncated CREATE_DOCX/PLAN/etc. mid-string looks exactly
            // like a malformed trigger, but the fix is completely different —
            // shorter output, not a prompt bug).
            if !parse_result.errors.is_empty() {
                let truncated = llm::router::is_truncated(&stream_result.finish_reason);
                for err in &parse_result.errors {
                    log::warn!(
                        "Trigger parse error ({}/{}, response {} chars, finish_reason={:?}{}): {} | near: {:?}",
                        provider_clone, model_clone, stream_result.content.len(), stream_result.finish_reason,
                        if truncated { ", LIKELY TRUNCATED" } else { "" },
                        err.message, err.raw,
                    );
                }
            }

            // Execute triggers if any were found
            let mut trigger_feedback = Vec::new();
            let mut has_content_feedback = false;
            // Non-failing (action, file_path) pairs, kept so a turn that ends
            // with zero chat text (all triggers, no MESSAGE — weaker models
            // frequently do this) can still show the user *something* rather
            // than a blank bubble. See synthesize_confirmation below.
            let mut executed_actions: Vec<(String, Option<String>)> = Vec::new();
            if !parse_result.triggers.is_empty() {
                let perm_level = llm::permissions::get_level_for_provider(&provider_clone)
                    .unwrap_or_else(|_| "chat_only".into());

                // PLAN creates rows in the conversation's own SQLite tables
                // (plans/plan_tasks), not a workspace file — handle it here
                // directly rather than routing it through triggers::executor,
                // which only knows about the workspace filesystem and has no
                // DB handle.
                let (plan_triggers, rest): (Vec<_>, Vec<_>) = parse_result.triggers.iter()
                    .cloned()
                    .partition(|t| t.action == "PLAN");
                const CONNECTOR_ACTIONS: &[&str] = &["GITHUB_READ", "NOTION_READ", "SLACK_READ", "GDRIVE_READ"];
                let (connector_triggers, file_triggers): (Vec<_>, Vec<_>) = rest.into_iter()
                    .partition(|t| CONNECTOR_ACTIONS.contains(&t.action.as_str()));

                for t in &plan_triggers {
                    let allowed = llm::permissions::allowed_triggers_for_level(&perm_level)
                        .map(|a| a.iter().any(|x| x == "PLAN"))
                        .unwrap_or(false);
                    if !allowed {
                        let level_label = llm::permissions::level_label(&perm_level).unwrap_or_default();
                        let detail = format!("Permission denied: {level_label} cannot PLAN");
                        trigger_feedback.push(formatter::format_result("PLAN", "FAIL", &detail));
                        any_trigger_failed = true;
                        continue;
                    }
                    let title = t.params.first().cloned().unwrap_or_else(|| "Untitled plan".to_string());
                    let tasks: Vec<String> = t.params.get(1)
                        .map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
                        .unwrap_or_default();
                    if tasks.is_empty() {
                        trigger_feedback.push(formatter::format_result("PLAN", "FAIL", "No task lines found in plan content"));
                        any_trigger_failed = true;
                        continue;
                    }
                    match crate::commands::plan::create_plan(&db_clone, conv_id, &title, &tasks) {
                        Ok(created) => {
                            log::info!(
                                "Trigger executed (conversation {conv_id}): PLAN OK \"{title}\" with {} steps",
                                created.tasks.len(),
                            );
                            trigger_feedback.push(formatter::format_result(
                                "PLAN", "OK", &format!("Created plan \"{title}\" with {} steps", created.tasks.len()),
                            ));
                            executed_actions.push(("PLAN".to_string(), Some(title.clone())));
                            let _ = app_clone.emit("plan://created", &created);
                        }
                        Err(e) => {
                            log::warn!("Trigger executed (conversation {conv_id}): PLAN FAIL \"{title}\": {}", e.message);
                            trigger_feedback.push(formatter::format_result("PLAN", "FAIL", &e.message));
                            any_trigger_failed = true;
                        }
                    }
                }

                // Each of these hits an external service the user must
                // separately opt into in Settings > Connectors by saving a
                // token — handle_connector_read_trigger covers the shared
                // permission-check/credential-lookup/feedback plumbing;
                // only the actual API call differs per connector.
                for t in &connector_triggers {
                    match t.action.as_str() {
                        "GITHUB_READ" => {
                            let owner_repo = t.params.first().cloned().unwrap_or_default();
                            let path = t.params.get(1).cloned().unwrap_or_default();
                            let target = format!("{owner_repo}/{path}");
                            handle_connector_read_trigger(
                                "GITHUB_READ", target, "github", "GitHub", &perm_level, conv_id, &app_clone,
                                profile_id_clone, &mut trigger_feedback, &mut any_trigger_failed, &mut has_content_feedback,
                                |token| async move { crate::connectors::github::fetch_file(&owner_repo, &path, Some(&token)).await },
                            ).await;
                        }
                        "NOTION_READ" => {
                            let page_id = t.params.first().cloned().unwrap_or_default();
                            let target = page_id.clone();
                            handle_connector_read_trigger(
                                "NOTION_READ", target, "notion", "Notion", &perm_level, conv_id, &app_clone,
                                profile_id_clone, &mut trigger_feedback, &mut any_trigger_failed, &mut has_content_feedback,
                                |token| async move { crate::connectors::notion::fetch_page(&page_id, &token).await },
                            ).await;
                        }
                        "SLACK_READ" => {
                            let channel_id = t.params.first().cloned().unwrap_or_default();
                            let target = channel_id.clone();
                            handle_connector_read_trigger(
                                "SLACK_READ", target, "slack", "Slack", &perm_level, conv_id, &app_clone,
                                profile_id_clone, &mut trigger_feedback, &mut any_trigger_failed, &mut has_content_feedback,
                                |token| async move { crate::connectors::slack::fetch_channel_history(&channel_id, &token).await },
                            ).await;
                        }
                        "GDRIVE_READ" => {
                            let file_id = t.params.first().cloned().unwrap_or_default();
                            handle_gdrive_read(
                                &file_id, &perm_level, conv_id, &app_clone, profile_id_clone,
                                &mut trigger_feedback, &mut any_trigger_failed, &mut has_content_feedback,
                            ).await;
                        }
                        _ => unreachable!("CONNECTOR_ACTIONS partition guarantees only these four actions"),
                    }
                }

                if let Some(ref ws_path) = workspace_path_clone {
                    let exec_result = executor::execute_batch(
                        &file_triggers,
                        ws_path,
                        &perm_level,
                        conv_id,
                        &snapshot_store,
                    );

                    // Build RESULT feedback for the LLM
                    for tr in &exec_result.results {
                        log::info!(
                            "Trigger executed (conversation {conv_id}): {} {} {:?} - {}",
                            tr.action, tr.status, tr.file_path, tr.detail,
                        );
                        trigger_feedback.push(formatter::format_result(
                            &tr.action, &tr.status, &tr.detail,
                        ));
                        // WARN is an expected, documented outcome (e.g. CREATE
                        // finding the file already exists) — not a failure,
                        // so it shouldn't fail a plan step or read as an error.
                        if tr.status == "FAIL" {
                            any_trigger_failed = true;
                        } else if tr.status == "OK" && !matches!(tr.action.as_str(), "LIST" | "READ" | "PREVIEW") {
                            // Only real, completed changes belong in a
                            // synthesized confirmation: WARN (e.g. CREATE
                            // finding the file already exists) didn't actually
                            // do anything, and read-only actions aren't
                            // changes the user needs confirmed.
                            executed_actions.push((tr.action.clone(), tr.file_path.clone()));
                        }

                        // Emit event to frontend
                        let _ = app_clone.emit("trigger://executed", tr);
                    }

                    // Build CONTENT feedback for READ/LIST responses. A READ/LIST
                    // is only useful to the model if it gets to see the result and
                    // act on it — that's what drives auto-continuation below,
                    // since otherwise the conversation would just stall until the
                    // user happens to send another message.
                    for cr in &exec_result.content_responses {
                        trigger_feedback.push(formatter::format_content(
                            &cr.path, &cr.content,
                        ));
                    }
                    has_content_feedback = has_content_feedback || !exec_result.content_responses.is_empty();
                } else {
                    // Triggers were parsed but there's no resolved workspace
                    // path to execute them against — surface this instead of
                    // silently dropping the file operation.
                    for tr in &file_triggers {
                        let detail = "Cannot resolve workspace path; file operation was not executed".to_string();
                        log::warn!("Trigger executed (conversation {conv_id}): {} FAIL - {detail}", tr.action);
                        trigger_feedback.push(formatter::format_result(&tr.action, "FAIL", &detail));
                        any_trigger_failed = true;
                        let result = executor::TriggerResult {
                            action: tr.action.clone(),
                            status: "FAIL".into(),
                            detail,
                            file_path: tr.params.first().cloned(),
                        };
                        let _ = app_clone.emit("trigger://executed", &result);
                    }
                }
            }

            // Emit parse errors to frontend
            for err in &parse_result.errors {
                let _ = app_clone.emit("trigger://error", err);
                trigger_feedback.push(formatter::format_result(
                    "UNKNOWN", "FAIL", &err.message,
                ));
                any_trigger_failed = true;
            }
            if !parse_result.errors.is_empty() && llm::router::is_truncated(&stream_result.finish_reason) {
                trigger_feedback.push(
                    "Note: your previous response appears to have been cut off by the model's \
                     output length limit before a trigger could be closed. Try producing shorter \
                     content, or split large file content across multiple smaller WRITE/APPEND calls."
                        .to_string(),
                );
            }

            let continue_more = has_content_feedback && continuations < MAX_AUTO_CONTINUATIONS;

            // A turn that's 100% triggers with no MESSAGE (weaker models
            // frequently skip it despite the protocol asking for one) leaves
            // nothing for the frontend's own trigger-stripping to display —
            // a blank chat bubble, with the only evidence of what happened
            // tucked away in the action log. Guarantee the user always sees
            // *something* rather than relying on model compliance for it.
            //
            // Crucially, this synthesized text is stored separately from
            // `content` (in `display_override`), never inside `content`
            // itself. `content` always holds the model's raw, unedited
            // output — the same value used both as the frontend's
            // stripping-source for display and as this conversation's
            // history fed back to the model on future turns. Persisting the
            // synthesized prose into `content` instead would teach the
            // model, from its own past turns, to imitate that fabricated
            // "Created X, edited Y." sentence directly instead of emitting
            // real triggers — which is exactly what defeats the file
            // operations the confirmation claims happened.
            let display_override = if continue_more || !parse_result.clean_text.trim().is_empty() || executed_actions.is_empty() {
                None
            } else {
                Some(synthesize_confirmation(&executed_actions))
            };
            let summary_text = display_override.clone().unwrap_or_else(|| parse_result.clean_text.clone());

            if let Ok(db) = db_clone.lock() {
                let _ = db.execute(
                    "UPDATE messages SET content = ?1, status = 'complete', input_tokens = ?2, output_tokens = ?3, display_override = ?4 WHERE id = ?5",
                    rusqlite::params![
                        stream_result.content,
                        stream_result.input_tokens,
                        stream_result.output_tokens,
                        display_override,
                        current_msg_id
                    ],
                );

                // Store trigger feedback as a hidden system message for the
                // LLM's next turn (whether that's this same auto-continuation
                // or a later turn the user kicks off manually).
                if !trigger_feedback.is_empty() {
                    let feedback_content = trigger_feedback.join("\n");
                    let now = now_ms();
                    let _ = db.execute(
                        "INSERT INTO messages (conversation_id, role, content, created_at, status) VALUES (?1, 'system', ?2, ?3, 'complete')",
                        rusqlite::params![conv_id, feedback_content, now],
                    );

                    if continue_more {
                        // Use the raw model output here, not `display_content`: a
                        // turn that's 100% triggers (e.g. just a READ) strips down
                        // to an empty `clean_text`, and an empty-content assistant
                        // message is exactly what providers like Mistral reject on
                        // the next request ("Assistant message must have either
                        // content or tool_calls, but not none"). The raw content
                        // is never empty when the model produced anything at all.
                        current_history.push(("assistant".to_string(), stream_result.content.clone()));
                        current_history.push(("system".to_string(), feedback_content));
                    }
                }
            }

            let _ = app_clone.emit("chat://done", ChatDone {
                message_id: current_msg_id,
                citations: vec![],
                has_more: continue_more,
                display_override: display_override.clone(),
            });

            if !continue_more {
                if let Some(task_id) = plan_task_id {
                    crate::commands::plan::complete_task(&db_clone, &app_clone, task_id, !any_trigger_failed, &summary_text);
                }

                // Auto-title on first exchange, once the whole exchange (all
                // auto-continuations included) has settled.
                if is_first {
                    let title_prompt = format!(
                        "Summarize in 5 words or less. Reply with title only, no punctuation: {content_clone}"
                    );

                    // Titling is a short, cheap, non-critical call — worth a
                    // few retries with backoff so a transient provider error
                    // (rate limiting, a momentary network blip) doesn't
                    // permanently leave the conversation as "New
                    // conversation". Same provider/model each attempt: the
                    // model catalog is a frontend-owned concept (see
                    // ChatInput's provider-availability filtering), so the
                    // backend has no other model/provider list to fall back
                    // through here.
                    const TITLE_RETRY_DELAYS_MS: [u64; 3] = [500, 1500, 4000];
                    let mut title_result = None;
                    for (attempt, delay_ms) in TITLE_RETRY_DELAYS_MS.iter().enumerate() {
                        match llm::stream_chat(
                            &app_clone,
                            -1, // sentinel: don't emit tokens for title gen
                            &provider_clone,
                            &model_clone,
                            vec![("user".to_string(), title_prompt.clone())],
                            profile_id_clone,
                        ).await {
                            Ok(r) => { title_result = Some(r); break; }
                            Err(e) => {
                                log::warn!(
                                    "Auto-title attempt {} failed for {provider_clone}/{model_clone} (conversation {conv_id}): {} (code: {:?})",
                                    attempt + 1, e.message, e.code,
                                );
                                tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
                            }
                        }
                    }

                    if let Some(title_result) = title_result {
                        log::info!(
                            "LLM response ({provider_clone}/{model_clone}, conversation {conv_id}, auto-title): {}",
                            title_result.content,
                        );
                        let title = title_result.content.trim().to_string();
                        if !title.is_empty() {
                            if let Ok(db) = db_clone.lock() {
                                let _ = db.execute(
                                    "UPDATE conversations SET title = ?1 WHERE id = ?2",
                                    rusqlite::params![title, conv_id],
                                );
                            }
                            let _ = app_clone.emit("conversation://titled", serde_json::json!({
                                "id": conv_id,
                                "title": title
                            }));
                        }
                    } else {
                        // The LLM call never succeeded (rate limiting, bad
                        // key, provider outage, ...) — fall back to a title
                        // derived locally from the user's own message rather
                        // than leaving the conversation stuck on "New
                        // conversation" forever with no visible explanation.
                        log::error!(
                            "Auto-title gave up after {} attempts for {provider_clone}/{model_clone} (conversation {conv_id}); using local fallback",
                            TITLE_RETRY_DELAYS_MS.len(),
                        );
                        let title = fallback_title(&content_clone);
                        if let Ok(db) = db_clone.lock() {
                            let _ = db.execute(
                                "UPDATE conversations SET title = ?1 WHERE id = ?2",
                                rusqlite::params![title, conv_id],
                            );
                        }
                        let _ = app_clone.emit("conversation://titled", serde_json::json!({
                            "id": conv_id,
                            "title": title
                        }));
                    }
                }

                // Keep a short, up-to-date resume of the conversation for
                // display above the files panel — refreshed after every
                // settled turn (not each auto-continuation), once there's
                // more than one real message to summarize. Queried fresh
                // from the DB rather than reusing `current_history`: that
                // variable only gets appended to during auto-continuations,
                // so for an ordinary turn it wouldn't yet include this
                // turn's own exchange.
                let recent_turns: Vec<(String, String)> = {
                    let db = db_clone.lock().ok();
                    db.and_then(|db| {
                        db.prepare(
                            "SELECT role, content FROM messages WHERE conversation_id = ?1 \
                             AND role IN ('user','assistant') AND content != '' \
                             ORDER BY created_at DESC LIMIT 6"
                        ).and_then(|mut stmt| {
                            stmt.query_map([conv_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                                .collect::<rusqlite::Result<Vec<_>>>()
                        }).ok()
                    }).unwrap_or_default()
                };
                if recent_turns.len() >= 2 {
                    let transcript: String = recent_turns.iter().rev()
                        .map(|(role, content)| format!("{role}: {content}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let summary_prompt = format!(
                        "Summarize this conversation in one short sentence (under 20 words), for a UI label. Reply with the summary only, no quotes:\n\n{transcript}"
                    );
                    match llm::stream_chat(
                        &app_clone, -1, &provider_clone, &model_clone,
                        vec![("user".to_string(), summary_prompt)],
                        profile_id_clone,
                    ).await {
                        Ok(r) => {
                            let summary = r.content.trim().to_string();
                            if !summary.is_empty() {
                                if let Ok(db) = db_clone.lock() {
                                    let _ = db.execute(
                                        "UPDATE conversations SET summary = ?1 WHERE id = ?2",
                                        rusqlite::params![summary, conv_id],
                                    );
                                }
                                let _ = app_clone.emit("conversation://summarized", serde_json::json!({
                                    "id": conv_id,
                                    "summary": summary
                                }));
                            }
                        }
                        Err(e) => {
                            log::warn!(
                                "Conversation summary generation failed for {provider_clone}/{model_clone} (conversation {conv_id}): {} (code: {:?})",
                                e.message, e.code,
                            );
                        }
                    }
                }

                // Auto-fill the project description from its own context.md
                // once one exists — but only while the description is still
                // empty, so this never overwrites something the user wrote
                // or edited themselves (see workspace_set_description).
                if let (Some(ws_id), Some(ref ws_path)) = (workspace_id_clone, &workspace_path_clone) {
                    let needs_description = db_clone.lock().ok()
                        .and_then(|db| db.query_row(
                            "SELECT description FROM workspaces WHERE id = ?1",
                            [ws_id],
                            |r| r.get::<_, Option<String>>(0),
                        ).ok())
                        .map(|d| d.map(|s| s.trim().is_empty()).unwrap_or(true))
                        .unwrap_or(false);

                    if needs_description {
                        if let Some(context_md) = llm::skills::read_context_md(ws_path) {
                            let description_prompt = format!(
                                "Based on this project's own notes, write a one-sentence description \
                                 (under 20 words) of what this project is. Reply with the description \
                                 only, no quotes:\n\n{context_md}"
                            );
                            match llm::stream_chat(
                                &app_clone, -1, &provider_clone, &model_clone,
                                vec![("user".to_string(), description_prompt)],
                                profile_id_clone,
                            ).await {
                                Ok(r) => {
                                    let description = r.content.trim().to_string();
                                    if !description.is_empty() {
                                        if let Ok(db) = db_clone.lock() {
                                            let _ = db.execute(
                                                "UPDATE workspaces SET description = ?1 WHERE id = ?2",
                                                rusqlite::params![description, ws_id],
                                            );
                                        }
                                        let _ = app_clone.emit("workspace://described", serde_json::json!({
                                            "workspace_id": ws_id,
                                            "description": description
                                        }));
                                    }
                                }
                                Err(e) => {
                                    log::warn!(
                                        "Workspace description generation failed for {provider_clone}/{model_clone} (workspace {ws_id}): {} (code: {:?})",
                                        e.message, e.code,
                                    );
                                }
                            }
                        }
                    }
                }

                break;
            }

            continuations += 1;

            // Create the next placeholder assistant message and tell the
            // frontend about it before streaming starts, so its chat://token
            // deltas land somewhere.
            let next_msg_id = {
                let db = match db_clone.lock() { Ok(d) => d, Err(_) => break };
                let now = now_ms();
                let _ = db.execute(
                    "INSERT INTO messages (conversation_id, role, content, created_at, status, provider, model) VALUES (?1, 'assistant', '', ?2, 'streaming', ?3, ?4)",
                    rusqlite::params![conv_id, now, provider_clone, model_clone],
                );
                db.last_insert_rowid()
            };

            let next_msg = {
                let db = match db_clone.lock() { Ok(d) => d, Err(_) => break };
                match db.query_row(
                    "SELECT id, conversation_id, role, content, created_at, token_count, input_tokens, output_tokens, error, status, provider, model, display_override FROM messages WHERE id = ?1",
                    [next_msg_id], row_to_message,
                ) {
                    Ok(m) => m,
                    Err(_) => break,
                }
            };

            let _ = app_clone.emit("chat://continuation", ChatContinuation { message: next_msg });
            current_msg_id = next_msg_id;
        }
    });

    // Return the placeholder message immediately so frontend knows the id
    let msg = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        db.query_row(
            "SELECT id, conversation_id, role, content, created_at, token_count, input_tokens, output_tokens, error, status, provider, model, display_override FROM messages WHERE id = ?1",
            [assistant_msg_id], row_to_message,
        )?
    };
    Ok(msg)
}

#[tauri::command]
pub fn tool_list(message_id: i64, db: State<Db>) -> Result<Vec<ToolCall>> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let mut stmt = db.prepare(
        "SELECT id, message_id, tool_name, arguments_json, status, result_json, created_at FROM tool_calls WHERE message_id = ?1"
    )?;
    let rows = stmt.query_map([message_id], |row| Ok(ToolCall {
        id: row.get(0)?,
        message_id: row.get(1)?,
        tool_name: row.get(2)?,
        arguments_json: row.get(3)?,
        status: row.get(4)?,
        result_json: row.get(5)?,
        created_at: row.get(6)?,
    }))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[tauri::command]
pub fn tool_approve(tool_call_id: i64, db: State<Db>) -> Result<ToolCall> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute("UPDATE tool_calls SET status = 'approved' WHERE id = ?1", [tool_call_id])?;
    let tc = db.query_row(
        "SELECT id, message_id, tool_name, arguments_json, status, result_json, created_at FROM tool_calls WHERE id = ?1",
        [tool_call_id],
        |row| Ok(ToolCall {
            id: row.get(0)?,
            message_id: row.get(1)?,
            tool_name: row.get(2)?,
            arguments_json: row.get(3)?,
            status: row.get(4)?,
            result_json: row.get(5)?,
            created_at: row.get(6)?,
        }),
    )?;
    Ok(tc)
}

#[tauri::command]
pub fn tool_reject(tool_call_id: i64, db: State<Db>) -> Result<ToolCall> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute("UPDATE tool_calls SET status = 'rejected' WHERE id = ?1", [tool_call_id])?;
    let tc = db.query_row(
        "SELECT id, message_id, tool_name, arguments_json, status, result_json, created_at FROM tool_calls WHERE id = ?1",
        [tool_call_id],
        |row| Ok(ToolCall {
            id: row.get(0)?,
            message_id: row.get(1)?,
            tool_name: row.get(2)?,
            arguments_json: row.get(3)?,
            status: row.get(4)?,
            result_json: row.get(5)?,
            created_at: row.get(6)?,
        }),
    )?;
    Ok(tc)
}

#[tauri::command]
pub fn search_hybrid(workspace_id: i64, query: String, limit: Option<i64>, db: State<Db>) -> Result<Vec<Citation>> {
    let limit = limit.unwrap_or(8) as usize;
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;

    // BM25 full-text results
    let escaped = query.replace('"', "\"\"").replace('*', "").replace('^', "");
    let fts_query = format!("\"{}\"", escaped);

    let bm25: Vec<(i64, f64)> = db.prepare(
        "SELECT c.id, -rank as score
         FROM chunks_fts
         JOIN chunks c ON chunks_fts.rowid = c.id
         JOIN files f ON c.file_id = f.id
         WHERE f.workspace_id = ?1 AND chunks_fts MATCH ?2
         ORDER BY rank LIMIT 50"
    ).and_then(|mut s| {
        s.query_map(rusqlite::params![workspace_id, fts_query], |r| Ok((r.get(0)?, r.get(1)?)))?
         .collect::<rusqlite::Result<Vec<_>>>()
    }).unwrap_or_default();

    // Vector results from chunk_embeddings table (cosine similarity approximated by dot product on normalized vectors)
    // We compute this in Rust since sqlite-vec extension may not be loaded.
    // For now, fall back to BM25-only if no embeddings exist.
    let _has_embeddings: bool = db.query_row(
        "SELECT COUNT(*) FROM chunk_embeddings ce JOIN chunks c ON ce.chunk_id=c.id JOIN files f ON c.file_id=f.id WHERE f.workspace_id=?1",
        [workspace_id], |r| r.get::<_, i64>(0),
    ).unwrap_or(0) > 0;

    // RRF fusion (k=60)
    const K: f64 = 60.0;
    let mut scores: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();

    for (rank, (chunk_id, _score)) in bm25.iter().enumerate() {
        *scores.entry(*chunk_id).or_insert(0.0) += 1.0 / (K + rank as f64 + 1.0);
    }

    // If we have embeddings, we'd add vector ranks here (omitted until sqlite-vec loaded)
    // The fusion still works with BM25-only; adding vector scores improves recall when available.

    // Sort by fused score and fetch top results
    let mut ranked: Vec<(i64, f64)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(limit);

    if ranked.is_empty() { return Ok(vec![]); }

    let ids: Vec<String> = ranked.iter().map(|(id, _)| id.to_string()).collect();
    let placeholders = ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 2)).collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT c.id, c.file_id, f.rel_path, c.content, c.page, c.heading
         FROM chunks c JOIN files f ON c.file_id = f.id
         WHERE c.id IN ({placeholders}) AND f.workspace_id = ?1"
    );

    let mut stmt = db.prepare(&sql).map_err(AtelierError::from)?;
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(workspace_id)];
    for id in &ranked { params.push(Box::new(id.0)); }

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let chunk_map: std::collections::HashMap<i64, Citation> = stmt.query_map(param_refs.as_slice(), |row| {
        let id: i64 = row.get(0)?;
        Ok((id, Citation {
            id: 0, message_id: 0,
            chunk_id: Some(id),
            file_id: Some(row.get(1)?),
            rel_path: row.get(2)?,
            snippet: row.get::<_, String>(3)?.chars().take(300).collect(),
            page: row.get(4)?,
            heading: row.get(5)?,
            rank: 0, score: 0.0,
        }))
    }).and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
    .unwrap_or_default()
    .into_iter()
    .map(|(id, c)| (id, c))
    .collect();

    let result = ranked.iter().enumerate().filter_map(|(i, (chunk_id, score))| {
        chunk_map.get(chunk_id).map(|c| Citation {
            rank: i as i64,
            score: *score,
            ..c.clone()
        })
    }).collect();

    Ok(result)
}

#[tauri::command]
pub fn undo_trigger(
    conversation_id: i64,
    snapshots: State<std::sync::Arc<SnapshotStore>>,
) -> Result<crate::triggers::snapshot::UndoResult> {
    snapshots.undo_last(conversation_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_title_takes_the_first_few_words_and_capitalizes() {
        assert_eq!(fallback_title("how do I center a div in css exactly"), "How do I center a div in css");
    }

    #[test]
    fn fallback_title_truncates_a_long_single_run() {
        let long = "a".repeat(100);
        let title = fallback_title(&long);
        assert!(title.ends_with('…'));
        assert!(title.chars().count() <= 61); // MAX_CHARS + the ellipsis char
    }

    #[test]
    fn fallback_title_handles_empty_content() {
        assert_eq!(fallback_title(""), "New conversation");
        assert_eq!(fallback_title("   "), "New conversation");
    }

    #[test]
    fn synthesize_confirmation_describes_a_single_action() {
        let actions = vec![("CREATE".to_string(), Some("plan.md".to_string()))];
        assert_eq!(synthesize_confirmation(&actions), "Created [plan.md](<atelier-file:plan.md>).");
    }

    #[test]
    fn synthesize_confirmation_joins_multiple_actions() {
        let actions = vec![
            ("CREATE".to_string(), Some("plan.md".to_string())),
            ("WRITE".to_string(), Some("plan.md".to_string())),
            ("EXPORT_PDF".to_string(), Some("plan.pdf".to_string())),
        ];
        assert_eq!(
            synthesize_confirmation(&actions),
            "Created [plan.md](<atelier-file:plan.md>), edited [plan.md](<atelier-file:plan.md>), exported [plan.pdf](<atelier-file:plan.pdf>)."
        );
    }

    #[test]
    fn synthesize_confirmation_wraps_spaced_filenames_in_angle_brackets() {
        // A bare markdown link destination can't contain spaces — real file
        // names very often do (e.g. an office-doc slug with spaces).
        let actions = vec![("CREATE_DOCX".to_string(), Some("business plan.docx".to_string()))];
        assert_eq!(
            synthesize_confirmation(&actions),
            "Created [business plan.docx](<atelier-file:business plan.docx>)."
        );
    }

    #[test]
    fn synthesize_confirmation_plan_title_is_not_a_file_link() {
        // PLAN's "path" slot is actually the plan's title, not a real file —
        // it must stay plain text, not a broken link to a nonexistent file.
        let actions = vec![("PLAN".to_string(), Some("Launch prep".to_string()))];
        assert_eq!(synthesize_confirmation(&actions), "Planned Launch prep.");
    }

    #[test]
    fn synthesize_confirmation_empty_input_is_empty() {
        assert_eq!(synthesize_confirmation(&[]), "");
    }

    /// Regression test for a history-contamination bug: a synthesized
    /// confirmation must never be readable through the same `content`
    /// column that conversation history is rebuilt from, only through the
    /// separate `display_override` column. If `content` ever picked up the
    /// synthesized text again, the model would start imitating fabricated
    /// "Created X, edited Y." prose instead of emitting real triggers on
    /// later turns (see run_turn's comment on `display_override` for the
    /// full mechanism).
    #[test]
    fn display_override_is_never_visible_through_the_history_column() {
        let db = crate::db::open_in_memory().unwrap();
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "INSERT INTO profiles (name, dir_name, root_path, created_at, last_active_at) VALUES ('p', 'p', '/tmp/p', 0, 0)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO workspaces (profile_id, name, path, created_at, last_opened_at) VALUES (1, 'w', '/tmp/w', 0, 0)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO conversations (workspace_id, title, created_at, updated_at) VALUES (1, 't', 0, 0)",
                [],
            ).unwrap();
            let raw_content = ">>>[CREATE_DOCX \"report.docx\" \"# Title\"]<<<";
            let synthesized = synthesize_confirmation(&[("CREATE_DOCX".to_string(), Some("report.docx".to_string()))]);
            conn.execute(
                "INSERT INTO messages (conversation_id, role, content, created_at, status, display_override) VALUES (1, 'assistant', ?1, 0, 'complete', ?2)",
                rusqlite::params![raw_content, synthesized],
            ).unwrap();
        }

        let conn = db.lock().unwrap();

        // The history-rebuilding query (mirrors the one in run_turn) must
        // only ever see the raw content, never the synthesized text.
        let history_content: String = conn.query_row(
            "SELECT content FROM messages WHERE role = 'assistant'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert!(history_content.contains(">>>[CREATE_DOCX"));
        assert!(!history_content.contains("Created [report.docx]"));

        // row_to_message must still surface the synthesized text separately.
        let msg = conn.query_row(
            "SELECT id, conversation_id, role, content, created_at, token_count, input_tokens, output_tokens, error, status, provider, model, display_override FROM messages WHERE role = 'assistant'",
            [],
            row_to_message,
        ).unwrap();
        assert_eq!(msg.content, history_content);
        assert_eq!(msg.display_override.as_deref(), Some("Created [report.docx](<atelier-file:report.docx>)."));
    }

    fn temp_workspace_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("atelier_chat_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn build_related_context_includes_parent_and_child_context_md() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock().unwrap();
        conn.execute(
            "INSERT INTO profiles (name, dir_name, root_path, created_at, last_active_at) VALUES ('p', 'p', '/tmp/p', 0, 0)",
            [],
        ).unwrap();

        let parent_dir = temp_workspace_dir();
        std::fs::write(parent_dir.join("context.md"), "Parent index: manages billing.").unwrap();
        conn.execute(
            "INSERT INTO workspaces (profile_id, name, path, created_at, last_opened_at) VALUES (1, 'Parent', ?1, 0, 0)",
            [parent_dir.to_str().unwrap()],
        ).unwrap();
        let parent_id = conn.last_insert_rowid();

        let child_dir = temp_workspace_dir();
        std::fs::write(child_dir.join("context.md"), "Child index: invoicing module.").unwrap();
        conn.execute(
            "INSERT INTO workspaces (profile_id, name, path, created_at, last_opened_at, parent_workspace_id) VALUES (1, 'Child', ?1, 0, 0, ?2)",
            rusqlite::params![child_dir.to_str().unwrap(), parent_id],
        ).unwrap();
        let child_id = conn.last_insert_rowid();

        let from_child = build_related_context(&conn, child_id);
        assert!(from_child.contains("Parent project \"Parent\""));
        assert!(from_child.contains("Parent index: manages billing."));

        let from_parent = build_related_context(&conn, parent_id);
        assert!(from_parent.contains("Sub-project \"Child\""));
        assert!(from_parent.contains("Child index: invoicing module."));

        std::fs::remove_dir_all(&parent_dir).ok();
        std::fs::remove_dir_all(&child_dir).ok();
    }

    #[test]
    fn build_related_context_is_empty_for_a_standalone_project() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock().unwrap();
        conn.execute(
            "INSERT INTO profiles (name, dir_name, root_path, created_at, last_active_at) VALUES ('p', 'p', '/tmp/p', 0, 0)",
            [],
        ).unwrap();
        let dir = temp_workspace_dir();
        conn.execute(
            "INSERT INTO workspaces (profile_id, name, path, created_at, last_opened_at) VALUES (1, 'Solo', ?1, 0, 0)",
            [dir.to_str().unwrap()],
        ).unwrap();
        let id = conn.last_insert_rowid();

        assert_eq!(build_related_context(&conn, id), "");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Regression test for the history-building branch in run_turn: once a
    /// conversation is compressed, only messages created after the
    /// compression point should count as "real" history — everything
    /// before it is represented by the memory block instead. Duplicates
    /// the query rather than calling run_turn directly since run_turn is
    /// an async, LLM-streaming function with no seam for a mock provider.
    #[test]
    fn compressed_conversation_history_only_includes_messages_after_compression() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock().unwrap();
        conn.execute(
            "INSERT INTO profiles (name, dir_name, root_path, created_at, last_active_at) VALUES ('p', 'p', '/tmp/p', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO workspaces (profile_id, name, path, created_at, last_opened_at) VALUES (1, 'w', '/tmp/w', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO conversations (workspace_id, title, created_at, updated_at) VALUES (1, 't', 0, 0)",
            [],
        ).unwrap();
        let conv_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at, status) VALUES (?1, 'user', 'old message', 100, 'complete')",
            [conv_id],
        ).unwrap();
        conn.execute(
            "UPDATE conversations SET compressed_memory = 'the memory', compressed_at = 500 WHERE id = ?1",
            [conv_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at, status) VALUES (?1, 'user', 'new message', 900, 'complete')",
            [conv_id],
        ).unwrap();

        let compressed_at: i64 = conn.query_row(
            "SELECT compressed_at FROM conversations WHERE id = ?1", [conv_id], |r| r.get(0),
        ).unwrap();

        let mut stmt = conn.prepare(
            "SELECT content FROM messages WHERE conversation_id = ?1 AND status != 'streaming' AND content != '' AND created_at > ?2 ORDER BY created_at ASC"
        ).unwrap();
        let history: Vec<String> = stmt.query_map(rusqlite::params![conv_id, compressed_at], |r| r.get(0)).unwrap()
            .collect::<rusqlite::Result<_>>().unwrap();

        assert_eq!(history, vec!["new message".to_string()]);
    }

    #[test]
    fn fork_copies_only_messages_up_to_the_chosen_point() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock().unwrap();
        conn.execute(
            "INSERT INTO profiles (name, dir_name, root_path, created_at, last_active_at) VALUES ('p', 'p', '/tmp/p', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO workspaces (profile_id, name, path, created_at, last_opened_at) VALUES (1, 'w', '/tmp/w', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO conversations (workspace_id, title, created_at, updated_at, provider, model) VALUES (1, 'Original', 0, 0, 'anthropic', 'claude')",
            [],
        ).unwrap();
        let conv_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at, status) VALUES (?1, 'user', 'hi', 100, 'complete')",
            [conv_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at, status) VALUES (?1, 'assistant', 'hello', 200, 'complete')",
            [conv_id],
        ).unwrap();
        let fork_point_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at, status) VALUES (?1, 'user', 'a later message', 300, 'complete')",
            [conv_id],
        ).unwrap();

        let forked = fork_impl(&conn, conv_id, fork_point_id).unwrap();
        assert_eq!(forked.title, "Original (fork)");
        assert_eq!(forked.workspace_id, 1);

        let contents: Vec<String> = conn.prepare(
            "SELECT content FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC"
        ).unwrap().query_map([forked.id], |r| r.get(0)).unwrap()
            .collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(contents, vec!["hi".to_string(), "hello".to_string()]);

        // The original conversation must be untouched.
        let original_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1", [conv_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(original_count, 3);
    }

    #[test]
    fn fork_rejects_a_message_from_another_conversation() {
        let db = crate::db::open_in_memory().unwrap();
        let conn = db.lock().unwrap();
        conn.execute(
            "INSERT INTO profiles (name, dir_name, root_path, created_at, last_active_at) VALUES ('p', 'p', '/tmp/p', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO workspaces (profile_id, name, path, created_at, last_opened_at) VALUES (1, 'w', '/tmp/w', 0, 0)",
            [],
        ).unwrap();
        conn.execute("INSERT INTO conversations (workspace_id, title, created_at, updated_at) VALUES (1, 'A', 0, 0)", []).unwrap();
        let conv_a = conn.last_insert_rowid();
        conn.execute("INSERT INTO conversations (workspace_id, title, created_at, updated_at) VALUES (1, 'B', 0, 0)", []).unwrap();
        let conv_b = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at, status) VALUES (?1, 'user', 'hi', 100, 'complete')",
            [conv_b],
        ).unwrap();
        let msg_in_b = conn.last_insert_rowid();

        assert!(fork_impl(&conn, conv_a, msg_in_b).is_err());
    }
}
