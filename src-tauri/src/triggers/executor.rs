use std::fs;
use std::path::{Path, PathBuf};
use serde::Serialize;
use crate::llm::permissions;
use super::parser::ParsedTrigger;
use super::snapshot::SnapshotStore;

const BINARY_CHECK_SIZE: usize = 8192;

#[derive(Debug, Clone, Serialize)]
pub struct TriggerResult {
    pub action: String,
    pub status: String, // "OK" or "FAIL"
    pub detail: String,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentResponse {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionResult {
    pub results: Vec<TriggerResult>,
    pub content_responses: Vec<ContentResponse>,
}

fn validate_path(workspace_root: &str, rel_path: &str) -> std::result::Result<PathBuf, String> {
    if rel_path.is_empty() {
        return Err("Empty file path".into());
    }

    let root = match Path::new(workspace_root).canonicalize() {
        Ok(r) => r,
        Err(e) => return Err(format!("Cannot resolve workspace root: {e}")),
    };

    let target = root.join(rel_path);

    let check_path = if target.exists() {
        match target.canonicalize() {
            Ok(p) => p,
            Err(e) => return Err(format!("Cannot resolve path: {e}")),
        }
    } else {
        let parent = target.parent().unwrap_or(&target);
        if parent.exists() {
            let canonical_parent = parent.canonicalize()
                .map_err(|e| format!("Cannot resolve parent: {e}"))?;
            if !canonical_parent.starts_with(&root) {
                return Err(format!("Path outside workspace boundary: {rel_path}"));
            }
            target.clone()
        } else {
            target.clone()
        }
    };

    if check_path != target && !check_path.starts_with(&root) {
        return Err(format!("Path outside workspace boundary: {rel_path}"));
    }

    Ok(target)
}

fn is_binary(path: &Path) -> bool {
    match fs::read(path) {
        Ok(bytes) => {
            let check_len = bytes.len().min(BINARY_CHECK_SIZE);
            bytes[..check_len].contains(&0)
        }
        Err(_) => false,
    }
}

pub fn execute_batch(
    triggers: &[ParsedTrigger],
    workspace_path: &str,
    permission_level: &str,
    conversation_id: i64,
    snapshots: &SnapshotStore,
) -> ExecutionResult {
    let allowed = permissions::allowed_triggers_for_level(permission_level)
        .unwrap_or_else(|_| vec!["MESSAGE".into()]);

    let mut results = Vec::new();
    let mut content_responses = Vec::new();

    for trigger in triggers {
        if trigger.action == "MESSAGE" {
            results.push(TriggerResult {
                action: "MESSAGE".into(),
                status: "OK".into(),
                detail: String::new(),
                file_path: None,
            });
            continue;
        }

        if !allowed.contains(&trigger.action) {
            let level_label = permissions::level_label(permission_level).unwrap_or_default();
            results.push(TriggerResult {
                action: trigger.action.clone(),
                status: "FAIL".into(),
                detail: format!("Permission denied: {level_label} cannot {}", trigger.action),
                file_path: trigger.params.first().cloned(),
            });
            continue;
        }

        let result = execute_single(trigger, workspace_path, conversation_id, snapshots, &mut content_responses);
        results.push(result);
    }

    ExecutionResult { results, content_responses }
}

fn execute_single(
    trigger: &ParsedTrigger,
    workspace_path: &str,
    conversation_id: i64,
    snapshots: &SnapshotStore,
    content_responses: &mut Vec<ContentResponse>,
) -> TriggerResult {
    match trigger.action.as_str() {
        "CREATE" => exec_create(trigger, workspace_path, conversation_id, snapshots),
        "DELETE" => exec_delete(trigger, workspace_path, conversation_id, snapshots),
        "WRITE" => exec_write(trigger, workspace_path, conversation_id, snapshots),
        "INSERT" => exec_insert(trigger, workspace_path, conversation_id, snapshots),
        "APPEND" => exec_append(trigger, workspace_path, conversation_id, snapshots),
        "PREVIEW" => exec_preview(trigger, workspace_path),
        "READ" => exec_read(trigger, workspace_path, content_responses),
        "RENAME" => exec_rename(trigger, workspace_path, conversation_id, snapshots),
        "LIST" => exec_list(trigger, workspace_path, content_responses),
        "CREATE_DOCX" => exec_create_office(trigger, workspace_path, conversation_id, snapshots, super::office::build_docx),
        "CREATE_XLSX" => exec_create_office(trigger, workspace_path, conversation_id, snapshots, super::office::build_xlsx),
        "CREATE_PPTX" => exec_create_office(trigger, workspace_path, conversation_id, snapshots, super::office::build_pptx),
        "CREATE_MP3" => exec_create_office(trigger, workspace_path, conversation_id, snapshots, super::tts::text_to_mp3),
        "EXPORT_PDF" => exec_export_pdf(trigger, workspace_path, conversation_id, snapshots),
        _ => TriggerResult {
            action: trigger.action.clone(),
            status: "FAIL".into(),
            detail: format!("Unknown action: {}", trigger.action),
            file_path: None,
        },
    }
}

fn exec_create(t: &ParsedTrigger, ws: &str, conv_id: i64, snaps: &SnapshotStore) -> TriggerResult {
    let rel_path = match t.params.first() {
        Some(p) => p,
        None => return fail(&t.action, "Missing file path parameter"),
    };

    let target = match validate_path(ws, rel_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, rel_path),
    };

    if target.exists() {
        return warn_with_path(&t.action, &format!("File already exists: {rel_path} (use WRITE or APPEND to modify it)"), rel_path);
    }

    if let Some(parent) = target.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return fail_with_path(&t.action, &format!("Cannot create directory: {e}"), rel_path);
        }
    }

    // Snapshot: file doesn't exist yet, so capture records None
    let _ = snaps.capture(conv_id, "CREATE", target.to_str().unwrap_or(""));

    if let Err(e) = fs::write(&target, "") {
        return fail_with_path(&t.action, &format!("Cannot create file: {e}"), rel_path);
    }

    ok_with_detail(&t.action, rel_path, "Created empty file".to_string())
}

/// Shared by CREATE_DOCX/CREATE_XLSX/CREATE_PPTX: validate the path, build
/// the binary document from the trigger's content parameter (via one of
/// office::build_docx/build_xlsx/build_pptx), and write it out. Overwrites
/// an existing file, unlike CREATE — these formats are generated as one
/// complete document rather than built up incrementally, so regenerating
/// in place is the more useful default.
fn exec_create_office(
    t: &ParsedTrigger,
    ws: &str,
    conv_id: i64,
    snaps: &SnapshotStore,
    build: impl Fn(&str) -> std::result::Result<Vec<u8>, String>,
) -> TriggerResult {
    let rel_path = match t.params.first() {
        Some(p) => p,
        None => return fail(&t.action, "Missing file path parameter"),
    };

    let content = match t.params.get(1) {
        Some(c) => c,
        None => return fail(&t.action, "Missing content parameter"),
    };

    let target = match validate_path(ws, rel_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, rel_path),
    };

    if let Some(parent) = target.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return fail_with_path(&t.action, &format!("Cannot create directory: {e}"), rel_path);
        }
    }

    let bytes = match build(content) {
        Ok(b) => b,
        Err(e) => return fail_with_path(&t.action, &e, rel_path),
    };

    let _ = snaps.capture(conv_id, &t.action, target.to_str().unwrap_or(""));

    if let Err(e) = fs::write(&target, &bytes) {
        return fail_with_path(&t.action, &format!("Cannot write file: {e}"), rel_path);
    }

    ok_with_detail(&t.action, rel_path, format!("{} bytes written", bytes.len()))
}

/// Renders an existing HTML file already on disk to a real PDF at a second
/// path. Takes a source path rather than inline content (unlike
/// CREATE_DOCX/XLSX/PPTX) because the natural workflow is "convert the file
/// you already wrote" — re-pasting a whole HTML document's markup into a
/// trigger parameter just to convert it would duplicate potentially large
/// content for no reason.
fn exec_export_pdf(t: &ParsedTrigger, ws: &str, conv_id: i64, snaps: &SnapshotStore) -> TriggerResult {
    let source_path = match t.params.first() {
        Some(p) => p,
        None => return fail(&t.action, "Missing source HTML path parameter"),
    };

    let output_path = match t.params.get(1) {
        Some(p) => p,
        None => return fail(&t.action, "Missing output PDF path parameter"),
    };

    let source_target = match validate_path(ws, source_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, source_path),
    };

    if !source_target.exists() {
        return fail_with_path(&t.action, &format!("File not found: {source_path}"), source_path);
    }

    let html = match fs::read_to_string(&source_target) {
        Ok(s) => s,
        Err(e) => return fail_with_path(&t.action, &format!("Cannot read {source_path}: {e}"), source_path),
    };

    let output_target = match validate_path(ws, output_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, output_path),
    };

    if let Some(parent) = output_target.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return fail_with_path(&t.action, &format!("Cannot create directory: {e}"), output_path);
        }
    }

    let bytes = match super::pdf::html_to_pdf(&html) {
        Ok(b) => b,
        Err(e) => return fail_with_path(&t.action, &e, output_path),
    };

    let _ = snaps.capture(conv_id, &t.action, output_target.to_str().unwrap_or(""));

    if let Err(e) = fs::write(&output_target, &bytes) {
        return fail_with_path(&t.action, &format!("Cannot write file: {e}"), output_path);
    }

    ok_with_detail(&t.action, output_path, format!("{} bytes written from {source_path}", bytes.len()))
}

fn exec_delete(t: &ParsedTrigger, ws: &str, conv_id: i64, snaps: &SnapshotStore) -> TriggerResult {
    let rel_path = match t.params.first() {
        Some(p) => p,
        None => return fail(&t.action, "Missing file path parameter"),
    };

    let target = match validate_path(ws, rel_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, rel_path),
    };

    if !target.exists() {
        return fail_with_path(&t.action, &format!("File not found: {rel_path}"), rel_path);
    }

    if target.is_dir() {
        return fail_with_path(&t.action, "Cannot delete directories, only files", rel_path);
    }

    let _ = snaps.capture(conv_id, "DELETE", target.to_str().unwrap_or(""));

    if let Err(e) = fs::remove_file(&target) {
        return fail_with_path(&t.action, &format!("Cannot delete file: {e}"), rel_path);
    }

    ok_with_detail(&t.action, rel_path, "File removed from disk".to_string())
}

fn exec_write(t: &ParsedTrigger, ws: &str, conv_id: i64, snaps: &SnapshotStore) -> TriggerResult {
    let rel_path = match t.params.first() {
        Some(p) => p,
        None => return fail(&t.action, "Missing file path parameter"),
    };

    let content = match t.params.get(1) {
        Some(c) => c,
        None => return fail(&t.action, "Missing file content parameter"),
    };

    let target = match validate_path(ws, rel_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, rel_path),
    };

    // Create parent dirs if needed
    if let Some(parent) = target.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return fail_with_path(&t.action, &format!("Cannot create directory: {e}"), rel_path);
        }
    }

    let _ = snaps.capture(conv_id, "WRITE", target.to_str().unwrap_or(""));

    if let Err(e) = fs::write(&target, content) {
        return fail_with_path(&t.action, &format!("Cannot write file: {e}"), rel_path);
    }

    ok_with_detail(&t.action, rel_path, format!("{} lines, {} bytes written", content.lines().count(), content.len()))
}

fn exec_insert(t: &ParsedTrigger, ws: &str, conv_id: i64, snaps: &SnapshotStore) -> TriggerResult {
    let rel_path = match t.params.first() {
        Some(p) => p,
        None => return fail(&t.action, "Missing file path parameter"),
    };

    let content = match t.params.get(1) {
        Some(c) => c,
        None => return fail(&t.action, "Missing content parameter"),
    };

    let line_str = match t.params.get(2) {
        Some(l) => l,
        None => return fail(&t.action, "Missing line number parameter"),
    };

    let line_num: i64 = match line_str.parse() {
        Ok(n) => n,
        Err(_) => return fail(&t.action, &format!("Invalid line number: {line_str}")),
    };

    let target = match validate_path(ws, rel_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, rel_path),
    };

    if !target.exists() {
        return fail_with_path(&t.action, &format!("File not found: {rel_path}"), rel_path);
    }

    let _ = snaps.capture(conv_id, "INSERT", target.to_str().unwrap_or(""));

    let existing = match fs::read_to_string(&target) {
        Ok(s) => s,
        Err(e) => return fail_with_path(&t.action, &format!("Cannot read file: {e}"), rel_path),
    };

    let mut lines: Vec<&str> = existing.lines().collect();
    let insert_idx = if line_num <= 0 {
        0
    } else if (line_num as usize) > lines.len() {
        lines.len()
    } else {
        (line_num - 1) as usize
    };

    let content_lines: Vec<&str> = content.lines().collect();
    for (i, line) in content_lines.iter().enumerate() {
        lines.insert(insert_idx + i, line);
    }

    let result = lines.join("\n");
    if let Err(e) = fs::write(&target, &result) {
        return fail_with_path(&t.action, &format!("Cannot write file: {e}"), rel_path);
    }

    ok_with_detail(&t.action, rel_path, format!("Inserted {} line(s) at line {line_num}", content_lines.len()))
}

fn exec_append(t: &ParsedTrigger, ws: &str, conv_id: i64, snaps: &SnapshotStore) -> TriggerResult {
    let rel_path = match t.params.first() {
        Some(p) => p,
        None => return fail(&t.action, "Missing file path parameter"),
    };

    let content = match t.params.get(1) {
        Some(c) => c,
        None => return fail(&t.action, "Missing content parameter"),
    };

    let target = match validate_path(ws, rel_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, rel_path),
    };

    if !target.exists() {
        return fail_with_path(&t.action, &format!("File not found: {rel_path}"), rel_path);
    }

    let _ = snaps.capture(conv_id, "APPEND", target.to_str().unwrap_or(""));

    let mut existing = match fs::read_to_string(&target) {
        Ok(s) => s,
        Err(e) => return fail_with_path(&t.action, &format!("Cannot read file: {e}"), rel_path),
    };

    if !existing.ends_with('\n') && !existing.is_empty() {
        existing.push('\n');
    }
    existing.push_str(content);

    if let Err(e) = fs::write(&target, &existing) {
        return fail_with_path(&t.action, &format!("Cannot write file: {e}"), rel_path);
    }

    ok_with_detail(&t.action, rel_path, format!("Appended {} bytes", content.len()))
}

fn exec_preview(t: &ParsedTrigger, ws: &str) -> TriggerResult {
    let rel_path = match t.params.first() {
        Some(p) => p,
        None => return fail(&t.action, "Missing file path parameter"),
    };

    let target = match validate_path(ws, rel_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, rel_path),
    };

    if !target.exists() {
        return fail_with_path(&t.action, &format!("File not found: {rel_path}"), rel_path);
    }

    // Frontend handles the actual preview via the trigger://executed event
    ok_with_detail(&t.action, rel_path, "Opened in viewer".to_string())
}

fn exec_read(t: &ParsedTrigger, ws: &str, responses: &mut Vec<ContentResponse>) -> TriggerResult {
    let rel_path = match t.params.first() {
        Some(p) => p,
        None => return fail(&t.action, "Missing file path parameter"),
    };

    let target = match validate_path(ws, rel_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, rel_path),
    };

    if !target.exists() {
        return fail_with_path(&t.action, &format!("File not found: {rel_path}"), rel_path);
    }

    if is_binary(&target) {
        return fail_with_path(&t.action, &format!("Cannot read binary file: {rel_path}"), rel_path);
    }

    match fs::read_to_string(&target) {
        Ok(content) => {
            let detail = format!("{} characters read", content.len());
            responses.push(ContentResponse {
                path: rel_path.clone(),
                content,
            });
            ok_with_detail(&t.action, rel_path, detail)
        }
        Err(e) => fail_with_path(&t.action, &format!("Cannot read file: {e}"), rel_path),
    }
}

fn exec_rename(t: &ParsedTrigger, ws: &str, conv_id: i64, snaps: &SnapshotStore) -> TriggerResult {
    let old_path = match t.params.first() {
        Some(p) => p,
        None => return fail(&t.action, "Missing old path parameter"),
    };

    let new_path = match t.params.get(1) {
        Some(p) => p,
        None => return fail(&t.action, "Missing new path parameter"),
    };

    let old_target = match validate_path(ws, old_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, old_path),
    };

    let new_target = match validate_path(ws, new_path) {
        Ok(p) => p,
        Err(e) => return fail_with_path(&t.action, &e, new_path),
    };

    if !old_target.exists() {
        return fail_with_path(&t.action, &format!("File not found: {old_path}"), old_path);
    }

    if new_target.exists() {
        return fail_with_path(&t.action, &format!("File already exists: {new_path}"), new_path);
    }

    if let Some(parent) = new_target.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return fail_with_path(&t.action, &format!("Cannot create directory: {e}"), new_path);
        }
    }

    let _ = snaps.capture_rename(
        conv_id,
        old_target.to_str().unwrap_or(""),
        new_target.to_str().unwrap_or(""),
    );

    if let Err(e) = fs::rename(&old_target, &new_target) {
        return fail_with_path(&t.action, &format!("Cannot rename: {e}"), old_path);
    }

    ok_with_detail(&t.action, new_path, format!("Renamed from {old_path}"))
}

fn exec_list(t: &ParsedTrigger, ws: &str, responses: &mut Vec<ContentResponse>) -> TriggerResult {
    let dir_path = match t.params.first() {
        Some(p) => p,
        None => return fail(&t.action, "Missing directory path parameter"),
    };

    let effective_path = if dir_path == "." || dir_path.is_empty() {
        "".to_string()
    } else {
        dir_path.clone()
    };

    let target = if effective_path.is_empty() {
        PathBuf::from(ws)
    } else {
        match validate_path(ws, &effective_path) {
            Ok(p) => p,
            Err(e) => return fail_with_path(&t.action, &e, dir_path),
        }
    };

    if !target.exists() || !target.is_dir() {
        return fail_with_path(&t.action, &format!("Directory not found: {dir_path}"), dir_path);
    }

    match fs::read_dir(&target) {
        Ok(entries) => {
            let mut listing: Vec<String> = entries
                .flatten()
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if e.path().is_dir() {
                        format!("{name}/")
                    } else {
                        name
                    }
                })
                .collect();
            listing.sort();
            let detail = format!("{} entries", listing.len());
            responses.push(ContentResponse {
                path: dir_path.clone(),
                content: listing.join("\n"),
            });
            ok_with_detail(&t.action, dir_path, detail)
        }
        Err(e) => fail_with_path(&t.action, &format!("Cannot list directory: {e}"), dir_path),
    }
}

/// A short factual summary of what happened (e.g. bytes written, lines
/// inserted) — shown when the user expands the action in the UI, since a
/// bare "OK" doesn't tell them much on its own.
fn ok_with_detail(action: &str, path: &str, detail: String) -> TriggerResult {
    TriggerResult {
        action: action.to_string(),
        status: "OK".into(),
        detail,
        file_path: Some(path.to_string()),
    }
}

fn fail(action: &str, detail: &str) -> TriggerResult {
    TriggerResult {
        action: action.to_string(),
        status: "FAIL".into(),
        detail: detail.to_string(),
        file_path: None,
    }
}

fn fail_with_path(action: &str, detail: &str, path: &str) -> TriggerResult {
    TriggerResult {
        action: action.to_string(),
        status: "FAIL".into(),
        detail: detail.to_string(),
        file_path: Some(path.to_string()),
    }
}

/// For outcomes that aren't really failures — the model doing exactly what
/// the protocol tells it to do (e.g. CREATE-ing a file to see whether it
/// exists yet, per the context.md idiom in llm-functions-v1.md) but landing
/// on a precondition that just means "nothing to do here". Neither the user
/// nor the model should read this as something having gone wrong.
fn warn_with_path(action: &str, detail: &str, path: &str) -> TriggerResult {
    TriggerResult {
        action: action.to_string(),
        status: "WARN".into(),
        detail: detail.to_string(),
        file_path: Some(path.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triggers::parser;

    fn temp_workspace() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("atelier_exec_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn setup_config() {
        let json = r#"{
            "version": 1,
            "levels": {
                "chat_only": { "label": "Chat Only", "description": ".", "allowed_triggers": ["MESSAGE"] },
                "full_access": { "label": "Full Access", "description": ".", "allowed_triggers": ["MESSAGE","CREATE","DELETE","WRITE","INSERT","APPEND","PREVIEW","READ","RENAME","LIST","CREATE_DOCX","CREATE_XLSX","CREATE_PPTX","CREATE_MP3","EXPORT_PDF"] }
            },
            "providers": {}
        }"#;
        let config = crate::llm::permissions::load_config_from_str(json).unwrap();
        let mut guard = crate::llm::permissions::CONFIG.write().unwrap();
        *guard = config;
    }

    #[test]
    fn create_and_write() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        let parsed = parser::parse(&format!(
            r#">>>[CREATE "test.txt"]<<<
>>>[WRITE "test.txt" "hello world"]<<<"#
        ));

        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results.len(), 2);
        assert_eq!(result.results[0].status, "OK");
        assert_eq!(result.results[1].status, "OK");
        assert_eq!(fs::read_to_string(ws.join("test.txt")).unwrap(), "hello world");

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn permission_denied() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        let parsed = parser::parse(r#">>>[CREATE "test.txt"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "chat_only", 1, &snaps);
        assert_eq!(result.results[0].status, "FAIL");
        assert!(result.results[0].detail.contains("Permission denied"));

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn read_returns_content() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        fs::write(ws.join("data.txt"), "file content").unwrap();

        let parsed = parser::parse(r#">>>[READ "data.txt"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "OK");
        assert_eq!(result.content_responses.len(), 1);
        assert_eq!(result.content_responses[0].content, "file content");

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn delete_nonexistent_fails() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        let parsed = parser::parse(r#">>>[DELETE "nope.txt"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "FAIL");
        assert!(result.results[0].detail.contains("not found"));

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn create_existing_warns_not_fails() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        fs::write(ws.join("exists.txt"), "data").unwrap();

        let parsed = parser::parse(r#">>>[CREATE "exists.txt"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        // Per llm-functions-v1.md, "CREATE a file to check if it exists" is a
        // documented, expected idiom (see the context.md protocol) — this is
        // not a real failure, so it shouldn't read as one.
        assert_eq!(result.results[0].status, "WARN");
        assert!(result.results[0].detail.contains("already exists"));
        // The existing file must be left untouched.
        assert_eq!(fs::read_to_string(ws.join("exists.txt")).unwrap(), "data");

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn list_directory() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        fs::write(ws.join("a.txt"), "").unwrap();
        fs::write(ws.join("b.txt"), "").unwrap();
        fs::create_dir(ws.join("subdir")).unwrap();

        let parsed = parser::parse(r#">>>[LIST "."]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "OK");
        assert_eq!(result.content_responses.len(), 1);
        let listing = &result.content_responses[0].content;
        assert!(listing.contains("a.txt"));
        assert!(listing.contains("b.txt"));
        assert!(listing.contains("subdir/"));

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn insert_at_line() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        fs::write(ws.join("file.txt"), "line1\nline2\nline3").unwrap();

        let parsed = parser::parse(r#">>>[INSERT "file.txt" "inserted" "2"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "OK");
        let content = fs::read_to_string(ws.join("file.txt")).unwrap();
        assert_eq!(content, "line1\ninserted\nline2\nline3");

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn append_to_file() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        fs::write(ws.join("file.txt"), "existing").unwrap();

        let parsed = parser::parse(r#">>>[APPEND "file.txt" "new stuff"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "OK");
        let content = fs::read_to_string(ws.join("file.txt")).unwrap();
        assert_eq!(content, "existing\nnew stuff");

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn rename_file() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        fs::write(ws.join("old.txt"), "data").unwrap();

        let parsed = parser::parse(r#">>>[RENAME "old.txt" "new.txt"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "OK");
        assert!(!ws.join("old.txt").exists());
        assert_eq!(fs::read_to_string(ws.join("new.txt")).unwrap(), "data");

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn create_docx_writes_a_real_zip() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        let parsed = parser::parse(r##">>>[CREATE_DOCX "report.docx" "# Title\nBody text."]<<<"##);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "OK");
        let bytes = fs::read(ws.join("report.docx")).unwrap();
        assert_eq!(&bytes[0..4], b"PK\x03\x04");

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn create_xlsx_writes_a_real_zip() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        let parsed = parser::parse(r#">>>[CREATE_XLSX "data.xlsx" "Name,Age\nAlice,30"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "OK");
        let bytes = fs::read(ws.join("data.xlsx")).unwrap();
        assert_eq!(&bytes[0..4], b"PK\x03\x04");

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn create_pptx_writes_a_real_zip() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        let parsed = parser::parse(r##">>>[CREATE_PPTX "deck.pptx" "# Slide One\n- point a"]<<<"##);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "OK");
        let bytes = fs::read(ws.join("deck.pptx")).unwrap();
        assert_eq!(&bytes[0..4], b"PK\x03\x04");

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn create_docx_overwrites_existing_file() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        fs::write(ws.join("report.docx"), "not a real docx yet").unwrap();

        let parsed = parser::parse(r##">>>[CREATE_DOCX "report.docx" "# Title"]<<<"##);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "OK");
        let bytes = fs::read(ws.join("report.docx")).unwrap();
        assert_eq!(&bytes[0..4], b"PK\x03\x04");

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn create_mp3_writes_real_audio_or_reports_a_clear_engine_error() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        let parsed = parser::parse(r#">>>[CREATE_MP3 "narration.mp3" "Hello from Atelier."]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);

        // Whether this succeeds depends on the OS TTS engine being present
        // (macOS always has `say`; Linux needs espeak-ng installed) — both
        // outcomes are valid here, see triggers::tts for the real,
        // unmocked engine test.
        match result.results[0].status.as_str() {
            "OK" => {
                let bytes = fs::read(ws.join("narration.mp3")).unwrap();
                assert_eq!(bytes[0], 0xFF);
            }
            "FAIL" => assert!(result.results[0].detail.contains("Text-to-speech requires")),
            other => panic!("unexpected status: {other}"),
        }

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn create_office_docs_denied_without_permission() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        let parsed = parser::parse(r#">>>[CREATE_XLSX "data.xlsx" "a,b\n1,2"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "chat_only", 1, &snaps);
        assert_eq!(result.results[0].status, "FAIL");
        assert!(result.results[0].detail.contains("Permission denied"));

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn export_pdf_renders_existing_html_file() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        fs::write(ws.join("plan.html"), "<html><body><h1>Plan</h1></body></html>").unwrap();

        let parsed = parser::parse(r#">>>[EXPORT_PDF "plan.html" "plan.pdf"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "OK");
        let bytes = fs::read(ws.join("plan.pdf")).unwrap();
        assert_eq!(&bytes[0..5], b"%PDF-");

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn export_pdf_fails_when_source_missing() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        let parsed = parser::parse(r#">>>[EXPORT_PDF "missing.html" "out.pdf"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "full_access", 1, &snaps);
        assert_eq!(result.results[0].status, "FAIL");
        assert!(result.results[0].detail.contains("not found"));

        fs::remove_dir_all(&ws).ok();
    }

    #[test]
    fn export_pdf_denied_without_permission() {
        setup_config();
        let ws = temp_workspace();
        let snaps = SnapshotStore::new();

        fs::write(ws.join("plan.html"), "<html><body>Plan</body></html>").unwrap();

        let parsed = parser::parse(r#">>>[EXPORT_PDF "plan.html" "plan.pdf"]<<<"#);
        let result = execute_batch(&parsed.triggers, ws.to_str().unwrap(), "chat_only", 1, &snaps);
        assert_eq!(result.results[0].status, "FAIL");
        assert!(result.results[0].detail.contains("Permission denied"));

        fs::remove_dir_all(&ws).ok();
    }
}
