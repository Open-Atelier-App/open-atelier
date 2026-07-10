# Environment Variables

Only environment variables that are read directly by repository code are listed here.

| Variable | Used By | Purpose | Source |
|---|---|---|---|
| `TAURI_DEV_HOST` | Vite config | Sets Vite dev-server host and HMR host/port when developing with Tauri on a custom host. | [vite.config.ts](../../vite.config.ts#L5) |
| `ATELIER_GOOGLE_OAUTH_CLIENT_ID` | Rust build (`build.rs`) | Compile-time override for the Google Drive OAuth client ID; falls back to a rejected placeholder when unset. | [src-tauri/src/connectors/google_oauth.rs](../../src-tauri/src/connectors/google_oauth.rs#L26) |
| `ATELIER_GOOGLE_OAUTH_CLIENT_SECRET` | Rust build (`build.rs`) | Compile-time override for the Google Drive OAuth client secret (non-confidential for a Desktop app client, per Google's installed-app guidance); falls back to a rejected placeholder when unset. | [src-tauri/src/connectors/google_oauth.rs](../../src-tauri/src/connectors/google_oauth.rs#L29) |
| `ATELIER_GITHUB_OAUTH_CLIENT_ID` | Rust build (`build.rs`) | Compile-time override for the GitHub OAuth App client ID (Device Flow); falls back to a rejected placeholder when unset. | [src-tauri/src/connectors/github_oauth.rs](../../src-tauri/src/connectors/github_oauth.rs#L19) |

These three are read with `option_env!` at compile time (not `std::env::var` at runtime), so they must be set in the build environment (e.g. CI) before `cargo build`/`tauri build` runs — setting them on a machine that only runs the already-built binary has no effect.
