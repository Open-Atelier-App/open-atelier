# Module Guide

This document summarizes every major module inspected in the repository.

## Frontend Bootstrap

- Purpose: mount React app and global error boundary.
- Public interfaces: `App` render tree.
- Dependencies: React, ReactDOM, `ErrorBoundary`, CSS.
- Data models: none.
- Important workflows: `ReactDOM.createRoot(...).render(...)`.
- Known limitations: none found.
- Related files: [src/main.tsx](../../src/main.tsx#L7), [src/App.tsx](../../src/App.tsx#L19), [src/components/ErrorBoundary.tsx](../../src/components/ErrorBoundary.tsx#L6).

## Frontend IPC Wrapper

- Purpose: typed wrapper around Tauri `invoke`.
- Public interfaces: profile, workspace, file, index, search, chat, tools, settings, credential, platform, CLI, OAuth, and skills functions.
- Dependencies: `@tauri-apps/api/core`, shared frontend types.
- Data models: imports `Profile`, `Workspace`, `Message`, `Citation`, `ToolCall`, `KeyStatus`.
- Important workflows: maps camelCase frontend args to Rust command names.
- Known limitations: wrappers assume command names stay synchronized with Rust registration.
- Related files: [src/lib/tauri.ts](../../src/lib/tauri.ts#L10), [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs#L77).

## Shared Frontend Types

- Purpose: TypeScript mirror of Rust structs and provider metadata.
- Public interfaces: domain interfaces, `MODEL_OPTIONS`, `availableModels`, `PROVIDER_BADGE`.
- Dependencies: none outside TypeScript.
- Data models: profiles, workspaces, files, conversations, messages, citations, tool calls, events, provider/model metadata.
- Important workflows: provider/model picker and tests consume `MODEL_OPTIONS`.
- Known limitations: provider connectivity and model metadata can drift from backend credential support.
- Related files: [src/lib/types.ts](../../src/lib/types.ts#L1), [src/lib/types.test.ts](../../src/lib/types.test.ts#L4).

## Profile Store

- Purpose: manage profile list and active profile.
- Public interfaces: `load`, `create`, `switch`, `delete`, `update`.
- Dependencies: `src/lib/tauri.ts`.
- Data models: `Profile`.
- Important workflows: load list and active profile concurrently; switch updates active flags.
- Known limitations: path existence recovery is handled in components/UI state, not inside the store.
- Related files: [src/stores/profileStore.ts](../../src/stores/profileStore.ts#L5), [src/components/onboarding/ProfileSetup.tsx](../../src/components/onboarding/ProfileSetup.tsx#L28).

## Workspace Store

- Purpose: manage workspace list, active workspace, file tree, and indexing progress.
- Public interfaces: `load`, `open`, `setActive`, `loadFileTree`, `startIndex`, `updateIndexProgress`, `updateWorkspaceStatus`.
- Dependencies: Tauri wrapper.
- Data models: `Workspace`, `FileNode`, `IndexProgress`.
- Important workflows: opening a workspace fires background indexing; setting active workspace loads file tree.
- Known limitations: index start is fire-and-forget in `open`.
- Related files: [src/stores/workspaceStore.ts](../../src/stores/workspaceStore.ts#L5).

## Chat Store

- Purpose: manage conversations, messages, streaming state, citations, and tool calls.
- Public interfaces: conversation CRUD, `sendMessage`, `startConversationAndSend`, token/event handlers, tool approval/rejection.
- Dependencies: Tauri wrapper.
- Data models: `Conversation`, `Message`, `Citation`, `ToolCall`.
- Important workflows: optimistic local user/assistant messages before `ask` returns real assistant ID.
- Known limitations: `cancelStreaming` only changes frontend state; no backend cancellation command was found.
- Related files: [src/stores/chatStore.ts](../../src/stores/chatStore.ts#L5), [src-tauri/src/commands/chat.rs](../../src-tauri/src/commands/chat.rs#L101).

## UI Store

- Purpose: global UI state for sidebars, file viewer, theme, settings modal, selected model, profile recovery, CLI provider opt-in, and keyboard intents.
- Public interfaces: toggles/setters, `setModel`, `setCliProviderEnabled`, `loadEnabledCliProviders`, intent triggers.
- Dependencies: Tauri settings commands.
- Data models: selected provider/model strings, enabled CLI provider set.
- Important workflows: persists enabled CLI providers to `app_settings`.
- Known limitations: theme is applied to DOM but not persisted in inspected code.
- Related files: [src/stores/uiStore.ts](../../src/stores/uiStore.ts#L4).

## Layout Components

- Purpose: primary UI layout and navigation.
- Public interfaces: `LeftSidebar`, `CenterPane`, `RightBar`.
- Dependencies: stores, Tauri dialog/fs/shell plugins, Tauri wrapper.
- Data models: profiles, workspaces, file nodes, conversations.
- Important workflows: open/create project, profile switching/recovery, file tree operations, chat pane switching.
- Known limitations: file icons and viewer dispatch are hardcoded.
- Related files: [src/components/layout/LeftSidebar.tsx](../../src/components/layout/LeftSidebar.tsx#L16), [src/components/layout/CenterPane.tsx](../../src/components/layout/CenterPane.tsx#L8), [src/components/layout/RightBar.tsx](../../src/components/layout/RightBar.tsx#L34).

## Chat Components

- Purpose: provider/model selection, message composition, chat rendering, citations, and tool approval UI.
- Public interfaces: `ChatInput`, `ChatView`, `MessageBubble`, `CitationList`, `ToolCallCard`, `ProviderBadge`.
- Dependencies: chat/workspace/UI stores, Tauri wrapper, Markdown renderer.
- Data models: messages, citations, tool calls, providers.
- Important workflows: filter models by connected provider, stream messages via store updates, save assistant response as Markdown.
- Known limitations: provider availability filtering may omit newer providers because backend status list is incomplete.
- Related files: [src/components/chat/ChatInput.tsx](../../src/components/chat/ChatInput.tsx#L21), [src/components/chat/ChatView.tsx](../../src/components/chat/ChatView.tsx#L9), [src/components/chat/MessageBubble.tsx](../../src/components/chat/MessageBubble.tsx#L16), [src/components/chat/ToolCallCard.tsx](../../src/components/chat/ToolCallCard.tsx#L18).

## Settings UI

- Purpose: provider credential management, CLI session opt-in, skills display, theme, usage/cost estimates, and about text.
- Public interfaces: `SettingsView`.
- Dependencies: Tauri credential/settings/CLI/platform/skill commands; stores.
- Data models: provider config, credential state, CLI status, skills, usage rows.
- Important workflows: save/delete/test credentials, check CLI sessions, list profile skills, estimate costs from message token counts.
- Known limitations: pricing is hardcoded and approximate; key tests cover only some providers.
- Related files: [src/components/settings/SettingsView.tsx](../../src/components/settings/SettingsView.tsx#L173).

## Tauri App Bootstrap

- Purpose: initialize plugins, database, menu, commands, and app runtime.
- Public interfaces: `run()`.
- Dependencies: Tauri, plugins, DB module, commands, LLM skill/OAuth commands.
- Data models: managed `Db`.
- Important workflows: resolves app data DB path, opens DB, emits preferences menu event.
- Known limitations: startup uses `expect`, so DB path/open failure aborts startup.
- Related files: [src-tauri/src/main.rs](../../src-tauri/src/main.rs#L4), [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs#L13).

## Database Module

- Purpose: open SQLite, apply schema, provide DB type and `now_ms`.
- Public interfaces: `open`, `open_in_memory`, `now_ms`.
- Dependencies: rusqlite, schema, `AtelierError`.
- Data models: all schema tables.
- Important workflows: enables foreign keys/WAL/busy timeout.
- Known limitations: no schema version table found.
- Related files: [src-tauri/src/db/mod.rs](../../src-tauri/src/db/mod.rs#L10), [src-tauri/src/db/schema.rs](../../src-tauri/src/db/schema.rs#L1).

## Profile Commands

- Purpose: profile CRUD, active profile switching, directory recreation.
- Public interfaces: Tauri commands `profile_*`.
- Dependencies: DB, filesystem, models.
- Data models: `Profile`.
- Important workflows: profile creation creates the profile directory; profile switch clears all active flags then marks one active.
- Known limitations: `profile_update` changes root path without checking existence in the command.
- Related files: [src-tauri/src/commands/profile.rs](../../src-tauri/src/commands/profile.rs#L19).

## Workspace Commands

- Purpose: open/list/rename/delete/relocate workspaces.
- Public interfaces: Tauri commands `workspace_*`.
- Dependencies: DB, path canonicalization.
- Data models: `Workspace`.
- Important workflows: `workspace_open` validates path exists and is under active profile root.
- Known limitations: `workspace_close` is a no-op; `workspace_relocate` validates existence but does not enforce active-profile-root constraint in inspected code.
- Related files: [src-tauri/src/commands/workspace.rs](../../src-tauri/src/commands/workspace.rs#L28).

## File And Index Commands

- Purpose: file tree, file mutation/read, index start/cancel/status.
- Public interfaces: `file_*`, `index_*`.
- Dependencies: DB, filesystem, indexer.
- Data models: `WorkspaceFile`, `FileNode`.
- Important workflows: `validate_path` prevents path escape; `index_start` spawns background indexer.
- Known limitations: `index_cancel` only sets DB status idle; no cancellation token was found for a running index task.
- Related files: [src-tauri/src/commands/files.rs](../../src-tauri/src/commands/files.rs#L18).

## Chat Commands

- Purpose: conversation CRUD, chat streaming orchestration, search, tool approval/rejection.
- Public interfaces: conversation commands, `ask`, `search_hybrid`, `tool_*`.
- Dependencies: DB, LLM router, skills, Tauri events.
- Data models: `Conversation`, `Message`, `Citation`, `ToolCall`.
- Important workflows: `ask` inserts messages, prepends skill context, spawns streaming task, emits completion/error/title events.
- Known limitations: `search_hybrid` currently uses BM25-only ranking despite embedding storage.
- Related files: [src-tauri/src/commands/chat.rs](../../src-tauri/src/commands/chat.rs#L36).

## Settings And Credential Commands

- Purpose: credential persistence, key testing, app settings.
- Public interfaces: `key_*`, `cred_*`, `settings_*`.
- Dependencies: credential store, DB, reqwest.
- Data models: `KeyStatus`, JSON settings.
- Important workflows: credentials are stored locally; settings are stored in lazily created `app_settings`.
- Known limitations: `key_list_status` omits several providers present in Settings.
- Related files: [src-tauri/src/commands/settings.rs](../../src-tauri/src/commands/settings.rs#L20), [src-tauri/src/commands/cred_store.rs](../../src-tauri/src/commands/cred_store.rs#L1).

## CLI Detection

- Purpose: detect usable local CLI sessions without reading secrets unless provider helper requires parsing session format.
- Public interfaces: `detect_cli_credentials`.
- Dependencies: home directory paths, OAuth helper modules.
- Data models: `CliCredentialStatus`.
- Important workflows: checks external CLI session files (Codex CLI, Gemini CLI, etc.); helpers validate usability.
- Known limitations: macOS Keychain detection for external CLI sessions is separate and user-triggered.
- Related files: [src-tauri/src/commands/cli_detect.rs](../../src-tauri/src/commands/cli_detect.rs#L28).

## LLM Modules

- Purpose: route and stream chat completions from external/local providers.
- Public interfaces: `stream_chat`, provider `stream` functions, OAuth `stream_oauth` functions.
- Dependencies: reqwest, provider APIs, credential store, Tauri events.
- Data models: provider JSON payloads, `StreamResult`.
- Important workflows: router fetches credentials, selects provider, provider streams tokens and token usage.
- Known limitations: no provider trait; roadmap says trait should be considered later if duplication grows.
- Related files: [src-tauri/src/llm/router.rs](../../src-tauri/src/llm/router.rs#L31), [src-tauri/src/llm/openai_compatible.rs](../../src-tauri/src/llm/openai_compatible.rs#L90).

## Skills

- Purpose: load Markdown instructions from profile folders and prepend them to LLM requests.
- Public interfaces: `skill_list`, `build_context`, `load_skills`.
- Dependencies: filesystem.
- Data models: `Skill`, `SkillInfo`.
- Important workflows: all non-empty `.md` files under `<profile_root>/skills/` are sorted by name and included.
- Known limitations: no per-workspace/task scoping; no editor UI.
- Related files: [src-tauri/src/llm/skills.rs](../../src-tauri/src/llm/skills.rs#L25).

## Indexer And Embedder

- Purpose: walk workspace files, chunk text, store FTS/embedding data.
- Public interfaces: `run_indexer`, `embed_texts`.
- Dependencies: filesystem, DB, OpenAI/Ollama embedding APIs, Tauri events.
- Data models: files, chunks, chunk embeddings.
- Important workflows: skip hidden/binary/large files; upsert metadata; batch embeddings.
- Known limitations: embeddings are stored but not used in search ranking yet.
- Related files: [src-tauri/src/indexer/mod.rs](../../src-tauri/src/indexer/mod.rs#L82), [src-tauri/src/indexer/embedder.rs](../../src-tauri/src/indexer/embedder.rs#L11).
