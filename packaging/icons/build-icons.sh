#!/usr/bin/env bash
# Turn a piece of square-ish artwork into the committed icon assets.
#
# Usage: build-icons.sh <source-artwork.png>
#
# Requires ImageMagick 7 (brew install imagemagick).
#
# macOS does not round app icons for you: artwork must supply its own transparent
# corners or it renders as a hard-edged square beside every properly masked icon
# in the Dock. So the art is inset on Apple's icon grid and clipped to a
# superellipse.
set -euo pipefail

SRC="${1:?usage: build-icons.sh <source-artwork.png> [zoom]}"
# Fraction of the short edge to keep before masking. Generated artwork tends to
# carry generous margins, which stack with the inset below and leave the subject
# too small to read at 32px. Cropping in first is what makes it legible.
ZOOM="${2:-1.0}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ASSETS="$REPO_ROOT/native/assets"

CANVAS=1024
INSET=100                      # Apple's proportion: 824 of artwork on a 1024 canvas
SIDE=$((CANVAS - INSET * 2))
EXPONENT=5                     # superellipse power; 4-5 gives the continuous-corner look

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# --- square the source --------------------------------------------------------
# Centre-crop to the shorter edge rather than scaling both axes, so nothing
# stretches.
SHORT="$(magick identify -format "%[fx:int(min(w,h)*$ZOOM)]" "$SRC")"
magick "$SRC" -gravity center -crop "${SHORT}x${SHORT}+0+0" +repage \
  -resize "${SIDE}x${SIDE}!" "$WORK/art.png"

# --- superellipse mask --------------------------------------------------------
# Rendered at 2x and downsampled, which antialiases the curve for free.
HALF2=$(awk "BEGIN { print $SIDE - 0.5 }")
magick -size "$((SIDE * 2))x$((SIDE * 2))" xc:black \
  -fx "(abs((i-$HALF2)/$HALF2)^$EXPONENT + abs((j-$HALF2)/$HALF2)^$EXPONENT) <= 1 ? 1 : 0" \
  -resize "${SIDE}x${SIDE}" "$WORK/mask.png"

# --- composite ----------------------------------------------------------------
# Two invocations on purpose. `-alpha off` stays in effect as a setting for the
# rest of the command, and a later `-extent` in the same pipeline then collapses
# the result to GrayscaleAlpha, throwing the artwork away and leaving a black
# silhouette. Splitting resets the setting.
magick "$WORK/art.png" "$WORK/mask.png" \
  -alpha off -compose CopyOpacity -composite \
  "$WORK/masked.png"

magick "$WORK/masked.png" \
  -background none -gravity center -extent "${CANVAS}x${CANVAS}" \
  "$ASSETS/icon-1024.png"

# Guard against a silent recurrence: a full-colour icon must not be grayscale.
TYPE="$(magick identify -format '%[type]' "$ASSETS/icon-1024.png")"
case "$TYPE" in
  Grayscale*) echo "ERROR: artwork lost, icon came out $TYPE" >&2; exit 1 ;;
esac

# --- derivatives --------------------------------------------------------------
# 256 is what gets embedded in the binary for the window/taskbar icon.
magick "$ASSETS/icon-1024.png" -resize 256x256 "$ASSETS/icon.png"

# Multi-size ICO for the Windows exe.
magick "$ASSETS/icon-1024.png" \
  -define icon:auto-resize=256,128,64,48,32,16 "$ASSETS/icon.ico"

echo "wrote:"
for f in icon-1024.png icon.png icon.ico; do
  printf '  %-16s %s\n' "$f" "$(magick identify -format '%wx%h %b' "$ASSETS/$f" | head -1)"
done
echo
echo "corner alpha (should be 0): $(magick "$ASSETS/icon-1024.png" -format '%[fx:floor(255*p{4,4}.a)]' info:)"
echo "centre alpha (should be 255): $(magick "$ASSETS/icon-1024.png" -format '%[fx:floor(255*p{512,512}.a)]' info:)"
