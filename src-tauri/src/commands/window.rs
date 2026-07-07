use tauri::{AppHandle, Manager};
use crate::error::{AtelierError, ErrorCode, Result};

/// Returns platform info for conditional UI features.
#[tauri::command]
pub fn platform_info() -> serde_json::Value {
    serde_json::json!({
        "os": std::env::consts::OS,         // "macos" | "linux" | "windows"
        "arch": std::env::consts::ARCH,
        "apple_fm_available": cfg!(target_os = "macos"),
    })
}

// Bounds how much of the log file gets pulled into a report — old entries
// don't help diagnose what just happened, and there's no reason to hand
// megabytes of history to a chat message. Raised from 200_000 now that user
// input, full raw LLM responses, and every trigger execution are logged
// (not just parse errors and provider failures), so a handful of turns can
// otherwise push earlier ones out of the tail before they're even reported.
const MAX_LOG_BYTES: usize = 750_000;

/// A copy-pasteable block with app/platform info plus the tail of the log
/// file (populated by tauri-plugin-log, see lib.rs) — meant to be attached
/// to a bug report instead of a screenshot alone.
#[tauri::command]
pub fn diagnostic_report(app: AppHandle) -> Result<String> {
    let mut report = String::new();
    report.push_str(&format!("Open Atelier {}\n", app.package_info().version));
    report.push_str(&format!("Commit: {}\n", env!("ATELIER_GIT_COMMIT")));
    report.push_str(&format!("OS: {} ({})\n", std::env::consts::OS, std::env::consts::ARCH));
    report.push_str("\n--- Recent log output ---\n");

    let log_dir = app.path().app_log_dir()
        .map_err(|e| AtelierError::new(ErrorCode::IoError, format!("Cannot resolve log directory: {e}")))?;
    let log_path = log_dir.join(format!("{}.log", app.package_info().name));

    match std::fs::read(&log_path) {
        Ok(bytes) => {
            let start = bytes.len().saturating_sub(MAX_LOG_BYTES);
            let tail = String::from_utf8_lossy(&bytes[start..]);
            if start > 0 {
                report.push_str("[...earlier log entries truncated...]\n");
            }
            report.push_str(&tail);
        }
        Err(e) => {
            report.push_str(&format!("(no log file yet at {}: {e})\n", log_path.display()));
        }
    }

    Ok(report)
}

#[tauri::command]
pub fn open_path(path: String) -> Result<()> {
    open::that(&path)
        .map_err(|e| AtelierError::io(format!("Failed to open path: {e}")))?;
    Ok(())
}
