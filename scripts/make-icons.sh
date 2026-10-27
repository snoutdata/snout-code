#!/usr/bin/env bash
# Rebuild the app icon files from assets/icon/snout.svg:
#   assets/icon/snout.icns     the macOS bundle icon (scripts/package-mac.sh)
#   assets/icon/snout-512.png  the window icon compiled into the binary
#
# Needs resvg (cargo install resvg) and iconutil (Xcode command line tools).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SVG="$ROOT/assets/icon/snout.svg"
RESVG="${RESVG:-resvg}"
command -v "$RESVG" >/dev/null || { echo "resvg not found; cargo install resvg" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
SET="$WORK/snout.iconset"
mkdir "$SET"
for size in 16 32 128 256 512; do
  "$RESVG" -w "$size" -h "$size" "$SVG" "$SET/icon_${size}x${size}.png"
  "$RESVG" -w $((size * 2)) -h $((size * 2)) "$SVG" "$SET/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$SET" -o "$ROOT/assets/icon/snout.icns"
cp "$SET/icon_256x256@2x.png" "$ROOT/assets/icon/snout-512.png"
echo "Wrote assets/icon/snout.icns and assets/icon/snout-512.png"
