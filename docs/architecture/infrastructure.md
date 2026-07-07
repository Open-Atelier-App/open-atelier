# Infrastructure And Deployment Architecture

## Runtime Shape

Open Atelier is a Tauri desktop application. It runs a React frontend in the WebView and a Rust backend in the Tauri main process. Tauri config sets one main window with title "Open Atelier" and minimum size constraints ([src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json#L11)).

## Build Configuration

Tauri uses `../dist` for production frontend assets, `http://localhost:1420` for development, `pnpm dev` before dev runs, and `pnpm build` before production builds ([src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json#L5)). Vite uses port `1420`, strict port mode, optional `TAURI_DEV_HOST`, and ignores `src-tauri` for file watching ([vite.config.ts](../../vite.config.ts#L15)).

## Native Plugins And Permissions

The app installs Tauri dialog, filesystem, and shell plugins ([src-tauri/src/lib.rs](../../src-tauri/src/lib.rs#L16)). Capabilities allow dialog open/save/message/ask, filesystem read/write/read-dir/mkdir/remove/rename/exists/stat with `fs:scope` of `**`, and shell open ([src-tauri/capabilities/default.json](../../src-tauri/capabilities/default.json#L6)).

Security note: Tauri config has `csp: null` and asset protocol scope `**` ([src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json#L21)). Application commands add path validation for workspace/file operations, but the Tauri capability scope itself is broad.

## CI

CI runs on `macos-14` and `ubuntu-22.04`, installs Rust and Node/pnpm, installs Linux WebKit/GTK dependencies on Ubuntu, runs Rust fmt/clippy/tests, TypeScript, ESLint, Vitest, and a Tauri debug build ([.github/workflows/ci.yml](../../.github/workflows/ci.yml#L10)).

## Release

Release builds run on tag pushes matching `v*` ([.github/workflows/release.yml](../../.github/workflows/release.yml#L3)). The matrix builds:

- macOS arm64 DMG
- macOS x64 DMG
- Linux x86_64 AppImage/DEB paths

The workflow uploads artifacts from Tauri bundle directories ([.github/workflows/release.yml](../../.github/workflows/release.yml#L55)).

Uncertainty: no step in the inspected release workflow creates a GitHub Release object.

## Supported Platforms

Supported platforms: macOS 12+ and Ubuntu 22.04+. Release workflow targets macOS and Ubuntu/Linux only ([.github/workflows/release.yml](../../.github/workflows/release.yml#L13)).

## Local Storage

The SQLite database is created in the app data directory as `atelier.db` ([src-tauri/src/lib.rs](../../src-tauri/src/lib.rs#L20)). Credentials are stored in the app data directory under `com.openatelier.app` using local files `.cred_master_key` and `credentials.enc.json` ([src-tauri/src/commands/cred_store.rs](../../src-tauri/src/commands/cred_store.rs#L30)).

## Unverified Infrastructure

- No Dockerfile, Kubernetes config, hosted backend, or cloud deployment files were found in the inspected repository file list.
- No environment-specific `.env` file or env schema was found; only `TAURI_DEV_HOST` is read directly by code ([vite.config.ts](../../vite.config.ts#L5)).
