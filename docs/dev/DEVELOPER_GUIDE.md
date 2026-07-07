# Developer Guide

This document preserves the requested `docs/dev/DEVELOPER_GUIDE.md` entry point. The maintained development docs are split by task:

- [Setup](../development/setup.md)
- [Testing](../development/testing.md)
- [Deployment](../development/deployment.md)
- [Coding standards](../ai/coding-standards.md)
- [Implementation rules](../ai/implementation-rules.md)
- [API reference](../reference/api-reference.md)

Only repository-verified information is included in the linked docs.

## Local Setup

Use pnpm for dependencies and scripts. CI installs Node 22, Rust stable, pnpm dependencies, and Linux Tauri system packages ([.github/workflows/ci.yml](../../.github/workflows/ci.yml#L19), [.github/workflows/ci.yml](../../.github/workflows/ci.yml#L27), [.github/workflows/ci.yml](../../.github/workflows/ci.yml#L32), [package.json](../../package.json#L6)).

```bash
pnpm install
```

## Environment Variables

`TAURI_DEV_HOST` is the only environment variable read directly in inspected source; Vite uses it for host binding and HMR configuration ([vite.config.ts](../../vite.config.ts#L5)). See [environment-variables.md](../reference/environment-variables.md).

## Running The Application

```bash
pnpm tauri dev
```

Tauri starts the frontend with `pnpm dev` and expects `http://localhost:1420` during development ([src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json#L5), [vite.config.ts](../../vite.config.ts#L15)).

## Running Tests

Repository scripts expose `pnpm lint`, `pnpm test`, and `pnpm build`; CI also runs `cargo fmt`, `cargo clippy`, `cargo test`, TypeScript, ESLint, Vitest, and a debug Tauri build ([package.json](../../package.json#L6), [.github/workflows/ci.yml](../../.github/workflows/ci.yml#L39)).

## Repository Structure

```text
src/                 React renderer
src/components/      UI components
src/stores/          Zustand stores
src/lib/             Tauri wrappers and shared types
src-tauri/src/       Rust Tauri main process
src-tauri/src/db/    SQLite schema and connection
src-tauri/src/llm/   Provider routing and provider modules
src-tauri/src/indexer/ Workspace indexing
docs/                Documentation
```

## Common Workflows

- Add frontend command access in `src/lib/tauri.ts`, then register/implement the Rust command in `src-tauri/src/lib.rs` and a command module ([src/lib/tauri.ts](../../src/lib/tauri.ts#L10), [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs#L77)).
- Add provider metadata in `src/lib/types.ts` and route provider behavior through `src-tauri/src/llm/router.rs` ([src/lib/types.ts](../../src/lib/types.ts#L149), [src-tauri/src/llm/router.rs](../../src-tauri/src/llm/router.rs#L31)).
- Keep file operations behind workspace-root validation ([src-tauri/src/commands/files.rs](../../src-tauri/src/commands/files.rs#L18)).

## Coding Conventions

TypeScript strict mode and unused checks are enabled; ESLint uses TypeScript, React Hooks, and React Refresh rules. Rust commands generally return serializable `AtelierError` results ([tsconfig.json](../../tsconfig.json#L2), [eslint.config.js](../../eslint.config.js#L7), [src-tauri/src/error.rs](../../src-tauri/src/error.rs#L4)).

## Debugging Guide

- Frontend startup: inspect Vite port `1420` and `TAURI_DEV_HOST` ([vite.config.ts](../../vite.config.ts#L15)).
- Tauri startup: DB setup happens before command registration ([src-tauri/src/lib.rs](../../src-tauri/src/lib.rs#L19)).
- Chat streaming: inspect `ask`, `llm/router.rs`, and `chat://*` listeners ([src-tauri/src/commands/chat.rs](../../src-tauri/src/commands/chat.rs#L101), [src-tauri/src/llm/router.rs](../../src-tauri/src/llm/router.rs#L97), [src/hooks/useTauriEvents.ts](../../src/hooks/useTauriEvents.ts#L21)).
- Indexing: inspect `index_start`, `run_indexer`, and `index://*` events ([src-tauri/src/commands/files.rs](../../src-tauri/src/commands/files.rs#L175), [src-tauri/src/indexer/mod.rs](../../src-tauri/src/indexer/mod.rs#L82)).

## Deployment Process

Local packaging uses `pnpm tauri build`. CI performs debug packaging; tag-based release workflow builds macOS and Linux artifacts and uploads them as workflow artifacts ([package.json](../../package.json#L6), [.github/workflows/ci.yml](../../.github/workflows/ci.yml#L59), [.github/workflows/release.yml](../../.github/workflows/release.yml#L52)).
