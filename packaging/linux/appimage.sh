#!/usr/bin/env bash
# Wrap a built Linux binary in an AppImage.
#
# Usage: appimage.sh <binary> <version> <output-dir>
set -euo pipefail

BINARY="${1:?usage: appimage.sh <binary> <version> <output-dir>}"
VERSION="${2:?missing version}"
OUT_DIR="${3:?missing output dir}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ICON="$REPO_ROOT/native/assets/icon.png"

EXECUTABLE="neaponoise"
DISPLAY_NAME="Neapolitan Noise"
FILE_STEM="Neapolitan-Noise"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

APPDIR="$WORK/NeapolitanNoise.AppDir"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/256x256/apps"

cp "$BINARY" "$APPDIR/usr/bin/$EXECUTABLE"
chmod +x "$APPDIR/usr/bin/$EXECUTABLE"

cp "$ICON" "$APPDIR/usr/share/icons/hicolor/256x256/apps/$EXECUTABLE.png"
cp "$ICON" "$APPDIR/$EXECUTABLE.png"

cat > "$APPDIR/usr/share/applications/$EXECUTABLE.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=$DISPLAY_NAME
Comment=White, brown and pink noise generator
Exec=$EXECUTABLE
Icon=$EXECUTABLE
Categories=Audio;AudioVideo;
Terminal=false
DESKTOP
cp "$APPDIR/usr/share/applications/$EXECUTABLE.desktop" "$APPDIR/$EXECUTABLE.desktop"

cat > "$APPDIR/AppRun" <<APPRUN
#!/bin/sh
HERE="\$(dirname "\$(readlink -f "\$0")")"
exec "\$HERE/usr/bin/$EXECUTABLE" "\$@"
APPRUN
chmod +x "$APPDIR/AppRun"

# appimagetool itself ships as an AppImage; --appimage-extract-and-run avoids
# needing FUSE, which GitHub runners do not provide.
TOOL="$WORK/appimagetool"
curl -fsSL -o "$TOOL" \
  "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
chmod +x "$TOOL"

mkdir -p "$OUT_DIR"
OUTPUT="$OUT_DIR/$FILE_STEM-$VERSION-linux-x86_64.AppImage"
rm -f "$OUTPUT"
ARCH=x86_64 "$TOOL" --appimage-extract-and-run "$APPDIR" "$OUTPUT"

echo "built $OUTPUT"
