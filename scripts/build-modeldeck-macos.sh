#!/usr/bin/env bash
set -euo pipefail

MODELDECK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$MODELDECK_ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This helper builds the macOS application. Use pnpm build on other platforms." >&2
  exit 1
fi
if [[ ! -d node_modules ]]; then
  echo "Run pnpm install --frozen-lockfile before building." >&2
  exit 1
fi

node node_modules/typescript/bin/tsc --noEmit
node scripts/tauri.mjs build --bundles app

MODELDECK_BUNDLE="$MODELDECK_ROOT/src-tauri/target/release/bundle/macos/ModelDeck.app"
MODELDECK_VERSION="$(node -p "require('./package.json').version")"
MODELDECK_ARCHIVE="$MODELDECK_ROOT/release/v$MODELDECK_VERSION/ModelDeck-$MODELDECK_VERSION-macos-$(uname -m).zip"
MODELDECK_PLIST="$MODELDECK_BUNDLE/Contents/Info.plist"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleDisplayName' "$MODELDECK_PLIST")" == "ModelDeck" ]]
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$MODELDECK_PLIST")" == "com.vita.modeldeck" ]]
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$MODELDECK_PLIST")" == "$MODELDECK_VERSION" ]]
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleURLTypes:0:CFBundleURLSchemes:0' "$MODELDECK_PLIST")" == "modeldeck" ]]
mkdir -p "$(dirname "$MODELDECK_ARCHIVE")"
cp "$MODELDECK_ROOT/LICENSE" "$MODELDECK_BUNDLE/Contents/Resources/LICENSE"
cp "$MODELDECK_ROOT/docs/UPSTREAM.md" "$MODELDECK_BUNDLE/Contents/Resources/SOURCE-NOTICE.md"
/usr/bin/codesign --force --deep --sign - "$MODELDECK_BUNDLE"
/usr/bin/codesign --verify --deep --strict "$MODELDECK_BUNDLE"
/usr/bin/ditto -c -k --sequesterRsrc --keepParent "$MODELDECK_BUNDLE" "$MODELDECK_ARCHIVE"
/usr/bin/shasum -a 256 "$MODELDECK_ARCHIVE" > "$MODELDECK_ARCHIVE.sha256"
echo "Built ModelDeck $MODELDECK_VERSION: $MODELDECK_ARCHIVE"
