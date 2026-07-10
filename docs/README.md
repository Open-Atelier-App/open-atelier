# Open Atelier Documentation

Technical documentation for contributors. Product documentation and guides are available at [doc.open-atelier.app](https://doc.open-atelier.app).

## Architecture

- [System overview](architecture/overview.md) - application purpose, patterns, module dependency map, data flows, request lifecycle, auth flow, background jobs, external services, and deployment shape.
- [Frontend architecture](architecture/frontend.md) - React app structure, state stores, components, and event handling.
- [Backend architecture](architecture/backend.md) - Tauri command layer, Rust modules, LLM routing, indexing, credentials, and events.
- [Database architecture](architecture/database.md) - SQLite schema, relationships, migrations, and data ownership.
- [Infrastructure](architecture/infrastructure.md) - Tauri bundle config, permissions, CI, release workflow, and runtime environment.
- [Architecture compatibility index](architecture/ARCHITECTURE.md) - bridge for the explicitly requested architecture path.

## Development

- [Setup](development/setup.md) - prerequisites, install, local setup, and repository structure.
- [Testing](development/testing.md) - automated tests, manual QA, and current coverage gaps.
- [Deployment](development/deployment.md) - local builds, Tauri packaging, CI, and release artifacts.
- [Developer guide bridge](dev/DEVELOPER_GUIDE.md) - single-page index for setup, testing, deployment, conventions, and debugging.

## Reference

- [Environment variables](reference/environment-variables.md)
- [API reference](reference/api-reference.md)
- [July 2026 improvement pass](reference/2026-07-improvement-pass.md) - every feature added while working through the French-language improvement report: composer changes, sidebar features, chat message viewers (charts, math, Mermaid, recipe/map/kanban/weather cards, scratch timers), and what was deliberately scoped down or left out.
- [Source records](reference/source-records/README.md)

## AI Context

- [Project memory](ai/project-memory.md)
- [Coding standards](ai/coding-standards.md)
- [Implementation rules](ai/implementation-rules.md)
- [Project memory bridge](project-memory.md)
- [Module guide](module/MODULES.md)

## Source Records

Preserved earlier project notes, QA evidence, and operational context. Retained because generated docs cite them as evidence.

- [Development reference](reference/source-records/architecture/development-reference.md)
- [Operations runbook](reference/source-records/operations/runbook.md)
- [Run report](reference/source-records/operations/run-report.md)
- [Automated QA](reference/source-records/qa/automated-qa.md)
- [Manual QA](reference/source-records/qa/manual-qa.md)
- [QA result: 2026-06-18](reference/source-records/qa/results/2026-06-18.md)
