use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;
use crate::db::{Db, now_ms};
use crate::error::{AtelierError, ErrorCode, Result};
use crate::models::{WorkspaceFile, FileNode};

const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB
const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024; // 10 MB
const ALLOWED_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

fn get_workspace_path(db: &rusqlite::Connection, workspace_id: i64) -> Result<String> {
    db.query_row(
        "SELECT path FROM workspaces WHERE id = ?1",
        [workspace_id],
        |r| r.get(0),
    ).map_err(|_| AtelierError::not_found("Workspace not found"))
}

fn validate_path(workspace_root: &str, rel_path: &str) -> Result<PathBuf> {
    let root = PathBuf::from(workspace_root).canonicalize()
        .map_err(|e| AtelierError::io(e.to_string()))?;
    let target = root.join(rel_path);
    // Canonicalize parent to handle new files that don't exist yet
    let parent = target.parent().unwrap_or(&target);
    let canonical_parent = if parent.exists() {
        parent.canonicalize().map_err(|e| AtelierError::io(e.to_string()))?
    } else {
        parent.to_path_buf()
    };
    if !canonical_parent.starts_with(&root) {
        return Err(AtelierError::path_not_allowed(rel_path));
    }
    Ok(target)
}

fn build_tree(root: &Path, base: &Path) -> Vec<FileNode> {
    let mut nodes = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return nodes,
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| {
        let is_file = e.file_type().map(|t| t.is_file()).unwrap_or(false);
        (is_file as u8, e.file_name())
    });
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') { continue; }
        let path = entry.path();
        let rel_path = path.strip_prefix(base).unwrap_or(&path)
            .to_string_lossy().to_string();
        let is_dir = path.is_dir();
        let meta = fs::metadata(&path).ok();
        let size_bytes = meta.as_ref().and_then(|m| if !is_dir { Some(m.len() as i64) } else { None });
        let ext = if !is_dir { path.extension().and_then(|e| e.to_str()).map(|s| s.to_string()) } else { None };
        let children = if is_dir { Some(build_tree(&path, base)) } else { None };
        nodes.push(FileNode { name, rel_path, is_dir, children, size_bytes, ext });
    }
    nodes
}

#[tauri::command]
pub fn file_list_tree(workspace_id: i64, db: State<Db>) -> Result<Vec<FileNode>> {
    let ws_path = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        get_workspace_path(&db, workspace_id)?
    };
    let root = Path::new(&ws_path);
    Ok(build_tree(root, root))
}

#[tauri::command]
pub fn file_create(workspace_id: i64, rel_path: String, content: String, db: State<Db>) -> Result<WorkspaceFile> {
    let ws_path = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        get_workspace_path(&db, workspace_id)?
    };
    let target = validate_path(&ws_path, &rel_path)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(AtelierError::from)?;
    }
    fs::write(&target, &content).map_err(AtelierError::from)?;

    let meta = fs::metadata(&target).map_err(AtelierError::from)?;
    let now = now_ms();
    let abs_path = target.to_string_lossy().to_string();
    let ext = target.extension().and_then(|e| e.to_str()).map(|s| s.to_string());

    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute(
        "INSERT INTO files (workspace_id, rel_path, abs_path, ext, size_bytes, mtime, index_state)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')
         ON CONFLICT(workspace_id, rel_path) DO UPDATE SET size_bytes=excluded.size_bytes, mtime=excluded.mtime, index_state='pending'",
        rusqlite::params![workspace_id, rel_path, abs_path, ext, meta.len() as i64, now],
    )?;
    let id = db.last_insert_rowid();
    Ok(WorkspaceFile {
        id, workspace_id,
        rel_path, abs_path,
        ext, size_bytes: meta.len() as i64,
        mtime: now, content_hash: None,
        index_state: "pending".into(), skip_reason: None, indexed_at: None,
    })
}

/// Saves a base64-encoded image (dropped, pasted, or picked in the chat
/// composer) into the workspace's `attachments/` folder as a real project
/// file, reusing the same write path as `file_create` — it shows up in the
/// file tree and can be referenced (e.g. via a markdown image link, which
/// the message composer does) like anything else, rather than living in
/// some separate, invisible blob store.
#[tauri::command]
pub fn image_attachment_save(workspace_id: i64, filename: String, base64_data: String, db: State<Db>) -> Result<WorkspaceFile> {
    let ext = Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    if !ALLOWED_IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return Err(AtelierError::new(ErrorCode::Unsupported, format!("Unsupported image type: .{ext}")));
    }

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(base64_data.as_bytes())
        .map_err(|e| AtelierError::new(ErrorCode::Unsupported, format!("Invalid image data: {e}")))?;
    if bytes.len() > MAX_IMAGE_SIZE {
        return Err(AtelierError::new(ErrorCode::FileTooLarge, format!("Image is {} MB, max is {} MB", bytes.len() / 1_000_000, MAX_IMAGE_SIZE / 1_000_000)));
    }

    let ws_path = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        get_workspace_path(&db, workspace_id)?
    };

    // Unique, collision-proof name — pasted/dropped images rarely have a
    // meaningful filename of their own (e.g. "image.png" from a clipboard
    // paste), so timestamp-prefixing avoids silently overwriting a
    // previous attachment that happened to get the same generic name.
    let safe_name = Path::new(&filename).file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("image.{ext}"));
    let rel_path = format!("attachments/{}-{safe_name}", now_ms());

    let target = validate_path(&ws_path, &rel_path)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(AtelierError::from)?;
    }
    fs::write(&target, &bytes).map_err(AtelierError::from)?;

    let meta = fs::metadata(&target).map_err(AtelierError::from)?;
    let now = now_ms();
    let abs_path = target.to_string_lossy().to_string();

    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute(
        "INSERT INTO files (workspace_id, rel_path, abs_path, ext, size_bytes, mtime, index_state)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')
         ON CONFLICT(workspace_id, rel_path) DO UPDATE SET size_bytes=excluded.size_bytes, mtime=excluded.mtime, index_state='pending'",
        rusqlite::params![workspace_id, rel_path, abs_path, ext, meta.len() as i64, now],
    )?;
    let id = db.last_insert_rowid();
    Ok(WorkspaceFile {
        id, workspace_id,
        rel_path, abs_path,
        ext: Some(ext), size_bytes: meta.len() as i64,
        mtime: now, content_hash: None,
        index_state: "pending".into(), skip_reason: None, indexed_at: None,
    })
}

#[tauri::command]
pub fn file_rename(workspace_id: i64, old_rel_path: String, new_rel_path: String, db: State<Db>) -> Result<WorkspaceFile> {
    let ws_path = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        get_workspace_path(&db, workspace_id)?
    };
    let old_target = validate_path(&ws_path, &old_rel_path)?;
    let new_target = validate_path(&ws_path, &new_rel_path)?;

    if new_target.exists() {
        return Err(AtelierError::io(format!("{new_rel_path} already exists")));
    }
    fs::rename(&old_target, &new_target).map_err(AtelierError::from)?;

    let meta = fs::metadata(&new_target).map_err(AtelierError::from)?;
    let now = now_ms();
    let abs_path = new_target.to_string_lossy().to_string();
    let ext = new_target.extension().and_then(|e| e.to_str()).map(|s| s.to_string());

    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute(
        "UPDATE files SET rel_path=?1, abs_path=?2, ext=?3, mtime=?4 WHERE workspace_id=?5 AND rel_path=?6",
        rusqlite::params![new_rel_path, abs_path, ext, now, workspace_id, old_rel_path],
    )?;
    Ok(WorkspaceFile {
        id: 0, workspace_id,
        rel_path: new_rel_path, abs_path,
        ext, size_bytes: meta.len() as i64,
        mtime: now, content_hash: None,
        index_state: "pending".into(), skip_reason: None, indexed_at: None,
    })
}

#[tauri::command]
pub fn file_delete(workspace_id: i64, rel_path: String, db: State<Db>) -> Result<()> {
    let ws_path = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        get_workspace_path(&db, workspace_id)?
    };
    let target = validate_path(&ws_path, &rel_path)?;
    if target.is_dir() {
        fs::remove_dir_all(&target).map_err(AtelierError::from)?;
    } else {
        fs::remove_file(&target).map_err(AtelierError::from)?;
    }
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute(
        "DELETE FROM files WHERE workspace_id=?1 AND rel_path=?2",
        rusqlite::params![workspace_id, rel_path],
    )?;
    Ok(())
}

#[tauri::command]
pub fn file_write(workspace_id: i64, rel_path: String, content: String, db: State<Db>) -> Result<WorkspaceFile> {
    let ws_path = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        get_workspace_path(&db, workspace_id)?
    };
    let target = validate_path(&ws_path, &rel_path)?;
    if !target.exists() {
        return Err(AtelierError::not_found(format!("File not found: {rel_path}")));
    }
    fs::write(&target, &content).map_err(AtelierError::from)?;

    let meta = fs::metadata(&target).map_err(AtelierError::from)?;
    let now = now_ms();
    let abs_path = target.to_string_lossy().to_string();
    let ext = target.extension().and_then(|e| e.to_str()).map(|s| s.to_string());

    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute(
        "UPDATE files SET size_bytes=?1, mtime=?2, index_state='pending' WHERE workspace_id=?3 AND rel_path=?4",
        rusqlite::params![meta.len() as i64, now, workspace_id, rel_path],
    )?;
    let id = db.query_row(
        "SELECT id FROM files WHERE workspace_id=?1 AND rel_path=?2",
        rusqlite::params![workspace_id, rel_path],
        |r| r.get(0),
    ).unwrap_or(0);
    Ok(WorkspaceFile {
        id, workspace_id,
        rel_path, abs_path,
        ext, size_bytes: meta.len() as i64,
        mtime: now, content_hash: None,
        index_state: "pending".into(), skip_reason: None, indexed_at: None,
    })
}

#[tauri::command]
pub fn file_read_raw(workspace_id: i64, rel_path: String, db: State<Db>) -> Result<String> {
    let ws_path = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        get_workspace_path(&db, workspace_id)?
    };
    let target = validate_path(&ws_path, &rel_path)?;
    let meta = fs::metadata(&target).map_err(AtelierError::from)?;
    if meta.len() > MAX_FILE_SIZE {
        return Err(AtelierError::new(crate::error::ErrorCode::FileTooLarge, "File too large to read"));
    }
    let content = fs::read_to_string(&target)
        .map_err(|e| AtelierError::io(format!("Cannot read {rel_path}: {e}")))?;
    Ok(content)
}

#[tauri::command]
pub fn file_read_office_preview(
    workspace_id: i64,
    rel_path: String,
    db: State<Db>,
) -> Result<crate::triggers::office::OfficePreview> {
    let ws_path = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        get_workspace_path(&db, workspace_id)?
    };
    let target = validate_path(&ws_path, &rel_path)?;
    let meta = fs::metadata(&target).map_err(AtelierError::from)?;
    if meta.len() > MAX_FILE_SIZE {
        return Err(AtelierError::new(crate::error::ErrorCode::FileTooLarge, "File too large to read"));
    }
    let bytes = fs::read(&target)
        .map_err(|e| AtelierError::io(format!("Cannot read {rel_path}: {e}")))?;
    crate::triggers::office::read_office_preview(&rel_path, &bytes)
        .map_err(|e| AtelierError::new(crate::error::ErrorCode::Unsupported, e))
}

/// Direct UI-triggered counterpart to the EXPORT_PDF trigger (see
/// triggers::pdf::html_to_pdf) — the "Export to PDF" button in the file
/// viewer for an HTML file, bypassing the LLM/trigger protocol entirely
/// since this is the user acting on a file they're already looking at, not
/// something the model needs to decide to do. Writes a sibling file with
/// the same name and a .pdf extension, overwriting it if present, and
/// returns its relative path so the frontend can jump straight to it.
#[tauri::command]
pub fn file_export_pdf(workspace_id: i64, rel_path: String, db: State<Db>) -> Result<String> {
    let ws_path = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        get_workspace_path(&db, workspace_id)?
    };
    let target = validate_path(&ws_path, &rel_path)?;
    let html = fs::read_to_string(&target)
        .map_err(|e| AtelierError::io(format!("Cannot read {rel_path}: {e}")))?;

    let bytes = crate::triggers::pdf::html_to_pdf(&html)
        .map_err(|e| AtelierError::new(crate::error::ErrorCode::Unsupported, e))?;

    let pdf_rel_path = Path::new(&rel_path).with_extension("pdf")
        .to_string_lossy().to_string();
    let pdf_target = validate_path(&ws_path, &pdf_rel_path)?;
    fs::write(&pdf_target, &bytes)
        .map_err(|e| AtelierError::io(format!("Cannot write {pdf_rel_path}: {e}")))?;

    Ok(pdf_rel_path)
}

#[tauri::command]
pub fn index_start(workspace_id: i64, db: State<Db>, app: tauri::AppHandle) -> Result<()> {
    let ws_path = {
        let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
        db.execute(
            "UPDATE workspaces SET index_status = 'indexing' WHERE id = ?1",
            [workspace_id],
        )?;
        get_workspace_path(&db, workspace_id)?
    };

    // Spawn indexer in background
    let db_clone = db.inner().clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::indexer::run_indexer(app_clone, db_clone, workspace_id, ws_path).await;
    });

    Ok(())
}

#[tauri::command]
pub fn index_cancel(workspace_id: i64, db: State<Db>) -> Result<()> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    db.execute(
        "UPDATE workspaces SET index_status = 'idle' WHERE id = ?1",
        [workspace_id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn index_status(workspace_id: i64, db: State<Db>) -> Result<serde_json::Value> {
    let db = db.lock().map_err(|_| AtelierError::internal("lock"))?;
    let status: String = db.query_row(
        "SELECT index_status FROM workspaces WHERE id = ?1",
        [workspace_id],
        |r| r.get(0),
    ).map_err(|_| AtelierError::not_found("Workspace not found"))?;
    let done: i64 = db.query_row(
        "SELECT COUNT(*) FROM files WHERE workspace_id = ?1 AND index_state = 'indexed'",
        [workspace_id],
        |r| r.get(0),
    ).unwrap_or(0);
    let total: i64 = db.query_row(
        "SELECT COUNT(*) FROM files WHERE workspace_id = ?1",
        [workspace_id],
        |r| r.get(0),
    ).unwrap_or(0);
    Ok(serde_json::json!({ "done": done, "total": total, "status": status }))
}
