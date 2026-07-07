use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub dir_name: String,
    pub root_path: String,
    pub created_at: i64,
    pub last_active_at: i64,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workspace {
    pub id: i64,
    pub profile_id: i64,
    pub path: String,
    pub name: String,
    pub created_at: i64,
    pub last_opened_at: i64,
    pub index_status: String,
    pub settings_json: Option<String>,
    /// A sub-project's parent, if any. Sub-projects share context with
    /// their parent in both directions — see `related_context` in
    /// commands/chat.rs — but otherwise behave as ordinary, independent
    /// workspaces (own files, conversations, index).
    pub parent_workspace_id: Option<i64>,
    /// A one-sentence, user-editable summary shown under the project name.
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceFile {
    pub id: i64,
    pub workspace_id: i64,
    pub rel_path: String,
    pub abs_path: String,
    pub ext: Option<String>,
    pub size_bytes: i64,
    pub mtime: i64,
    pub content_hash: Option<String>,
    pub index_state: String,
    pub skip_reason: Option<String>,
    pub indexed_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileNode {
    pub name: String,
    pub rel_path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileNode>>,
    pub size_bytes: Option<i64>,
    pub ext: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Chunk {
    pub id: i64,
    pub file_id: i64,
    pub chunk_index: i64,
    pub content: String,
    pub token_count: Option<i64>,
    pub char_start: i64,
    pub char_end: i64,
    pub page: Option<i64>,
    pub heading: Option<String>,
    pub embed_state: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Conversation {
    pub id: i64,
    pub workspace_id: i64,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub provider: Option<String>,
    pub model: Option<String>,
    /// A short, LLM-generated resume of the conversation so far — shown
    /// above the files panel. Refreshed after each settled turn once
    /// there's more than one message to summarize; see run_turn.
    pub summary: Option<String>,
    /// A thorough, LLM-generated memory of everything before
    /// `compressed_at`, written by conversation_compress. Once set,
    /// run_turn builds history from `compressed_memory` plus only the
    /// messages created after `compressed_at`, instead of the full raw
    /// transcript — letting the user keep chatting in the same thread
    /// (optionally switching provider/model) without re-sending the whole
    /// history on every turn.
    pub compressed_memory: Option<String>,
    pub compressed_at: Option<i64>,
    /// Which user-defined folder (see ConversationGroup) this conversation
    /// is filed under, if any — set by drag-and-drop in ConversationList.
    pub group_id: Option<i64>,
}

/// A user-defined folder for organizing a workspace's conversations by
/// subject — purely organizational, created/renamed/reordered by the user
/// via drag-and-drop in the sidebar; conversations are assigned to one by
/// setting their own `group_id`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConversationGroup {
    pub id: i64,
    pub workspace_id: i64,
    pub name: String,
    pub position: i64,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: i64,
    pub conversation_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: i64,
    pub token_count: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub error: Option<String>,
    pub status: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    /// Set only when `content` (the raw model output, also fed back to the
    /// model as its own conversation history) isn't fit to show the user
    /// as-is — e.g. a turn that was all triggers and no MESSAGE gets a
    /// synthesized "Created X, edited Y." confirmation here instead. The
    /// frontend renders this in place of stripping triggers from `content`
    /// when present. Never fed back to the model: see run_turn in
    /// commands/chat.rs for why that distinction matters.
    pub display_override: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Citation {
    pub id: i64,
    pub message_id: i64,
    pub chunk_id: Option<i64>,
    pub file_id: Option<i64>,
    pub rel_path: String,
    pub page: Option<i64>,
    pub heading: Option<String>,
    pub snippet: String,
    pub rank: i64,
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedDocument {
    pub id: i64,
    pub workspace_id: i64,
    pub message_id: Option<i64>,
    pub rel_path: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: i64,
    pub message_id: i64,
    pub tool_name: String,
    pub arguments_json: String,
    pub status: String,
    pub result_json: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyStatus {
    pub provider: String,
    pub exists: bool,
    /// Which storage backend the credential lives in: "keychain" |
    /// "local_encrypted" | "none".
    pub backend: String,
}

// Event payloads emitted to frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexProgress {
    pub workspace_id: i64,
    pub done: u64,
    pub total: u64,
    pub current_file: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatToken {
    pub message_id: i64,
    pub delta: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatDone {
    pub message_id: i64,
    pub citations: Vec<Citation>,
    /// True when this message's trigger results (a READ/LIST) require the
    /// model to see them and keep going — a new assistant message has
    /// already been created and a `chat://continuation` event fired for it.
    /// The frontend should NOT clear its "streaming" indicator when this is
    /// true; another message is already on its way.
    pub has_more: bool,
    /// Mirrors the message row's `display_override` — the frontend applies
    /// it immediately so a same-session, all-triggers-no-MESSAGE turn shows
    /// its synthesized confirmation right away instead of only after the
    /// conversation is next reloaded from the DB.
    pub display_override: Option<String>,
}

/// Emitted when a turn's trigger results (e.g. a READ) require the model to
/// continue automatically without new user input. Carries the freshly
/// created placeholder assistant message so the frontend can append it to
/// the conversation before its `chat://token` deltas start arriving.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatContinuation {
    pub message: Message,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatError {
    pub message_id: i64,
    pub error: crate::error::AtelierError,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolProposed {
    pub tool_call_id: i64,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConversationWithMessages {
    pub conversation: Conversation,
    pub messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Plan {
    pub id: i64,
    pub conversation_id: i64,
    pub title: String,
    pub status: String, // pending | running | done | failed
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlanTask {
    pub id: i64,
    pub plan_id: i64,
    pub seq: i64,
    pub description: String,
    pub status: String, // pending | running | done | failed
    pub summary: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlanWithTasks {
    pub plan: Plan,
    pub tasks: Vec<PlanTask>,
}
