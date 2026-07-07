# Development Setup

## Prerequisites

Verified prerequisites from repository docs and workflows:

- Node 22 is used in CI ([.github/workflows/ci.yml](../../.github/workflows/ci.yml#L27)).
- pnpm is used for frontend dependencies and scripts ([package.json](../../package.json#L6)).
- Rust stable is installed in CI ([.github/workflows/ci.yml](../../.github/workflows/ci.yml#L19)).
- Linux builds require WebKit/GTK/AppIndicator/Soup/DBus/OpenSSL packages on Ubuntu ([.github/workflows/ci.yml](../../.github/workflows/ci.yml#L32)).
- The CREATE_MP3 trigger's text-to-speech needs `espeak-ng` installed on Linux (macOS uses the built-in `say` command, no extra install needed) ([.github/workflows/ci.yml](../../.github/workflows/ci.yml#L38), [src-tauri/src/triggers/tts.rs](../../src-tauri/src/triggers/tts.rs#L1)).

Source builds require Rust 1.75+, Node 22+, and pnpm.

## Install

```bash
pnpm install
```

This is used by CI and the runbook ([.github/workflows/ci.yml](../../.github/workflows/ci.yml#L49), [docs/reference/source-records/operations/runbook.md](../reference/source-records/operations/runbook.md#L29)).

## Repository Structure

```text
src/                React renderer process
src/components/     UI components
src/stores/         Zustand state stores
src/lib/            Tauri IPC wrapper and shared frontend types
src/hooks/          Tauri event hooks
src-tauri/src/      Rust Tauri backend
src-tauri/src/db/   SQLite connection and schema
src-tauri/src/commands/ Tauri command handlers
src-tauri/src/llm/  LLM providers and router
src-tauri/src/indexer/ Workspace indexing and embeddings
docs/               Project documentation
.github/workflows/ CI and release workflows
```

The renderer/main process split is documented in [docs/reference/source-records/architecture/development-reference.md](../reference/source-records/architecture/development-reference.md#L29).

## Running The Application

For the full desktop app:

```bash
pnpm tauri dev
```

Tauri config runs `pnpm dev` first and expects Vite at `http://localhost:1420` ([src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json#L5)).

For frontend-only Vite dev:

```bash
pnpm dev
```

`pnpm dev` maps to `vite` ([package.json](../../package.json#L6)).

## Environment Variables

Only one environment variable is read directly in the inspected code:

- `TAURI_DEV_HOST`: used by Vite to bind host and configure HMR host/port ([vite.config.ts](../../vite.config.ts#L5)).

See [../reference/environment-variables.md](../reference/environment-variables.md).

## Common Development Workflows

### Add A Frontend Command Call

1. Add a typed wrapper in [src/lib/tauri.ts](../../src/lib/tauri.ts#L10).
2. Implement or reuse a Rust command registered in [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs#L77).
3. Consume the wrapper from a store or component.

### Add A Provider Model

1. Add provider/model metadata in [src/lib/types.ts](../../src/lib/types.ts#L149).
2. Ensure provider badge coverage in [src/lib/types.ts](../../src/lib/types.ts#L181).
3. Add or reuse a backend router branch in [src-tauri/src/llm/router.rs](../../src-tauri/src/llm/router.rs#L42).
4. Add provider implementation or OpenAI-compatible base URL in [src-tauri/src/llm/openai_compatible.rs](../../src-tauri/src/llm/openai_compatible.rs#L90).

### Add A File Operation

1. Validate paths under the workspace root using the pattern in [src-tauri/src/commands/files.rs](../../src-tauri/src/commands/files.rs#L18).
2. Add the Tauri command and frontend wrapper.
3. Refresh file tree state through [src/stores/workspaceStore.ts](../../src/stores/workspaceStore.ts#L83).

## Coding Conventions Inferred From Code

- TypeScript strict mode is enabled with unused locals/parameters denied ([tsconfig.json](../../tsconfig.json#L2)).
- ESLint uses recommended JS/TypeScript configs plus React Hooks and React Refresh rules ([eslint.config.js](../../eslint.config.js#L7)).
- Frontend command access is centralized in `src/lib/tauri.ts`.
- Backend errors use serializable `AtelierError` and `ErrorCode` ([src-tauri/src/error.rs](../../src-tauri/src/error.rs#L4)).
- Backend domain modules return `crate::error::Result<T>` and map IO/DB/provider failures to `AtelierError`.
- UI styling is currently inline plus CSS variables in [src/index.css](../../src/index.css#L3).

## Debugging Guide

- Frontend startup: check Vite on port `1420` ([vite.config.ts](../../vite.config.ts#L15)).
- Tauri startup: DB open failures happen during `setup` before command registration ([src-tauri/src/lib.rs](../../src-tauri/src/lib.rs#L19)).
- Chat streaming: inspect `ask`, provider router, and `chat://*` event listeners ([src-tauri/src/commands/chat.rs](../../src-tauri/src/commands/chat.rs#L101), [src/hooks/useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L21)).
- Indexing: inspect `index_start`, `run_indexer`, and `index://*` listeners ([src-tauri/src/commands/files.rs](../../src-tauri/src/commands/files.rs#L175), [src-tauri/src/indexer/mod.rs](../../src-tauri/src/indexer/mod.rs#L82)).
- Credentials: inspect settings commands and `cred_store` ([src-tauri/src/commands/settings.rs](../../src-tauri/src/commands/settings.rs#L20), [src-tauri/src/commands/cred_store.rs](../../src-tauri/src/commands/cred_store.rs#L1)).

## Deployment Process

See [deployment.md](deployment.md). Verified commands are `pnpm build` and `pnpm tauri build` ([package.json](../../package.json#L6), [src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json#L5)).
