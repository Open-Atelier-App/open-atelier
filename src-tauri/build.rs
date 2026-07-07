fn main() {
    // Best-effort short commit hash for the diagnostic report (see
    // commands::window::diagnostic_report) so a bug report can be pinned to
    // an exact build instead of just a version number that's identical
    // across every commit between releases. Falls back to "unknown" for
    // source tarballs/CI environments with no .git directory rather than
    // failing the build.
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=ATELIER_GIT_COMMIT={commit}");
    // Re-run when HEAD moves (new commit/checkout) so the embedded hash
    // doesn't go stale across incremental builds.
    println!("cargo:rerun-if-changed=../.git/HEAD");

    tauri_build::build()
}
