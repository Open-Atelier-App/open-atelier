# Testing

## Automated Checks

The documented every-push checks are:

```bash
pnpm lint
pnpm exec tsc --noEmit -p .
pnpm test
cd src-tauri && cargo test
cd src-tauri && cargo clippy
```

These are listed in [docs/reference/source-records/qa/automated-qa.md](../reference/source-records/qa/automated-qa.md#L5). CI runs Rust fmt, clippy, tests, TypeScript, ESLint, Vitest, and a Tauri debug smoke build ([.github/workflows/ci.yml](../../.github/workflows/ci.yml#L40)).

## Existing Automated Tests

The inspected test files show one Vitest suite for model/provider metadata ([src/lib/types.test.ts](../../src/lib/types.test.ts#L4)). It verifies required providers, unique provider/model pairs, required fields, and provider badge entries.

## Manual QA

Manual QA covers stability, providers, skills, Studio, UI, and usage/cost estimates ([docs/reference/source-records/qa/manual-qa.md](../reference/source-records/qa/manual-qa.md#L5)).

## Known Test Gaps

The QA docs explicitly list missing tests:

- No command tests yet for profile/workspace commands ([docs/reference/source-records/qa/automated-qa.md](../reference/source-records/qa/automated-qa.md#L26)).
- No Rust-side provider request-shape tests yet ([docs/reference/source-records/qa/automated-qa.md](../reference/source-records/qa/automated-qa.md#L31)).
- No component tests for `FileViewerPanel`, `RightBar`, or onboarding flows ([docs/reference/source-records/qa/automated-qa.md](../reference/source-records/qa/automated-qa.md#L39)).

## Sandbox Limitations

The QA docs note that Rust/Tauri checks may fail in sandboxes without GTK/WebKit system libraries and should not be reported as passing if they did not run ([docs/reference/source-records/qa/automated-qa.md](../reference/source-records/qa/automated-qa.md#L18)).

## Debugging Failing Tests

- Provider model metadata failures usually point at [src/lib/types.ts](../../src/lib/types.ts#L149).
- Rust build or clippy failures may require Linux system packages listed in CI ([.github/workflows/ci.yml](../../.github/workflows/ci.yml#L32)).
- Tauri debug build failures may involve frontend build config in [src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json#L5) or Vite config in [vite.config.ts](../../vite.config.ts#L7).
