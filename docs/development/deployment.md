# Deployment

## Local Production Build

Verified build commands:

```bash
pnpm build
pnpm tauri build
```

`pnpm build` runs TypeScript and Vite build ([package.json](../../package.json#L6)). `pnpm tauri build` invokes the Tauri CLI through the `tauri` script ([package.json](../../package.json#L10)). Tauri runs `pnpm build` before bundling ([src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json#L5)).

## Bundle Configuration

The Tauri bundle is active for all targets and uses configured icons ([src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json#L29)). The app identifier is `com.openatelier.app` ([src-tauri/tauri.conf.json](../../src-tauri/tauri.conf.json#L2)).

## CI Build

CI runs a debug Tauri build after linting and tests ([.github/workflows/ci.yml](../../.github/workflows/ci.yml#L61)).

## Release Workflow

Release workflow triggers on tags matching `v*` ([.github/workflows/release.yml](../../.github/workflows/release.yml#L3)). It builds:

- `aarch64-apple-darwin` on macOS
- `x86_64-apple-darwin` on macOS
- `x86_64-unknown-linux-gnu` on Ubuntu

These targets are defined in the release matrix ([.github/workflows/release.yml](../../.github/workflows/release.yml#L10)). Artifacts are uploaded from Tauri bundle output paths ([.github/workflows/release.yml](../../.github/workflows/release.yml#L55)).

## Deployment Uncertainties

- No GitHub Release creation step was found in the release workflow.
- No signing/notarization configuration was found in the inspected Tauri config.
- No hosted backend, cloud infrastructure, Dockerfile, or server deployment was found in the repository.
