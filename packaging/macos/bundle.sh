#!/usr/bin/env bash
# Assemble "Neapolitan Noise.app" from a built binary and wrap it in a DMG.
#
# Usage: bundle.sh <binary> <version> <output-dir>
#
# Runs on macOS only (needs iconutil, codesign, hdiutil).
set -euo pipefail

BINARY="${1:?usage: bundle.sh <binary> <version> <output-dir>}"
VERSION="${2:?missing version}"
OUT_DIR="${3:?missing output dir}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ICON_SOURCE="$REPO_ROOT/native/assets/icon-1024.png"

# Display name has a space; the executable inside the bundle does not.
APP_NAME="Neapolitan Noise"
EXECUTABLE="neaponoise"
# Reverse-DNS of a domain actually owned, which is the convention and keeps the ID
# globally unique. Worth getting right now: changing it later makes macOS treat
# this as a different app, with fresh Gatekeeper approval and settings.
BUNDLE_ID="me.scottbrinkmeyer.neapolitan-noise"
# Hyphens rather than spaces in artifact filenames, so download URLs stay clean.
FILE_STEM="Neapolitan-Noise"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

APP="$WORK/$APP_NAME.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BINARY" "$APP/Contents/MacOS/$EXECUTABLE"
chmod +x "$APP/Contents/MacOS/$EXECUTABLE"

# --- icon ---------------------------------------------------------------------
ICONSET="$WORK/icon.iconset"
mkdir -p "$ICONSET"
for spec in "16:icon_16x16" "32:icon_16x16@2x" "32:icon_32x32" "64:icon_32x32@2x" \
            "128:icon_128x128" "256:icon_128x128@2x" "256:icon_256x256" \
            "512:icon_256x256@2x" "512:icon_512x512" "1024:icon_512x512@2x"; do
  size="${spec%%:*}"
  name="${spec##*:}"
  sips -s format png -z "$size" "$size" "$ICON_SOURCE" --out "$ICONSET/$name.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/icon.icns"

# --- Info.plist ---------------------------------------------------------------
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$APP_NAME</string>
  <key>CFBundleDisplayName</key><string>$APP_NAME</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleExecutable</key><string>$EXECUTABLE</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.music</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# --- signature ----------------------------------------------------------------
# Ad-hoc signature. Required, not optional: `lipo` invalidates the signature the
# linker puts on each slice, and macOS refuses to run an unsigned arm64 binary.
# This is not notarization; Gatekeeper still warns on first open.
codesign --force --sign - --timestamp=none "$APP" >/dev/null 2>&1 \
  || codesign --force --sign - "$APP/Contents/MacOS/$EXECUTABLE"
codesign --verify --deep --strict "$APP"

# --- dmg ----------------------------------------------------------------------
STAGING="$WORK/dmg"
mkdir -p "$STAGING"
cp -R "$APP" "$STAGING/"
ln -s /Applications "$STAGING/Applications"

mkdir -p "$OUT_DIR"

# Name the DMG after what is actually inside it, so a local single-arch build
# is not mislabelled as universal.
case "$(lipo -archs "$APP/Contents/MacOS/$EXECUTABLE")" in
  *arm64*x86_64*|*x86_64*arm64*) ARCH_LABEL="universal" ;;
  *arm64*)                       ARCH_LABEL="arm64" ;;
  *x86_64*)                      ARCH_LABEL="x64" ;;
  *)                             ARCH_LABEL="unknown" ;;
esac

DMG="$OUT_DIR/$FILE_STEM-$VERSION-macos-$ARCH_LABEL.dmg"
rm -f "$DMG"
hdiutil create \
  -volname "$APP_NAME" \
  -srcfolder "$STAGING" \
  -ov -format UDZO \
  "$DMG" >/dev/null

echo "built $DMG"
lipo -archs "$APP/Contents/MacOS/$EXECUTABLE"
