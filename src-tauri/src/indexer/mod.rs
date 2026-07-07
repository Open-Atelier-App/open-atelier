pub mod embedder;

use std::path::{Path, PathBuf};
use std::fs;
use tauri::{AppHandle, Emitter};
use crate::commands::cred_store::store_get;
use crate::db::{Db, now_ms};
use crate::error::Result;
use crate::models::IndexProgress;

const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;
const CHUNK_SIZE: usize = 2000;
const CHUNK_OVERLAP: usize = 400;
const EMBED_BATCH: usize = 100;
const SERVICE_NAME: &str = "com.openatelier.app";

static SKIP_EXTS: &[&str] = &[
    "exe", "dll", "so", "dylib", "bin", "obj", "o", "a", "lib",
    "png", "jpg", "jpeg", "gif", "svg", "ico", "webp", "bmp",
    "mp3", "mp4", "wav", "ogg", "avi", "mov", "mkv",
    "zip", "tar", "gz", "bz2", "xz", "7z", "rar",
    "woff", "woff2", "ttf", "eot",
    "db", "sqlite", "sqlite3",
];

fn should_skip(path: &Path, size: u64) -> Option<&'static str> {
    if size > MAX_FILE_SIZE { return Some("too_large"); }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if SKIP_EXTS.contains(&ext.as_str()) { return Some("binary"); }
    None
}

fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(root) else { return files; };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "node_modules" || name == "target" { continue; }
        if path.is_dir() { files.extend(walk_files(&path)); }
        else { files.push(path); }
    }
    files
}

fn chunk_text(text: &str) -> Vec<(usize, usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    if total == 0 { return vec![]; }
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut guard = 0;
    while start < total {
        let end = (start + CHUNK_SIZE).min(total);
        chunks.push((start, end, chars[start..end].iter().collect()));
        if end >= total { break; }
        start = end.saturating_sub(CHUNK_OVERLAP);
        guard += 1;
        if guard > 10000 { break; }
    }
    chunks
}

fn get_embed_creds() -> Option<(String, String)> {
    // Try OpenAI key first, then Ollama URL. store_get is fallback-aware:
    // it tries the OS keychain first, then the local encrypted store if the
    // OS keychain is unavailable (e.g. no Secret Service in a Linux
    // container/dev VM).
    for (provider, cred_type) in &[("openai", "api_key"), ("openai", "api_key"), ("ollama", "ollama_url")] {
        let name = format!("{SERVICE_NAME}/{provider}/{cred_type}");
        if let Ok(Some((val, _backend))) = store_get(SERVICE_NAME, &name) {
            return Some((provider.to_string(), val));
        }
        // Fall back to legacy credential name (pre-cred_type schema).
        if let Ok(Some((val, _backend))) = store_get(SERVICE_NAME, provider) {
            return Some((provider.to_string(), val));
        }
    }
    None
}

pub async fn run_indexer(app: AppHandle, db: Db, workspace_id: i64, workspace_path: String) {
    let _ = index_workspace(&app, &db, workspace_id, &workspace_path).await;
    if let Ok(db) = db.lock() {
        let _ = db.execute("UPDATE workspaces SET index_status = 'complete' WHERE id = ?1", [workspace_id]);
    }
    let _ = app.emit("index://complete", serde_json::json!({ "workspace_id": workspace_id }));
}

async fn index_workspace(app: &AppHandle, db: &Db, workspace_id: i64, workspace_path: &str) -> Result<()> {
    let root = Path::new(workspace_path);
    let files = walk_files(root);
    let total = files.len() as u64;
    let embed_creds = get_embed_creds(); // Option<(provider, key)>
    let mut pending_embed: Vec<(i64, String)> = Vec::new(); // (chunk_id, content)

    for (i, path) in files.iter().enumerate() {
        let rel_path = path.strip_prefix(root).unwrap_or(path).to_string_lossy().to_string();

        let _ = app.emit("index://progress", IndexProgress {
            workspace_id, done: i as u64, total,
            current_file: Some(rel_path.clone()),
        });

        let meta = match fs::metadata(path) { Ok(m) => m, Err(_) => continue };

        if let Some(reason) = should_skip(path, meta.len()) {
            upsert_file_skip(db, workspace_id, &rel_path, path, &meta, reason).await;
            continue;
        }

        let bytes = match fs::read(path) { Ok(b) => b, Err(_) => continue };
        let hash = blake3::hash(&bytes).to_hex().to_string();

        let existing_hash: Option<String> = db.lock().ok().and_then(|db| {
            db.query_row(
                "SELECT content_hash FROM files WHERE workspace_id=?1 AND rel_path=?2",
                rusqlite::params![workspace_id, rel_path], |r| r.get(0),
            ).ok()
        });
        if existing_hash.as_deref() == Some(&hash) { continue; }

        let text = match String::from_utf8(bytes) {
            Ok(t) => t,
            Err(_) => { upsert_file_skip(db, workspace_id, &rel_path, path, &meta, "binary").await; continue; }
        };

        let file_id: i64 = {
            let Ok(db) = db.lock() else { continue };
            let abs_path = path.to_string_lossy().to_string();
            let ext = path.extension().and_then(|e| e.to_str()).map(|s| s.to_string());
            let now = now_ms();
            let _ = db.execute(
                "INSERT INTO files (workspace_id,rel_path,abs_path,ext,size_bytes,mtime,content_hash,index_state)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,'pending')
                 ON CONFLICT(workspace_id,rel_path) DO UPDATE SET
                 content_hash=excluded.content_hash,size_bytes=excluded.size_bytes,mtime=excluded.mtime,index_state='pending'",
                rusqlite::params![workspace_id, rel_path, abs_path, ext, meta.len() as i64, now, hash],
            );
            match db.query_row("SELECT id FROM files WHERE workspace_id=?1 AND rel_path=?2",
                rusqlite::params![workspace_id, rel_path], |r| r.get(0)) {
                Ok(id) => id, Err(_) => continue,
            }
        };

        { let Ok(db) = db.lock() else { continue }; let _ = db.execute("DELETE FROM chunks WHERE file_id=?1", [file_id]); }

        let chunks = chunk_text(&text);
        for (ci, (char_start, char_end, content)) in chunks.iter().enumerate() {
            let chunk_id: i64 = {
                let Ok(db) = db.lock() else { break };
                let _ = db.execute(
                    "INSERT INTO chunks (file_id,chunk_index,content,char_start,char_end,embed_state) VALUES (?1,?2,?3,?4,?5,'pending')",
                    rusqlite::params![file_id, ci as i64, content, *char_start as i64, *char_end as i64],
                );
                db.last_insert_rowid()
            };
            if embed_creds.is_some() {
                pending_embed.push((chunk_id, content.clone()));
            }
        }

        { let Ok(db) = db.lock() else { continue };
          let _ = db.execute("UPDATE files SET index_state='indexed',indexed_at=?1,content_hash=?2 WHERE id=?3",
              rusqlite::params![now_ms(), hash, file_id]); }

        // Flush embedding batch
        if let Some((ref provider, ref key)) = embed_creds {
            if pending_embed.len() >= EMBED_BATCH {
                flush_embeddings(db, pending_embed.drain(..).collect(), provider, key).await;
            }
        }
    }

    // Final embedding flush
    if let Some((ref provider, ref key)) = embed_creds {
        if !pending_embed.is_empty() {
            flush_embeddings(db, pending_embed, provider, key).await;
        }
    }

    Ok(())
}

async fn flush_embeddings(db: &Db, batch: Vec<(i64, String)>, provider: &str, key: &str) {
    let texts: Vec<&str> = batch.iter().map(|(_, t)| t.as_str()).collect();
    let Ok(embeddings) = embedder::embed_texts(&texts, provider, key).await else { return };

    for ((chunk_id, _), embedding) in batch.iter().zip(embeddings.iter()) {
        // Store embedding as JSON blob (compatible without sqlite-vec extension at read time)
        let json = serde_json::to_string(embedding).unwrap_or_default();
        let Ok(db) = db.lock() else { continue };
        // Try vec_chunks first, fall back to JSON column on chunks
        let _ = db.execute(
            "INSERT OR REPLACE INTO chunk_embeddings (chunk_id, embedding_json) VALUES (?1, ?2)",
            rusqlite::params![chunk_id, json],
        );
        let _ = db.execute(
            "UPDATE chunks SET embed_state='done' WHERE id=?1",
            [chunk_id],
        );
    }
}

async fn upsert_file_skip(db: &Db, workspace_id: i64, rel_path: &str, path: &Path, meta: &fs::Metadata, reason: &str) {
    let Ok(db) = db.lock() else { return };
    let abs_path = path.to_string_lossy().to_string();
    let ext = path.extension().and_then(|e| e.to_str()).map(|s| s.to_string());
    let _ = db.execute(
        "INSERT INTO files (workspace_id,rel_path,abs_path,ext,size_bytes,mtime,index_state,skip_reason)
         VALUES (?1,?2,?3,?4,?5,?6,'skipped',?7)
         ON CONFLICT(workspace_id,rel_path) DO UPDATE SET index_state='skipped',skip_reason=excluded.skip_reason",
        rusqlite::params![workspace_id, rel_path, abs_path, ext, meta.len() as i64, now_ms(), reason],
    );
}
