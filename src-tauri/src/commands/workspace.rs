use std::path::Path;
use tauri::{AppHandle, State};
use crate::db::{Db, now_ms};
use crate::error::{AtelierError, Result};
use crate::models::Workspace;
use crate::llm;

const WORKSPACE_COLUMNS: &str = "id, profile_id, path, name, created_at, last_opened_at, index_status, settings_json, parent_workspace_id, description";

fn row_to_workspace(row: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        path: row.get(2)?,
        name: row.get(3)?,
        created_at: row.get(4)?,
        last_opened_at: row.get(5)?,
        index_status: row.get(6)?,
        settings_json: row.get(7)?,
        parent_workspace_id: row.get(8)?,
        description: row.get(9)?,
    })
}

fn path_to_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled")
        .to_string()
}

#[tauri::command]
pub fn workspace_open(path: String, parent_workspace_id: Option<i64>, db: State<Db>) -> Result<Workspace> {
    if !Path::new(&path).exists() {
        return Err(AtelierError::not_found(format!("Path does not exist: {path}")));
    }
    let name = path_to_name(&path);
    let now = now_ms();

    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;

    // Get active profile
    let (profile_id, profile_root): (i64, String) = db.query_row(
        "SELECT id, root_path FROM profiles WHERE is_active = 1 LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|_| AtelierError::internal("No active profile"))?;

    // Projects must live inside the active profile's folder, so the
    // workspace list always reflects "what belongs to this profile"
    // instead of mixing in unrelated locations on disk.
    let canonical_path = Path::new(&path).canonicalize()
        .map_err(|e| AtelierError::io(format!("Cannot resolve path {path}: {e}")))?;
    let canonical_root = Path::new(&profile_root).canonicalize()
        .map_err(|e| AtelierError::io(format!("Cannot resolve profile directory {profile_root}: {e}")))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(AtelierError::new(
            crate::error::ErrorCode::PathNotAllowed,
            format!("Projects must be inside the active profile's folder ({profile_root})"),
        ));
    }

    if let Some(parent_id) = parent_workspace_id {
        let parent_profile_id: i64 = db.query_row(
            "SELECT profile_id FROM workspaces WHERE id = ?1",
            [parent_id],
            |r| r.get(0),
        ).map_err(|_| AtelierError::not_found("Parent project not found"))?;
        if parent_profile_id != profile_id {
            return Err(AtelierError::new(
                crate::error::ErrorCode::PathNotAllowed,
                "Parent project must belong to the same profile",
            ));
        }
    }

    // Upsert workspace. parent_workspace_id is only applied on first
    // creation (the INSERT branch) — reopening an already-known path
    // shouldn't silently reparent it.
    db.execute(
        "INSERT INTO workspaces (profile_id, path, name, created_at, last_opened_at, index_status, parent_workspace_id)
         VALUES (?1, ?2, ?3, ?4, ?5, 'idle', ?6)
         ON CONFLICT(path) DO UPDATE SET last_opened_at = excluded.last_opened_at",
        rusqlite::params![profile_id, path, name, now, now, parent_workspace_id],
    )?;

    let ws = db.query_row(
        &format!("SELECT {WORKSPACE_COLUMNS} FROM workspaces WHERE path = ?1"),
        [&path],
        row_to_workspace,
    )?;
    Ok(ws)
}

#[tauri::command]
pub fn workspace_list(profile_id: i64, db: State<Db>) -> Result<Vec<Workspace>> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let mut stmt = db.prepare(
        &format!("SELECT {WORKSPACE_COLUMNS} FROM workspaces WHERE profile_id = ?1 ORDER BY last_opened_at DESC")
    )?;
    let rows = stmt.query_map([profile_id], row_to_workspace)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[tauri::command]
pub fn workspace_set_parent(id: i64, parent_workspace_id: Option<i64>, db: State<Db>) -> Result<Workspace> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    set_parent_impl(&db, id, parent_workspace_id)
}

/// Sets or clears (pass an empty/whitespace-only string) the one-sentence
/// description shown under the project name once it's selected.
#[tauri::command]
pub fn workspace_set_description(id: i64, description: String, db: State<Db>) -> Result<Workspace> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let trimmed = description.trim();
    let value: Option<&str> = if trimmed.is_empty() { None } else { Some(trimmed) };
    db.execute("UPDATE workspaces SET description = ?1 WHERE id = ?2", rusqlite::params![value, id])?;
    let ws = db.query_row(&format!("SELECT {WORKSPACE_COLUMNS} FROM workspaces WHERE id = ?1"), [id], row_to_workspace)?;
    Ok(ws)
}

fn set_parent_impl(conn: &rusqlite::Connection, id: i64, parent_workspace_id: Option<i64>) -> Result<Workspace> {
    if let Some(parent_id) = parent_workspace_id {
        if parent_id == id {
            return Err(AtelierError::new(crate::error::ErrorCode::PathNotAllowed, "A project cannot be its own parent"));
        }
        let (parent_profile_id, own_profile_id): (i64, i64) = (
            conn.query_row("SELECT profile_id FROM workspaces WHERE id = ?1", [parent_id], |r| r.get(0))
                .map_err(|_| AtelierError::not_found("Parent project not found"))?,
            conn.query_row("SELECT profile_id FROM workspaces WHERE id = ?1", [id], |r| r.get(0))
                .map_err(|_| AtelierError::not_found("Workspace not found"))?,
        );
        if parent_profile_id != own_profile_id {
            return Err(AtelierError::new(crate::error::ErrorCode::PathNotAllowed, "Parent project must belong to the same profile"));
        }
        // A sub-project one level deep is the only shape this feature
        // supports today — reject making a project a child of one of its
        // own descendants, which would otherwise create a cycle.
        let mut ancestor = Some(parent_id);
        while let Some(a) = ancestor {
            if a == id {
                return Err(AtelierError::new(crate::error::ErrorCode::PathNotAllowed, "That would create a parent/child cycle"));
            }
            ancestor = conn.query_row("SELECT parent_workspace_id FROM workspaces WHERE id = ?1", [a], |r| r.get(0)).ok().flatten();
        }
    }

    conn.execute("UPDATE workspaces SET parent_workspace_id = ?1 WHERE id = ?2", rusqlite::params![parent_workspace_id, id])?;
    let ws = conn.query_row(&format!("SELECT {WORKSPACE_COLUMNS} FROM workspaces WHERE id = ?1"), [id], row_to_workspace)?;
    Ok(ws)
}

#[tauri::command]
pub fn workspace_close(_id: i64) -> Result<()> {
    // No-op for now — state is just not tracking as "open"
    Ok(())
}

#[tauri::command]
pub fn workspace_rename(id: i64, name: String, db: State<Db>) -> Result<Workspace> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute("UPDATE workspaces SET name = ?1 WHERE id = ?2", rusqlite::params![name, id])?;
    let ws = db.query_row(
        &format!("SELECT {WORKSPACE_COLUMNS} FROM workspaces WHERE id = ?1"),
        [id],
        row_to_workspace,
    )?;
    Ok(ws)
}

#[tauri::command]
pub fn workspace_delete(id: i64, db: State<Db>) -> Result<()> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;

    let ws_path: String = db.query_row(
        "SELECT path FROM workspaces WHERE id = ?1",
        [id],
        |r| r.get(0),
    ).map_err(|_| AtelierError::not_found("Workspace not found"))?;

    // Cascade-delete associated data before removing the workspace record.
    // Foreign-key ON DELETE CASCADE handles most of this, but we explicitly
    // delete from tables that reference via indirect relationships.
    let conv_ids: Vec<i64> = {
        let mut stmt = db.prepare("SELECT id FROM conversations WHERE workspace_id = ?1")?;
        let rows = stmt.query_map([id], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for cid in &conv_ids {
        db.execute("DELETE FROM tool_calls WHERE message_id IN (SELECT id FROM messages WHERE conversation_id = ?1)", [cid])?;
        db.execute("DELETE FROM citations WHERE message_id IN (SELECT id FROM messages WHERE conversation_id = ?1)", [cid])?;
        db.execute("DELETE FROM messages WHERE conversation_id = ?1", [cid])?;
    }
    db.execute("DELETE FROM conversations WHERE workspace_id = ?1", [id])?;
    db.execute("DELETE FROM saved_documents WHERE workspace_id = ?1", [id])?;

    let file_ids: Vec<i64> = {
        let mut stmt = db.prepare("SELECT id FROM files WHERE workspace_id = ?1")?;
        let rows = stmt.query_map([id], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for fid in &file_ids {
        db.execute("DELETE FROM chunk_embeddings WHERE chunk_id IN (SELECT id FROM chunks WHERE file_id = ?1)", [fid])?;
        db.execute("DELETE FROM chunks WHERE file_id = ?1", [fid])?;
    }
    db.execute("DELETE FROM files WHERE workspace_id = ?1", [id])?;

    db.execute("DELETE FROM workspaces WHERE id = ?1", [id])?;

    // Remove filesystem directory (ignore if already missing).
    let dir = Path::new(&ws_path);
    if dir.exists() {
        std::fs::remove_dir_all(dir).ok();
    }

    Ok(())
}

#[tauri::command]
pub async fn workspace_suggest_name(
    app: AppHandle,
    id: i64,
    db: State<'_, Db>,
) -> Result<String> {
    // "Magic rename" is invoked from the project overview, before any
    // conversation is necessarily active — there's no reliable "currently
    // selected" model at that point (the new-chat picker is a mutable,
    // possibly-disconnected global default, not tied to this workspace).
    // Use whatever provider/model this workspace has actually chatted with
    // most recently instead, so the button uses a model the user has
    // already proven works rather than silently failing against one they
    // haven't configured.
    let (ws_path, provider, model, profile_id): (String, String, String, Option<i64>) = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        let ws_path: String = db.query_row("SELECT path FROM workspaces WHERE id = ?1", [id], |r| r.get(0))
            .map_err(|_| AtelierError::not_found("Workspace not found"))?;
        let (provider, model): (String, String) = db.query_row(
            "SELECT m.provider, m.model FROM messages m
             JOIN conversations c ON c.id = m.conversation_id
             WHERE c.workspace_id = ?1 AND m.provider IS NOT NULL AND m.model IS NOT NULL
             ORDER BY m.created_at DESC LIMIT 1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).map_err(|_| AtelierError::internal(
            "Chat with this project at least once before using magic rename, so Atelier knows which model to use."
        ))?;
        let profile_id: Option<i64> = db.query_row("SELECT profile_id FROM workspaces WHERE id = ?1", [id], |r| r.get(0)).ok();
        (ws_path, provider, model, profile_id)
    };

    let dir = Path::new(&ws_path);
    let mut listing = String::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for (i, entry) in entries.flatten().enumerate() {
            if i >= 40 { break; }
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            listing.push_str(&format!("{}{}\n", if is_dir { "[dir] " } else { "" }, name));
        }
    }

    if listing.is_empty() {
        return Err(AtelierError::internal("Empty directory — no files to analyze"));
    }

    let prompt = format!(
        "Based on this project's file listing, suggest a short descriptive name (2-4 words, no quotes, no punctuation). Reply with the name only.\n\nFiles:\n{listing}"
    );

    // The "magic rename" button should work reliably rather than making the
    // user press it again after a transient provider error (rate limiting,
    // a momentary network blip) — retry the same provider/model a few
    // times with backoff before giving up.
    const RETRY_DELAYS_MS: [u64; 3] = [500, 1500, 4000];
    let mut last_err = None;
    let mut result = None;
    for (attempt, delay_ms) in RETRY_DELAYS_MS.iter().enumerate() {
        match llm::stream_chat(&app, -1, &provider, &model, vec![("user".to_string(), prompt.clone())], profile_id).await {
            Ok(r) => { result = Some(r); break; }
            Err(e) => {
                log::warn!(
                    "workspace_suggest_name attempt {} failed for {provider}/{model} (workspace {id}): {} (code: {:?})",
                    attempt + 1, e.message, e.code,
                );
                last_err = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
            }
        }
    }
    let result = match result {
        Some(r) => r,
        None => return Err(last_err.unwrap_or_else(|| AtelierError::internal("Name suggestion failed"))),
    };

    let name = result.content.trim().to_string();
    if name.is_empty() {
        return Err(AtelierError::internal("LLM returned empty response"));
    }
    Ok(name)
}

#[tauri::command]
pub fn workspace_relocate(id: i64, new_path: String, db: State<Db>) -> Result<Workspace> {
    if !Path::new(&new_path).exists() {
        return Err(AtelierError::not_found(format!("Path does not exist: {new_path}")));
    }
    let name = path_to_name(&new_path);
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute(
        "UPDATE workspaces SET path = ?1, name = ?2 WHERE id = ?3",
        rusqlite::params![new_path, name, id],
    )?;
    let ws = db.query_row(
        &format!("SELECT {WORKSPACE_COLUMNS} FROM workspaces WHERE id = ?1"),
        [id],
        row_to_workspace,
    )?;
    Ok(ws)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_two_profiles_and_a_workspace() -> Db {
        let db = crate::db::open_in_memory().unwrap();
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "INSERT INTO profiles (name, dir_name, root_path, created_at, last_active_at) VALUES ('a', 'a', '/tmp/a', 0, 0)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO profiles (name, dir_name, root_path, created_at, last_active_at) VALUES ('b', 'b', '/tmp/b', 0, 0)",
                [],
            ).unwrap();
        }
        db
    }

    fn insert_workspace(conn: &rusqlite::Connection, profile_id: i64, name: &str) -> i64 {
        conn.execute(
            "INSERT INTO workspaces (profile_id, name, path, created_at, last_opened_at) VALUES (?1, ?2, ?3, 0, 0)",
            rusqlite::params![profile_id, name, format!("/tmp/{name}")],
        ).unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn set_parent_and_clear() {
        let db = setup_two_profiles_and_a_workspace();
        let conn = db.lock().unwrap();
        let parent = insert_workspace(&conn, 1, "parent");
        let child = insert_workspace(&conn, 1, "child");

        let ws = set_parent_impl(&conn, child, Some(parent)).unwrap();
        assert_eq!(ws.parent_workspace_id, Some(parent));

        let ws = set_parent_impl(&conn, child, None).unwrap();
        assert_eq!(ws.parent_workspace_id, None);
    }

    #[test]
    fn set_parent_rejects_self_parent() {
        let db = setup_two_profiles_and_a_workspace();
        let conn = db.lock().unwrap();
        let a = insert_workspace(&conn, 1, "a");
        assert!(set_parent_impl(&conn, a, Some(a)).is_err());
    }

    #[test]
    fn set_parent_rejects_cross_profile_parent() {
        let db = setup_two_profiles_and_a_workspace();
        let conn = db.lock().unwrap();
        let a = insert_workspace(&conn, 1, "a");
        let b = insert_workspace(&conn, 2, "b");
        assert!(set_parent_impl(&conn, a, Some(b)).is_err());
    }

    #[test]
    fn set_parent_rejects_cycle() {
        let db = setup_two_profiles_and_a_workspace();
        let conn = db.lock().unwrap();
        let a = insert_workspace(&conn, 1, "a");
        let b = insert_workspace(&conn, 1, "b");
        set_parent_impl(&conn, b, Some(a)).unwrap(); // b's parent is a
        // Making a's parent b would create a cycle (a -> b -> a).
        assert!(set_parent_impl(&conn, a, Some(b)).is_err());
    }
}
