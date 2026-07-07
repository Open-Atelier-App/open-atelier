#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

APP_NAME="Open Atelier"
BUNDLE_ID="com.openatelier.app"
SOURCE_APP="$ROOT_DIR/src-tauri/target/release/bundle/macos/${APP_NAME}.app"
DEST_APP="${DEST_APP:-/Applications/${APP_NAME}.app}"
BUILD_APP=1
OPEN_AFTER=0
QUIT_RUNNING=1

print_usage() {
  cat <<EOF
Usage: ./update-macos-app.sh [--no-build] [--no-quit] [--open]

Builds the latest macOS .app bundle and replaces:
  ${DEST_APP}

Options:
  --no-build  Install the existing bundle without rebuilding first.
  --no-quit   Do not ask a running copy of ${APP_NAME} to quit before replacing.
  --open      Open ${APP_NAME} after installing it.
  -h, --help  Show this help.
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

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-build)
      BUILD_APP=0
      shift
      ;;
    --no-quit)
      QUIT_RUNNING=0
      shift
      ;;
    --open)
      OPEN_AFTER=1
      shift
      ;;
    -h|--help)
      print_usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      print_usage >&2
      exit 1
      ;;
  esac
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script installs the macOS app and must be run on macOS." >&2
  exit 1
fi

require_command "node" "Install Node 22 or newer, then try again."
require_command "pnpm" "Install pnpm, then try again: corepack enable"
require_command "cargo" "Install Rust stable from https://rustup.rs, then try again."
require_command "ditto" "ditto is included with macOS; install the Xcode command line tools if it is missing."

if [[ "$BUILD_APP" -eq 1 ]]; then
  echo "Installing dependencies..."
  pnpm install

  echo "Building ${APP_NAME}.app..."
  pnpm tauri build --bundles app
fi

if [[ ! -d "$SOURCE_APP" ]]; then
  echo "Missing built app bundle: ${SOURCE_APP}" >&2
  echo "Run without --no-build to create it." >&2
  exit 1
fi

if [[ "$QUIT_RUNNING" -eq 1 ]] && pgrep -x "open-atelier" >/dev/null 2>&1; then
  echo "Asking any running copy of ${APP_NAME} to quit..."
  osascript -e "tell application id \"${BUNDLE_ID}\" to quit" >/dev/null 2>&1 || true

  for _ in {1..20}; do
    if ! pgrep -x "open-atelier" >/dev/null 2>&1; then
      break
    fi
    sleep 0.5
  done

  if pgrep -x "open-atelier" >/dev/null 2>&1; then
    echo "${APP_NAME} is still running; close it and rerun this script, or use --no-quit." >&2
    exit 1
  fi
fi

TMP_APP="$(dirname "$DEST_APP")/.${APP_NAME}.app.tmp"
echo "Replacing ${DEST_APP}..."
rm -rf "$TMP_APP"
ditto "$SOURCE_APP" "$TMP_APP"
rm -rf "$DEST_APP"
mv "$TMP_APP" "$DEST_APP"
xattr -dr com.apple.quarantine "$DEST_APP" 2>/dev/null || true

echo "Installed ${DEST_APP}"

if [[ "$OPEN_AFTER" -eq 1 ]]; then
  open "$DEST_APP"
fi
