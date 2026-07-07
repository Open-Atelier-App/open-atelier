#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

APP_NAME="Open Atelier"
BUNDLE_ID="com.openatelier.app"
BUNDLE_TYPE="appimage"
BUNDLE_DIR="$ROOT_DIR/src-tauri/target/release/bundle/${BUNDLE_TYPE}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/share/open-atelier}"
DEST_APP="$INSTALL_DIR/Open Atelier.AppImage"
DESKTOP_FILE="$HOME/.local/share/applications/open-atelier.desktop"
BUILD_APP=1
OPEN_AFTER=0
QUIT_RUNNING=1
INSTALL_DESKTOP_ENTRY=1

print_usage() {
  cat <<EOF
Usage: ./update-linux-app.sh [--bundle appimage|deb|rpm] [--no-build] [--no-quit] [--no-desktop-entry] [--open]

Builds the latest Linux app bundle and installs it to:
  ${DEST_APP}

Options:
  --bundle <type>      Bundle target to build (appimage, deb, rpm). Defaults to appimage.
  --no-build            Install the existing bundle without rebuilding first.
  --no-quit             Do not ask a running copy of ${APP_NAME} to quit before replacing.
  --no-desktop-entry    Skip installing a ~/.local/share/applications launcher entry.
  --open                Open ${APP_NAME} after installing it.
  -h, --help            Show this help.

Only the appimage bundle is installed by this script (a single portable
executable, no root required). --bundle deb/rpm builds the package but
leaves it in src-tauri/target/release/bundle/ for you to install with your
system's package manager (dpkg/rpm), since that requires root.
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
    --bundle)
      BUNDLE_TYPE="${2:-}"
      if [[ -z "$BUNDLE_TYPE" ]]; then
        echo "--bundle requires a value (appimage, deb, or rpm)" >&2
        exit 1
      fi
      shift 2
      ;;
    --no-build)
      BUILD_APP=0
      shift
      ;;
    --no-quit)
      QUIT_RUNNING=0
      shift
      ;;
    --no-desktop-entry)
      INSTALL_DESKTOP_ENTRY=0
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

case "$BUNDLE_TYPE" in
  appimage|deb|rpm) ;;
  *)
    echo "Unsupported --bundle value: ${BUNDLE_TYPE} (expected appimage, deb, or rpm)" >&2
    exit 1
    ;;
esac
BUNDLE_DIR="$ROOT_DIR/src-tauri/target/release/bundle/${BUNDLE_TYPE}"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "This script builds the Linux app and must be run on Linux." >&2
  exit 1
fi

require_command "node" "Install Node 22 or newer, then try again."
require_command "pnpm" "Install pnpm, then try again: corepack enable"
require_command "cargo" "Install Rust stable from https://rustup.rs, then try again."

if [[ "$BUILD_APP" -eq 1 ]]; then
  echo "Installing dependencies..."
  pnpm install

  echo "Building ${APP_NAME} (${BUNDLE_TYPE})..."
  pnpm tauri build --bundles "$BUNDLE_TYPE"
fi

if [[ ! -d "$BUNDLE_DIR" ]]; then
  echo "Missing built bundle directory: ${BUNDLE_DIR}" >&2
  echo "Run without --no-build to create it." >&2
  exit 1
fi

if [[ "$BUNDLE_TYPE" != "appimage" ]]; then
  found_package="$(find "$BUNDLE_DIR" -maxdepth 1 -type f -print -quit)"
  if [[ -z "$found_package" ]]; then
    echo "No .${BUNDLE_TYPE} package found in ${BUNDLE_DIR}" >&2
    exit 1
  fi
  echo "Built package: ${found_package}"
  echo "This script only auto-installs the appimage bundle (no root required)."
  echo "Install this one with your system's package manager, e.g.:"
  if [[ "$BUNDLE_TYPE" == "deb" ]]; then
    echo "  sudo dpkg -i \"${found_package}\""
  else
    echo "  sudo rpm -i \"${found_package}\""
  fi
  exit 0
fi

SOURCE_APP="$(find "$BUNDLE_DIR" -maxdepth 1 -type f -name '*.AppImage' -print -quit)"
if [[ -z "$SOURCE_APP" ]]; then
  echo "No .AppImage found in ${BUNDLE_DIR}" >&2
  echo "Run without --no-build to create it." >&2
  exit 1
fi

if [[ "$QUIT_RUNNING" -eq 1 ]] && pgrep -f "open-atelier" >/dev/null 2>&1; then
  echo "Asking any running copy of ${APP_NAME} to quit..."
  pkill -f "open-atelier" 2>/dev/null || true

  for _ in {1..20}; do
    if ! pgrep -f "open-atelier" >/dev/null 2>&1; then
      break
    fi
    sleep 0.5
  done

  if pgrep -f "open-atelier" >/dev/null 2>&1; then
    echo "${APP_NAME} is still running; close it and rerun this script, or use --no-quit." >&2
    exit 1
  fi
fi

echo "Installing to ${DEST_APP}..."
mkdir -p "$INSTALL_DIR"
TMP_APP="$INSTALL_DIR/.Open Atelier.AppImage.tmp"
cp "$SOURCE_APP" "$TMP_APP"
chmod +x "$TMP_APP"
rm -f "$DEST_APP"
mv "$TMP_APP" "$DEST_APP"

echo "Installed ${DEST_APP}"

if [[ "$INSTALL_DESKTOP_ENTRY" -eq 1 ]]; then
  mkdir -p "$(dirname "$DESKTOP_FILE")"
  cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=${APP_NAME}
Comment=Local-first AI workspace
Exec=env DESKTOP_FILE_ID=${BUNDLE_ID} "${DEST_APP}" %U
Icon=${BUNDLE_ID}
Terminal=false
Categories=Development;Utility;
EOF
  echo "Installed launcher entry: ${DESKTOP_FILE}"
fi

if [[ "$OPEN_AFTER" -eq 1 ]]; then
  "$DEST_APP" >/dev/null 2>&1 &
  disown
fi
