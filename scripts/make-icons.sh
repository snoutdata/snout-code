#!/usr/bin/env bash
# Rebuild the app icon files from assets/icon/snout.svg:
#   assets/icon/snout.icns     the macOS bundle icon (scripts/package-mac.sh)
#   assets/icon/snout-512.png  the window icon compiled into the binary
#   assets/icon/snout.ico      the Windows program and installer icon
#
# Needs resvg (cargo install resvg), iconutil (Xcode command line tools) and python3.
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

# The .ico: PNG images in the sizes Windows looks for.
ICO="$WORK/ico"
mkdir "$ICO"
for size in 16 24 32 48 64 128 256; do
  "$RESVG" -w "$size" -h "$size" "$SVG" "$ICO/$size.png"
done
python3 -I - "$ICO" "$ROOT/assets/icon/snout.ico" <<'PY'
import os, struct, sys
src, out = sys.argv[1], sys.argv[2]
sizes = [16, 24, 32, 48, 64, 128, 256]
images = [open(os.path.join(src, f"{s}.png"), "rb").read() for s in sizes]
offset = 6 + 16 * len(images)
entries = b""
for size, data in zip(sizes, images):
    entries += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(data), offset)
    offset += len(data)
open(out, "wb").write(struct.pack("<HHH", 0, 1, len(images)) + entries + b"".join(images))
PY
echo "Wrote assets/icon/snout.icns, assets/icon/snout-512.png and assets/icon/snout.ico"
