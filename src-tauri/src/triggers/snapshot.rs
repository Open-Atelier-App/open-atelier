use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use serde::Serialize;
use crate::error::{AtelierError, Result};

const MAX_SNAPSHOTS_PER_CONVERSATION: usize = 50;
const MAX_SNAPSHOT_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub conversation_id: i64,
    pub action: String,
    pub file_path: String,
    pub previous_content: Option<Vec<u8>>,
    pub previous_path: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
pub struct UndoResult {
    pub action: String,
    pub file_path: String,
    pub restored: bool,
    pub detail: String,
}

pub struct SnapshotStore {
    inner: Mutex<HashMap<i64, VecDeque<FileSnapshot>>>,
}

impl SnapshotStore {
    pub fn new() -> Self {
        Self { inner: Mutex::new(HashMap::new()) }
    }

    pub fn capture(
        &self,
        conversation_id: i64,
        action: &str,
        file_path: &str,
    ) -> Result<()> {
        let previous_content = if std::path::Path::new(file_path).exists() {
            let meta = std::fs::metadata(file_path)
                .map_err(|e| AtelierError::io(e.to_string()))?;

            if meta.len() > MAX_SNAPSHOT_FILE_SIZE {
                log::warn!("Skipping snapshot for {file_path}: file too large ({} bytes)", meta.len());
                None
            } else {
                Some(std::fs::read(file_path).map_err(|e| AtelierError::io(e.to_string()))?)
            }
        } else {
            None
        };

        let snapshot = FileSnapshot {
            conversation_id,
            action: action.to_string(),
            file_path: file_path.to_string(),
            previous_content,
            previous_path: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let mut store = self.inner.lock().map_err(|_| AtelierError::internal("snapshot lock"))?;
        let queue = store.entry(conversation_id).or_insert_with(VecDeque::new);

        if queue.len() >= MAX_SNAPSHOTS_PER_CONVERSATION {
            queue.pop_front();
        }
        queue.push_back(snapshot);

        Ok(())
    }

    pub fn capture_rename(
        &self,
        conversation_id: i64,
        old_path: &str,
        new_path: &str,
    ) -> Result<()> {
        let previous_content = if std::path::Path::new(old_path).exists() {
            let meta = std::fs::metadata(old_path)
                .map_err(|e| AtelierError::io(e.to_string()))?;
            if meta.len() > MAX_SNAPSHOT_FILE_SIZE {
                None
            } else {
                Some(std::fs::read(old_path).map_err(|e| AtelierError::io(e.to_string()))?)
            }
        } else {
            None
        };

        let snapshot = FileSnapshot {
            conversation_id,
            action: "RENAME".to_string(),
            file_path: new_path.to_string(),
            previous_content,
            previous_path: Some(old_path.to_string()),
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let mut store = self.inner.lock().map_err(|_| AtelierError::internal("snapshot lock"))?;
        let queue = store.entry(conversation_id).or_insert_with(VecDeque::new);

        if queue.len() >= MAX_SNAPSHOTS_PER_CONVERSATION {
            queue.pop_front();
        }
        queue.push_back(snapshot);

        Ok(())
    }

    pub fn undo_last(&self, conversation_id: i64) -> Result<UndoResult> {
        let mut store = self.inner.lock().map_err(|_| AtelierError::internal("snapshot lock"))?;

        let queue = store.get_mut(&conversation_id)
            .ok_or_else(|| AtelierError::internal("No snapshots for this conversation"))?;

        let snapshot = queue.pop_back()
            .ok_or_else(|| AtelierError::internal("No snapshots to undo"))?;

        match snapshot.action.as_str() {
            "RENAME" => {
                if let Some(ref old_path) = snapshot.previous_path {
                    std::fs::rename(&snapshot.file_path, old_path)
                        .map_err(|e| AtelierError::io(e.to_string()))?;
                    Ok(UndoResult {
                        action: "RENAME".into(),
                        file_path: snapshot.file_path.clone(),
                        restored: true,
                        detail: format!("Moved back to {old_path}"),
                    })
                } else {
                    Ok(UndoResult {
                        action: "RENAME".into(),
                        file_path: snapshot.file_path.clone(),
                        restored: false,
                        detail: "No previous path recorded".into(),
                    })
                }
            }
            "CREATE" => {
                // Undo create = delete the file
                if std::path::Path::new(&snapshot.file_path).exists() {
                    std::fs::remove_file(&snapshot.file_path)
                        .map_err(|e| AtelierError::io(e.to_string()))?;
                }
                Ok(UndoResult {
                    action: "CREATE".into(),
                    file_path: snapshot.file_path,
                    restored: true,
                    detail: "File removed".into(),
                })
            }
            _ => {
                // WRITE, DELETE, INSERT, APPEND: restore previous content
                match snapshot.previous_content {
                    Some(content) => {
                        if let Some(parent) = std::path::Path::new(&snapshot.file_path).parent() {
                            std::fs::create_dir_all(parent).ok();
                        }
                        std::fs::write(&snapshot.file_path, &content)
                            .map_err(|e| AtelierError::io(e.to_string()))?;
                        Ok(UndoResult {
                            action: snapshot.action,
                            file_path: snapshot.file_path,
                            restored: true,
                            detail: "Previous content restored".into(),
                        })
                    }
                    None => {
                        // File didn't exist before — delete it
                        if std::path::Path::new(&snapshot.file_path).exists() {
                            std::fs::remove_file(&snapshot.file_path)
                                .map_err(|e| AtelierError::io(e.to_string()))?;
                        }
                        Ok(UndoResult {
                            action: snapshot.action,
                            file_path: snapshot.file_path,
                            restored: true,
                            detail: "File removed (did not exist before)".into(),
                        })
                    }
                }
            }
        }
    }

    pub fn clear(&self, conversation_id: i64) {
        if let Ok(mut store) = self.inner.lock() {
            store.remove(&conversation_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("atelier_snapshot_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn capture_and_undo_write() {
        let dir = temp_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "original").unwrap();

        let store = SnapshotStore::new();
        store.capture(1, "WRITE", file.to_str().unwrap()).unwrap();

        // Simulate the write
        fs::write(&file, "modified").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "modified");

        // Undo
        let result = store.undo_last(1).unwrap();
        assert!(result.restored);
        assert_eq!(fs::read_to_string(&file).unwrap(), "original");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn capture_and_undo_delete() {
        let dir = temp_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "content").unwrap();

        let store = SnapshotStore::new();
        store.capture(1, "DELETE", file.to_str().unwrap()).unwrap();

        // Simulate delete
        fs::remove_file(&file).unwrap();
        assert!(!file.exists());

        // Undo
        let result = store.undo_last(1).unwrap();
        assert!(result.restored);
        assert!(file.exists());
        assert_eq!(fs::read_to_string(&file).unwrap(), "content");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn undo_empty_returns_error() {
        let store = SnapshotStore::new();
        assert!(store.undo_last(999).is_err());
    }

    #[test]
    fn rolling_buffer_eviction() {
        let dir = temp_dir();
        let file = dir.join("test.txt");
        fs::write(&file, "data").unwrap();

        let store = SnapshotStore::new();
        for _ in 0..60 {
            store.capture(1, "WRITE", file.to_str().unwrap()).unwrap();
        }

        let inner = store.inner.lock().unwrap();
        assert_eq!(inner[&1].len(), 50);

        fs::remove_dir_all(&dir).ok();
    }
}
