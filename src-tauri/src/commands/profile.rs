use std::fs;
use tauri::State;
use crate::db::{Db, now_ms};
use crate::error::{AtelierError, Result};
use crate::models::Profile;

fn row_to_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
    Ok(Profile {
        id: row.get(0)?,
        name: row.get(1)?,
        dir_name: row.get(2)?,
        root_path: row.get(3)?,
        created_at: row.get(4)?,
        last_active_at: row.get(5)?,
        is_active: row.get::<_, i64>(6)? != 0,
    })
}

#[tauri::command]
pub fn profile_list(db: State<Db>) -> Result<Vec<Profile>> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let mut stmt = db.prepare(
        "SELECT id, name, dir_name, root_path, created_at, last_active_at, is_active FROM profiles ORDER BY last_active_at DESC"
    )?;
    let rows = stmt.query_map([], row_to_profile)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[tauri::command]
pub fn profile_create(name: String, dir_name: String, root_path: String, db: State<Db>) -> Result<Profile> {
    // Create directory on disk
    fs::create_dir_all(&root_path)
        .map_err(|e| AtelierError::io(format!("Cannot create directory {root_path}: {e}")))?;

    let now = now_ms();
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute(
        "INSERT INTO profiles (name, dir_name, root_path, created_at, last_active_at, is_active) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        rusqlite::params![name, dir_name, root_path, now, now],
    )?;
    let id = db.last_insert_rowid();
    let profile = db.query_row(
        "SELECT id, name, dir_name, root_path, created_at, last_active_at, is_active FROM profiles WHERE id = ?1",
        [id],
        row_to_profile,
    )?;
    Ok(profile)
}

#[tauri::command]
pub fn profile_update(
    id: i64,
    name: Option<String>,
    dir_name: Option<String>,
    root_path: Option<String>,
    db: State<Db>,
) -> Result<Profile> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    if let Some(n) = name {
        db.execute("UPDATE profiles SET name = ?1 WHERE id = ?2", rusqlite::params![n, id])?;
    }
    if let Some(d) = dir_name {
        db.execute("UPDATE profiles SET dir_name = ?1 WHERE id = ?2", rusqlite::params![d, id])?;
    }
    if let Some(r) = root_path {
        db.execute("UPDATE profiles SET root_path = ?1 WHERE id = ?2", rusqlite::params![r, id])?;
    }
    let profile = db.query_row(
        "SELECT id, name, dir_name, root_path, created_at, last_active_at, is_active FROM profiles WHERE id = ?1",
        [id],
        row_to_profile,
    )?;
    Ok(profile)
}

#[tauri::command]
pub fn profile_delete(id: i64, db: State<Db>) -> Result<()> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;

    // Prevent deleting the last profile.
    let count: i64 = db.query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))?;
    if count <= 1 {
        return Err(AtelierError::new(
            crate::error::ErrorCode::Internal,
            "Cannot delete the only remaining profile",
        ));
    }

    let profile_root: String = db.query_row(
        "SELECT root_path FROM profiles WHERE id = ?1",
        [id],
        |r| r.get(0),
    ).map_err(|_| AtelierError::not_found("Profile not found"))?;

    // Cascade-delete workspace data.
    let ws_ids: Vec<i64> = {
        let mut stmt = db.prepare("SELECT id FROM workspaces WHERE profile_id = ?1")?;
        let rows = stmt.query_map([id], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for wid in &ws_ids {
        let conv_ids: Vec<i64> = {
            let mut stmt = db.prepare("SELECT id FROM conversations WHERE workspace_id = ?1")?;
            let rows = stmt.query_map([wid], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for cid in &conv_ids {
            db.execute("DELETE FROM tool_calls WHERE message_id IN (SELECT id FROM messages WHERE conversation_id = ?1)", [cid])?;
            db.execute("DELETE FROM citations WHERE message_id IN (SELECT id FROM messages WHERE conversation_id = ?1)", [cid])?;
            db.execute("DELETE FROM messages WHERE conversation_id = ?1", [cid])?;
        }
        db.execute("DELETE FROM conversations WHERE workspace_id = ?1", [wid])?;
        db.execute("DELETE FROM saved_documents WHERE workspace_id = ?1", [wid])?;
        let file_ids: Vec<i64> = {
            let mut stmt = db.prepare("SELECT id FROM files WHERE workspace_id = ?1")?;
            let rows = stmt.query_map([wid], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for fid in &file_ids {
            db.execute("DELETE FROM chunk_embeddings WHERE chunk_id IN (SELECT id FROM chunks WHERE file_id = ?1)", [fid])?;
            db.execute("DELETE FROM chunks WHERE file_id = ?1", [fid])?;
        }
        db.execute("DELETE FROM files WHERE workspace_id = ?1", [wid])?;
    }
    db.execute("DELETE FROM workspaces WHERE profile_id = ?1", [id])?;
    db.execute("DELETE FROM window_state WHERE profile_id = ?1", [id])?;
    db.execute("DELETE FROM profiles WHERE id = ?1", [id])?;

    // Remove the profile directory.
    let dir = std::path::Path::new(&profile_root);
    if dir.exists() {
        fs::remove_dir_all(dir).ok();
    }

    // Delete per-profile credentials if they exist.
    if let Some(data_dir) = dirs::data_dir() {
        let cred_dir = data_dir.join("com.openatelier.app").join("credentials").join(id.to_string());
        if cred_dir.exists() {
            fs::remove_dir_all(cred_dir).ok();
        }
    }

    Ok(())
}

#[tauri::command]
pub fn profile_switch(id: i64, db: State<Db>) -> Result<Profile> {
    let now = now_ms();
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute("UPDATE profiles SET is_active = 0", [])?;
    db.execute("UPDATE profiles SET is_active = 1, last_active_at = ?1 WHERE id = ?2", rusqlite::params![now, id])?;
    let profile = db.query_row(
        "SELECT id, name, dir_name, root_path, created_at, last_active_at, is_active FROM profiles WHERE id = ?1",
        [id],
        row_to_profile,
    ).map_err(|_| AtelierError::not_found("Profile not found"))?;
    Ok(profile)
}

#[tauri::command]
pub fn profile_recreate_dir(id: i64, db: State<Db>) -> Result<Profile> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let profile = db.query_row(
        "SELECT id, name, dir_name, root_path, created_at, last_active_at, is_active FROM profiles WHERE id = ?1",
        [id],
        row_to_profile,
    ).map_err(|_| AtelierError::not_found("Profile not found"))?;
    fs::create_dir_all(&profile.root_path)
        .map_err(|e| AtelierError::io(format!("Cannot create directory {}: {e}", profile.root_path)))?;
    Ok(profile)
}

#[tauri::command]
pub fn profile_get_active(db: State<Db>) -> Result<Option<Profile>> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let result = db.query_row(
        "SELECT id, name, dir_name, root_path, created_at, last_active_at, is_active FROM profiles WHERE is_active = 1 LIMIT 1",
        [],
        row_to_profile,
    );
    match result {
        Ok(p) => Ok(Some(p)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
