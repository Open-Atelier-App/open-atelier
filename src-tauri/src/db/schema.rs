pub const SCHEMA: &str = r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;

CREATE TABLE IF NOT EXISTS profiles (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT    NOT NULL,
    dir_name      TEXT    NOT NULL,
    root_path     TEXT    NOT NULL UNIQUE,
    created_at    INTEGER NOT NULL,
    last_active_at INTEGER NOT NULL,
    is_active     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS window_state (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id   INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    window_label TEXT    NOT NULL,
    tabs_json    TEXT    NOT NULL DEFAULT '[]',
    active_tab   INTEGER NOT NULL DEFAULT 0,
    bounds_json  TEXT
);

CREATE TABLE IF NOT EXISTS workspaces (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id     INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    path           TEXT    NOT NULL UNIQUE,
    name           TEXT    NOT NULL,
    created_at     INTEGER NOT NULL,
    last_opened_at INTEGER NOT NULL,
    index_status   TEXT    NOT NULL DEFAULT 'idle'
                   CHECK(index_status IN ('idle','indexing','complete','error')),
    settings_json  TEXT,
    parent_workspace_id INTEGER REFERENCES workspaces(id) ON DELETE SET NULL,
    description    TEXT
);

CREATE TABLE IF NOT EXISTS files (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    rel_path     TEXT    NOT NULL,
    abs_path     TEXT    NOT NULL,
    ext          TEXT,
    size_bytes   INTEGER NOT NULL DEFAULT 0,
    mtime        INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT,
    index_state  TEXT    NOT NULL DEFAULT 'pending'
                 CHECK(index_state IN ('pending','indexed','skipped','error')),
    skip_reason  TEXT,
    indexed_at   INTEGER,
    UNIQUE(workspace_id, rel_path)
);

CREATE INDEX IF NOT EXISTS idx_files_workspace ON files(workspace_id);
CREATE INDEX IF NOT EXISTS idx_files_workspace_state ON files(workspace_id, index_state);
CREATE INDEX IF NOT EXISTS idx_files_hash ON files(content_hash) WHERE content_hash IS NOT NULL;

CREATE TABLE IF NOT EXISTS chunks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id     INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    content     TEXT    NOT NULL,
    token_count INTEGER,
    char_start  INTEGER NOT NULL DEFAULT 0,
    char_end    INTEGER NOT NULL DEFAULT 0,
    page        INTEGER,
    heading     TEXT,
    embed_state TEXT    NOT NULL DEFAULT 'pending'
                CHECK(embed_state IN ('pending','done','error'))
);

CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_id);
CREATE INDEX IF NOT EXISTS idx_chunks_embed ON chunks(embed_state);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    content,
    content='chunks',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS chunks_fts_insert AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_delete AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, content) VALUES ('delete', old.id, old.content);
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_update AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, content) VALUES ('delete', old.id, old.content);
    INSERT INTO chunks_fts(rowid, content) VALUES (new.id, new.content);
END;

-- User-defined folders for organizing a workspace's conversations by
-- subject (see Settings-free drag-and-drop grouping in ConversationList).
-- `position` is a plain integer used only for user-driven drag-to-reorder
-- of the groups themselves, not date/recency.
CREATE TABLE IF NOT EXISTS conversation_groups (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name         TEXT    NOT NULL,
    position     INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_conversation_groups_workspace ON conversation_groups(workspace_id, position);

CREATE TABLE IF NOT EXISTS conversations (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title        TEXT    NOT NULL DEFAULT 'New conversation',
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    provider     TEXT,
    model        TEXT,
    summary      TEXT,
    compressed_memory TEXT,
    compressed_at     INTEGER,
    group_id     INTEGER REFERENCES conversation_groups(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_conversations_workspace ON conversations(workspace_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS messages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role            TEXT    NOT NULL CHECK(role IN ('system','user','assistant')),
    content         TEXT    NOT NULL,
    created_at      INTEGER NOT NULL,
    token_count     INTEGER,
    input_tokens    INTEGER,
    output_tokens   INTEGER,
    error           TEXT,
    status          TEXT    NOT NULL DEFAULT 'complete'
                    CHECK(status IN ('streaming','complete','error','cancelled')),
    provider        TEXT,
    model           TEXT,
    display_override TEXT
);

CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id, created_at);

CREATE TABLE IF NOT EXISTS citations (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    chunk_id   INTEGER REFERENCES chunks(id) ON DELETE SET NULL,
    file_id    INTEGER REFERENCES files(id) ON DELETE SET NULL,
    rel_path   TEXT    NOT NULL,
    page       INTEGER,
    heading    TEXT,
    snippet    TEXT    NOT NULL,
    rank       INTEGER NOT NULL DEFAULT 0,
    score      REAL    NOT NULL DEFAULT 0.0
);

CREATE INDEX IF NOT EXISTS idx_citations_message ON citations(message_id, rank);

CREATE TABLE IF NOT EXISTS saved_documents (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    message_id   INTEGER REFERENCES messages(id) ON DELETE SET NULL,
    rel_path     TEXT    NOT NULL,
    created_at   INTEGER NOT NULL
);

-- Embedding storage: JSON fallback when sqlite-vec extension unavailable
CREATE TABLE IF NOT EXISTS chunk_embeddings (
    chunk_id       INTEGER PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
    embedding_json TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS tool_calls (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id     INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    tool_name      TEXT    NOT NULL,
    arguments_json TEXT    NOT NULL,
    status         TEXT    NOT NULL DEFAULT 'pending'
                   CHECK(status IN ('pending','approved','rejected','executed','error')),
    result_json    TEXT,
    created_at     INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tool_calls_message ON tool_calls(message_id);

CREATE TABLE IF NOT EXISTS plans (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    title           TEXT    NOT NULL,
    status          TEXT    NOT NULL DEFAULT 'pending'
                    CHECK(status IN ('pending','running','done','failed')),
    created_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_plans_conversation ON plans(conversation_id, created_at);

CREATE TABLE IF NOT EXISTS plan_tasks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id     INTEGER NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    description TEXT    NOT NULL,
    status      TEXT    NOT NULL DEFAULT 'pending'
                CHECK(status IN ('pending','running','done','failed')),
    summary     TEXT,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_plan_tasks_plan ON plan_tasks(plan_id, seq);
"#;
