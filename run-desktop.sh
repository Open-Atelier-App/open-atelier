#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

APP_NAME="Open Atelier"
DEV_PORT="${TAURI_DEV_PORT:-1420}"

print_usage() {
  cat <<EOF
Usage: ./run-desktop.sh

Starts ${APP_NAME} as the local Tauri desktop app.
This runs: pnpm tauri dev
EOF
}

require_command() {
  local command_name="$1"
  local install_hint="$2"

  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Missing required command: ${command_name}" >&2
    echo "$install_hint" >&2
    exit 1
  fi
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  print_usage
  exit 0
fi

require_command "node" "Install Node 22 or newer, then try again."
require_command "pnpm" "Install pnpm, then try again: corepack enable"
require_command "cargo" "Install Rust stable from https://rustup.rs, then try again."

NODE_MAJOR="$(node -p "Number(process.versions.node.split('.')[0])" 2>/dev/null || echo 0)"
if [[ "$NODE_MAJOR" -lt 22 ]]; then
  echo "Warning: project docs expect Node 22 or newer; current Node is $(node --version)." >&2
fi

if [[ ! -d "node_modules" ]]; then
  echo "Installing frontend dependencies..."
  pnpm install
fi

if command -v lsof >/dev/null 2>&1; then
  if lsof -nP -iTCP:"$DEV_PORT" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "Port ${DEV_PORT} is already in use." >&2
    echo "Close the existing dev server first; Tauri starts Vite itself through beforeDevCommand." >&2
    echo >&2
    lsof -nP -iTCP:"$DEV_PORT" -sTCP:LISTEN >&2
    exit 1
  fi
fi

echo "Starting ${APP_NAME} as a desktop app..."
exec pnpm tauri dev
