#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
X86_BIN="${WOLF3D_X86_64_BIN:-$ROOT_DIR/target/x86_64-apple-darwin/release/wolf3d}"
ARM_BIN="${WOLF3D_ARM64_BIN:-$ROOT_DIR/target/aarch64-apple-darwin/release/wolf3d}"
SDL2_FRAMEWORK="${SDL2_FRAMEWORK_BUNDLE:-}"
SIGN_IDENTITY="${MACOS_CODESIGN_IDENTITY:-}"
NOTARY_PROFILE="${MACOS_NOTARY_PROFILE:-}"
DIST_DIR="${WOLF3D_DIST_DIR:-$ROOT_DIR/dist}"
APP_DIR="$DIST_DIR/Wolfenstein 3D.app"
CONTENTS="$APP_DIR/Contents"
DMG_PATH="$DIST_DIR/wolf3d-macos-universal.dmg"

for tool in lipo otool install_name_tool hdiutil; do
  command -v "$tool" >/dev/null || { printf '%s is required.\n' "$tool" >&2; exit 1; }
done
[[ -x "$X86_BIN" ]] || { printf 'Intel executable not found: %s\n' "$X86_BIN" >&2; exit 1; }
[[ -x "$ARM_BIN" ]] || { printf 'Apple Silicon executable not found: %s\n' "$ARM_BIN" >&2; exit 1; }
[[ -d "$SDL2_FRAMEWORK/SDL2.framework" ]] || {
  printf 'Set SDL2_FRAMEWORK_BUNDLE to the directory containing SDL2.framework.\n' >&2
  exit 1
}
if [[ -n "$NOTARY_PROFILE" && -z "$SIGN_IDENTITY" ]]; then
  printf 'MACOS_CODESIGN_IDENTITY is required when notarizing the DMG.\n' >&2
  exit 1
fi

rm -rf "$APP_DIR"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Frameworks" "$CONTENTS/Resources"
lipo -create "$X86_BIN" "$ARM_BIN" -output "$CONTENTS/MacOS/wolf3d"
ditto "$SDL2_FRAMEWORK/SDL2.framework" "$CONTENTS/Frameworks/SDL2.framework"
SDL2_BINARY="$CONTENTS/Frameworks/SDL2.framework/SDL2"
lipo -verify_arch x86_64 arm64 "$CONTENTS/MacOS/wolf3d"
lipo -verify_arch x86_64 arm64 "$SDL2_BINARY"
install -m 0644 "$ROOT_DIR/packaging/macos/Info.plist" "$CONTENTS/Info.plist"
install -m 0644 "$ROOT_DIR/README.rst" "$CONTENTS/Resources/README.rst"
install -m 0644 "$ROOT_DIR/PORTING.md" "$CONTENTS/Resources/PORTING.md"
install -m 0644 "$ROOT_DIR/WOLFSRC/README/LICENSE.DOC" "$CONTENTS/Resources/LICENSE.DOC"
APP_BINARY="$CONTENTS/MacOS/wolf3d"
SDL2_DEPENDENCIES="$(
  {
    otool -L "$X86_BIN"
    otool -L "$ARM_BIN"
  } | awk '$1 ~ /SDL2/ && $1 !~ /:$/ { print $1 }' | sort -u
)"
[[ -n "$SDL2_DEPENDENCIES" ]] || {
  printf 'Neither target executable links SDL2; cannot set the bundled framework dependency.\n' >&2
  exit 1
}
while IFS= read -r dependency; do
  install_name_tool -change "$dependency" \
    @executable_path/../Frameworks/SDL2.framework/SDL2 "$APP_BINARY"
done <<< "$SDL2_DEPENDENCIES"

if [[ -n "$SIGN_IDENTITY" ]]; then
  codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" \
    "$CONTENTS/Frameworks/SDL2.framework"
  codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" "$APP_DIR"
  codesign --verify --deep --strict "$APP_DIR"
fi

rm -f "$DMG_PATH"
hdiutil create -volname "Wolfenstein 3D" -srcfolder "$APP_DIR" \
  -ov -format UDZO "$DMG_PATH"
if [[ -n "$NOTARY_PROFILE" ]]; then
  xcrun notarytool submit "$DMG_PATH" --keychain-profile "$NOTARY_PROFILE" --wait
  xcrun stapler staple "$DMG_PATH"
  xcrun stapler validate "$DMG_PATH"
fi
printf 'Created %s\n' "$DMG_PATH"
