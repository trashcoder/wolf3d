#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_PATH="${WOLF3D_BIN:-$ROOT_DIR/target/release/wolf3d}"
DIST_DIR="${WOLF3D_DIST_DIR:-$ROOT_DIR/dist}"
APP_DIR="$DIST_DIR/AppDir"

[[ "$(uname -m)" == "x86_64" ]] || {
  printf 'Build the Linux AppImage on an x86_64 host.\n' >&2
  exit 1
}
if [[ ! -x "$BIN_PATH" ]]; then
  printf 'Release executable not found: %s\n' "$BIN_PATH" >&2
  exit 1
fi
command -v linuxdeploy >/dev/null || {
  printf 'linuxdeploy is required to bundle SDL2 runtime dependencies.\n' >&2
  exit 1
}
command -v appimagetool >/dev/null || {
  printf 'appimagetool is required to create the AppImage.\n' >&2
  exit 1
}

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/usr/bin" "$APP_DIR/usr/share/applications" \
  "$APP_DIR/usr/share/icons/hicolor/scalable/apps"
install -m 0755 "$BIN_PATH" "$APP_DIR/usr/bin/wolf3d"
install -m 0644 "$ROOT_DIR/packaging/linux/wolf3d.desktop" \
  "$APP_DIR/usr/share/applications/wolf3d.desktop"
install -m 0644 "$ROOT_DIR/packaging/linux/wolf3d.svg" \
  "$APP_DIR/usr/share/icons/hicolor/scalable/apps/wolf3d.svg"
install -m 0644 "$ROOT_DIR/README.rst" "$APP_DIR/README.rst"
install -m 0644 "$ROOT_DIR/PORTING.md" "$APP_DIR/PORTING.md"
install -m 0644 "$ROOT_DIR/WOLFSRC/README/LICENSE.DOC" "$APP_DIR/LICENSE.DOC"

cd "$DIST_DIR"
linuxdeploy --appdir "$APP_DIR" --executable "$APP_DIR/usr/bin/wolf3d" \
  --desktop-file "$APP_DIR/usr/share/applications/wolf3d.desktop" \
  --output appimage
