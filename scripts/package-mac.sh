#!/usr/bin/env bash
# Build snout.app and a .dmg to install it from, in target/release/bundle/.
#
#   scripts/package-mac.sh               for this Mac's architecture
#   scripts/package-mac.sh --universal   Apple silicon and Intel in one binary
#                                        (rustup target add x86_64-apple-darwin aarch64-apple-darwin)
#
# Signing: with a Developer ID, the app and .dmg are signed (hardened runtime), and the .dmg is
# notarized and stapled, so it opens on any Mac. Set, in the environment or in .signing.env at the
# repository root (gitignored):
#   SNOUT_APPLE_DIR       folder with developerID.p12, p12.password, DeveloperIDG2CA.cer (fetched
#                         from Apple if absent) and AuthKey_<key id>.p8 (App Store Connect API key)
#   SNOUT_NOTARY_ISSUER   that key's issuer ID; without it the build is signed but not notarized
# Without SNOUT_APPLE_DIR it is signed ad hoc: it runs on the Mac that built it, and elsewhere has
# to be allowed once in System Settings > Privacy & Security.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

universal=false
case "${1:-}" in
  --universal) universal=true ;;
  "") ;;
  *) sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac

# shellcheck disable=SC1091
[[ -f "$ROOT/.signing.env" ]] && source "$ROOT/.signing.env"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
OUT="$ROOT/target/release/bundle"
APP="$OUT/snout.app"

if $universal; then
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --release -p snout-gui --target "$t"
  done
  BIN="$(mktemp -d)/snout"
  lipo -create -output "$BIN" target/aarch64-apple-darwin/release/snout target/x86_64-apple-darwin/release/snout
  ARCH=universal
else
  cargo build --release -p snout-gui
  BIN="$ROOT/target/release/snout"
  ARCH="$(uname -m)"
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/snout"
cp assets/icon/snout.icns "$APP/Contents/Resources/snout.icns"
cat >"$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>snout</string>
  <key>CFBundleDisplayName</key><string>snout</string>
  <key>CFBundleIdentifier</key><string>com.snoutdata.git</string>
  <key>CFBundleExecutable</key><string>snout</string>
  <key>CFBundleIconFile</key><string>snout</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>NSHumanReadableCopyright</key><string>Copyright © $(date +%Y) SNOUTDATA INC. MIT License.</string>
</dict>
</plist>
PLIST

IDENTITY=""
if [[ -n "${SNOUT_APPLE_DIR:-}" ]]; then
  # The .p12 holds only the leaf certificate. codesign needs the chain to Apple's root, so put the
  # leaf, its key and the Developer ID G2 intermediate in a keychain of our own, searched while
  # we sign and removed after (the search list is put back as it was).
  [[ -f "$SNOUT_APPLE_DIR/developerID.p12" ]] || { echo "No developerID.p12 in $SNOUT_APPLE_DIR" >&2; exit 1; }
  INTER="$SNOUT_APPLE_DIR/DeveloperIDG2CA.cer"
  [[ -f "$INTER" ]] || curl -fsSL -o "$INTER" https://www.apple.com/certificateauthority/DeveloperIDG2CA.cer
  KC="$HOME/Library/Keychains/snout-git-signing.keychain-db"
  KCPW="snout-git-$$"
  ORIG_KEYCHAINS="$(security list-keychains -d user | sed -e 's/"//g' -e 's/^[[:space:]]*//')"
  cleanup_keychain() {
    # shellcheck disable=SC2086
    security list-keychains -d user -s $ORIG_KEYCHAINS >/dev/null 2>&1 || true
    security delete-keychain "$KC" 2>/dev/null || true
  }
  trap cleanup_keychain EXIT
  security delete-keychain "$KC" 2>/dev/null || true
  security create-keychain -p "$KCPW" "$KC"
  security set-keychain-settings "$KC"
  security unlock-keychain -p "$KCPW" "$KC"
  security import "$INTER" -k "$KC" -A >/dev/null
  security import "$SNOUT_APPLE_DIR/developerID.p12" -k "$KC" \
    -P "$(tr -d '\r\n' <"$SNOUT_APPLE_DIR/p12.password")" -T /usr/bin/codesign >/dev/null
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KCPW" "$KC" >/dev/null
  # shellcheck disable=SC2046,SC2086
  security list-keychains -d user -s "$KC" $ORIG_KEYCHAINS
  IDENTITY="$(security find-identity -v -p codesigning "$KC" | awk '/Developer ID Application/ { print $2; exit }')"
  [[ -n "$IDENTITY" ]] || { echo "No valid Developer ID Application identity in the .p12" >&2; exit 1; }
fi

if [[ -n "$IDENTITY" ]]; then
  codesign --force --options runtime --timestamp --keychain "$KC" --sign "$IDENTITY" "$APP"
else
  codesign --force --sign - "$APP"
fi
codesign --verify --strict "$APP"

DMG="$OUT/snout-$VERSION-$ARCH.dmg"
STAGE="$(mktemp -d)"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -quiet -volname "snout" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"

if [[ -n "$IDENTITY" ]]; then
  codesign --force --timestamp --keychain "$KC" --sign "$IDENTITY" "$DMG"
  KEY="$(ls "$SNOUT_APPLE_DIR"/AuthKey_*.p8 2>/dev/null | head -1)"
  if [[ -n "$KEY" && -n "${SNOUT_NOTARY_ISSUER:-}" ]]; then
    KEY_ID="$(basename "$KEY" .p8)"
    KEY_ID="${KEY_ID#AuthKey_}"
    echo "Notarizing $(basename "$DMG") (a few minutes)…"
    xcrun notarytool submit "$DMG" --key "$KEY" --key-id "$KEY_ID" --issuer "$SNOUT_NOTARY_ISSUER" --wait
    xcrun stapler staple "$DMG"
    spctl --assess --type open --context context:primary-signature -v "$DMG"
  else
    echo "Signed, not notarized: no AuthKey_*.p8 in $SNOUT_APPLE_DIR, or SNOUT_NOTARY_ISSUER unset." >&2
  fi
fi

echo "Built $APP"
echo "Built $DMG"
