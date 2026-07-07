# Backend Architecture

## Purpose

The backend is the Tauri main process implemented in Rust. It owns local persistence, file operations, provider credentials, indexing, LLM routing, platform integration, and event emission. Main process bootstrapping starts in [src-tauri/src/main.rs](../../src-tauri/src/main.rs#L4) and [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs#L13).

## Top-Level Modules

- `db`: opens SQLite, applies schema, provides time helper ([src-tauri/src/db/mod.rs](../../src-tauri/src/db/mod.rs#L10)).
- `commands`: Tauri command modules for profiles, workspaces, files, chat, settings, CLI detection, and platform info ([src-tauri/src/commands/mod.rs](../../src-tauri/src/commands/mod.rs#L1)).
- `llm`: provider router and provider implementations ([src-tauri/src/llm/mod.rs](../../src-tauri/src/llm/mod.rs#L1)).
- `indexer`: background workspace indexing and embeddings ([src-tauri/src/indexer/mod.rs](../../src-tauri/src/indexer/mod.rs#L82)).
- `models`: serialized domain and event payload structs ([src-tauri/src/models/mod.rs](../../src-tauri/src/models/mod.rs#L3)).
- `error`: serializable application error types ([src-tauri/src/error.rs](../../src-tauri/src/error.rs#L4)).

## Command API

All public backend calls are registered in the Tauri invoke handler ([src-tauri/src/lib.rs](../../src-tauri/src/lib.rs#L77)). The commands are grouped by domain:

- Profiles: `profile_list`, `profile_create`, `profile_update`, `profile_delete`, `profile_switch`, `profile_get_active`, `profile_recreate_dir`.
- Workspaces: `workspace_open`, `workspace_list`, `workspace_close`, `workspace_rename`, `workspace_delete`, `workspace_relocate`.
- Files/index: `file_list_tree`, `file_create`, `file_rename`, `file_delete`, `file_read_raw`, `index_start`, `index_cancel`, `index_status`.
- Chat/search/tools: conversation commands, `ask`, tool approval/rejection, `search_hybrid`.
- Settings/credentials: `key_*`, `cred_*`, `settings_*`.
- Platform/CLI/skills: `platform_info`, `detect_cli_credentials`, OAuth status commands, `skill_list`.

See [../reference/api-reference.md](../reference/api-reference.md) for a command-by-command reference.

## LLM Routing

The LLM router converts conversation history into provider message JSON, selects a provider branch, fetches credentials when needed, and calls a provider module ([src-tauri/src/llm/router.rs](../../src-tauri/src/llm/router.rs#L31)).

Provider modes:

- OpenAI API key: [src-tauri/src/llm/openai.rs](../../src-tauri/src/llm/openai.rs#L6)
- Anthropic API key: [src-tauri/src/llm/anthropic.rs](../../src-tauri/src/llm/anthropic.rs#L6)
- Google API key: [src-tauri/src/llm/google.rs](../../src-tauri/src/llm/google.rs#L6)
- Ollama local URL: [src-tauri/src/llm/ollama.rs](../../src-tauri/src/llm/ollama.rs#L6)
- Apple FM sidecar: [src-tauri/src/llm/apple.rs](../../src-tauri/src/llm/apple.rs#L14)
- OpenAI-compatible vendors: [src-tauri/src/llm/openai_compatible.rs](../../src-tauri/src/llm/openai_compatible.rs#L90)
- CLI session reuse: [src-tauri/src/llm/anthropic_oauth.rs](../../src-tauri/src/llm/anthropic_oauth.rs#L153), [src-tauri/src/llm/codex_oauth.rs](../../src-tauri/src/llm/codex_oauth.rs#L51), [src-tauri/src/llm/gemini_oauth.rs](../../src-tauri/src/llm/gemini_oauth.rs#L53)

Streaming providers emit `chat://token` through `emit_token` ([src-tauri/src/llm/router.rs](../../src-tauri/src/llm/router.rs#L97)).

## Chat Command Lifecycle

The `ask` command inserts the user message, inserts a streaming assistant placeholder, loads history, prepends workspace/skill context, spawns provider streaming, updates the DB on completion/error, emits events, and optionally generates a title for the first exchange ([src-tauri/src/commands/chat.rs](../../src-tauri/src/commands/chat.rs#L101)).

## Indexing

`index_start` marks a workspace indexing and spawns `run_indexer` ([src-tauri/src/commands/files.rs](../../src-tauri/src/commands/files.rs#L175)). The indexer walks files, skips large/binary paths, chunks UTF-8 text, stores file/chunk rows, optionally embeds chunks through OpenAI or Ollama, and emits progress/completion events ([src-tauri/src/indexer/mod.rs](../../src-tauri/src/indexer/mod.rs#L90)).

## Credentials

Credentials are stored via `cred_store` in a local encrypted file under the app data directory ([src-tauri/src/commands/cred_store.rs](../../src-tauri/src/commands/cred_store.rs#L68)). The module states the encryption is not a security boundary against a determined local attacker ([src-tauri/src/commands/cred_store.rs](../../src-tauri/src/commands/cred_store.rs#L16)).

## Connectors

`connectors` holds opt-in, third-party integrations that call an external service directly rather than staying local-only — a deliberate exception to the rest of the app's local-first default, only active once the user connects/pastes their own credential in Settings > Connectors ([src-tauri/src/connectors/mod.rs](../../src-tauri/src/connectors/mod.rs#L1)). Four read-only triggers follow the same shape: `GITHUB_READ` (GitHub REST API, personal access token or native OAuth — [github.rs](../../src-tauri/src/connectors/github.rs#L1)), `NOTION_READ` (Notion API, internal integration token that must also be shared onto each target page — [notion.rs](../../src-tauri/src/connectors/notion.rs#L1)), `SLACK_READ` (Slack Web API, bot token that must be invited into each target channel — [slack.rs](../../src-tauri/src/connectors/slack.rs#L1)), and `GDRIVE_READ` (Google Drive API, either a plain API key limited to files shared "Anyone with the link", or native OAuth for the signed-in user's own private files — [google_drive.rs](../../src-tauri/src/connectors/google_drive.rs#L1)). Each token/key is stored under its own provider name (`"github"`/`"notion"`/`"slack"`/`"google_drive"`), cred_type `"api_key"` (pasted token) or `"bearer_token"` (OAuth access token), in the existing per-profile `cred_store`. Unlike CREATE/WRITE/READ/etc., all four are handled directly in `run_turn` — GITHUB_READ/NOTION_READ/SLACK_READ via the shared `handle_connector_read_trigger` helper, GDRIVE_READ via its own `handle_gdrive_read` (it needs to try OAuth first with a refresh-and-retry-once on a stale token, then fall back to the API key, which doesn't fit the single-token-lookup shape the other three share) — rather than through `triggers::executor`, since the executor is synchronous and workspace-filesystem-only, with no network or credential access ([src-tauri/src/commands/chat.rs](../../src-tauri/src/commands/chat.rs#L60)).

**Native OAuth.** GitHub and Google Drive additionally offer a real "Connect" button instead of paste-a-token, since both can be done without a client secret that would need a backend server to keep confidential:
- GitHub uses the OAuth **Device Flow** ([github_oauth.rs](../../src-tauri/src/connectors/github_oauth.rs#L1)) — no client secret involved at all; the resulting token is stored under the same `"api_key"` slot a pasted PAT would use, so nothing downstream needs to know which method produced it.
- Google Drive uses the **installed-app PKCE flow with a local loopback redirect** ([google_oauth.rs](../../src-tauri/src/connectors/google_oauth.rs#L1)) — Google's own recommended pattern for desktop apps. The access token is stored under `"bearer_token"` and the refresh token under `"oauth_refresh_token"`, both provider `"google_drive"`, separate from the API-key slot so both auth methods can coexist.

Both flows ship with placeholder OAuth client IDs (`CLIENT_ID`/`CLIENT_SECRET` constants at the top of each file) — this repo doesn't have its own registered OAuth Apps, so replace them with real ones from GitHub's [Developer settings](https://github.com/settings/developers) (enable Device Flow) and the [Google Cloud Console](https://console.cloud.google.com/apis/credentials) (Desktop app client type) respectively before shipping.

## Known Limitations

- `workspace_close` is currently a no-op ([src-tauri/src/commands/workspace.rs](../../src-tauri/src/commands/workspace.rs#L86)).
- `search_hybrid` currently omits vector rank fusion even when embeddings exist ([src-tauri/src/commands/chat.rs](../../src-tauri/src/commands/chat.rs#L348)).
- Tool calls can be stored and approved/rejected, but provider-native tool-use wiring is not started.
- Gemini API-key and OAuth paths filter out system messages ([src-tauri/src/llm/google.rs](../../src-tauri/src/llm/google.rs#L21), [src-tauri/src/llm/gemini_oauth.rs](../../src-tauri/src/llm/gemini_oauth.rs#L75)).
