#!/usr/bin/env bash
# Build SnoutCode.app and a .dmg to install it from, in target/release/bundle/.
#
#   scripts/package-mac.sh
#
# The app is the release snout-code binary with the Java engine in Contents/Resources/engine:
# the engine distribution (snout-engine, jdtls with our bundles, java-debug and a Java 21 runtime)
# without what never runs on a Mac or on the Linux machines of Remote SSH (which run its jars on
# a Java of their own). The Linux builds of snout-server, which Remote SSH installs on the remote
# machine, are in Contents/Resources/server (built with scripts/build-server-linux.sh, so Docker
# is needed). The engine is built afresh beside the bundle, never in
# engine/build/dist, which a running development app may be using. Its runtime is linked from the
# build JDK, so the app is for this Mac's architecture.
#
# The app is built with the Ghostty terminal engine (the `ghostty` feature), which needs Zig 0.15
# (SNOUT_ZIG names its zig, else zig on PATH if it is 0.15, else Homebrew's zig@0.15) and, the first
# time, the network: the build fetches Ghostty's sources. SNOUT_NO_GHOSTTY=1 builds without it.
#
# Building the engine needs JDK 21: SNOUT_JDK21, else JAVA_HOME if it is 21, else one that
# /usr/libexec/java_home finds, else the one ./snout.sh downloads to ~/Library/Caches/SnoutCode.
#
# Signing: with a Developer ID, everything in the app that is code (the binary, the runtime's
# executables and libraries, the native libraries inside jdtls's jars) is signed with the hardened
# runtime; the app is notarized and stapled, then the .dmg is signed, notarized and stapled, so
# both open on any Mac, offline too. Set, in the environment or in .signing.env at the repository
# root (gitignored):
#   SNOUT_APPLE_DIR       folder with developerID.p12, p12.password, DeveloperIDG2CA.cer (fetched
#                         from Apple if absent) and AuthKey_<key id>.p8 (App Store Connect API key)
#   SNOUT_NOTARY_ISSUER   that key's issuer ID; without it the build is signed but not notarized
# Without SNOUT_APPLE_DIR it is signed ad hoc: it runs on the Mac that built it, and elsewhere has
# to be allowed once in System Settings > Privacy & Security.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

case "${1:-}" in
  "") ;;
  *) sed -n '2,24p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac

# shellcheck disable=SC1091
[[ -f "$ROOT/.signing.env" ]] && source "$ROOT/.signing.env"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
ARCH="$(uname -m)"
OUT="$ROOT/target/release/bundle"
APP="$OUT/SnoutCode.app"
ENGINE_DIST="$OUT/engine-dist"
WORK="$(mktemp -d)"

# --- The binary and the engine --------------------------------------------------------------

features=()
if [[ "${SNOUT_NO_GHOSTTY:-}" != 1 ]]; then
  zig_ok() { [[ -n "$1" && -x "$1" && "$("$1" version 2>/dev/null)" == 0.15.* ]]; }
  ZIG=""
  for c in "${SNOUT_ZIG:-}" "$(command -v zig || true)" "$(brew --prefix zig@0.15 2>/dev/null || true)/bin/zig"; do
    if zig_ok "$c"; then ZIG="$c"; break; fi
  done
  [[ -n "$ZIG" ]] || { echo "No Zig 0.15 for the Ghostty terminal engine: brew install zig@0.15, set SNOUT_ZIG, or SNOUT_NO_GHOSTTY=1." >&2; exit 1; }
  export PATH="$(dirname "$ZIG"):$PATH"
  features=(--features ghostty)
fi
cargo build --release -p snout-app ${features[@]+"${features[@]}"}
BIN="${CARGO_TARGET_DIR:-$ROOT/target}/release/snout-code"

java_major() { "$1/bin/java" -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java.specification.version = //p'; }
is_jdk21() { [[ -n "$1" && -x "$1/bin/java" && -x "$1/bin/jlink" && -d "$1/jmods" && "$(java_major "$1")" == "21" ]]; }
JDK=""
if [[ -n "${SNOUT_JDK21:-}" ]]; then
  is_jdk21 "$SNOUT_JDK21" || { echo "SNOUT_JDK21 is not a full JDK 21: $SNOUT_JDK21" >&2; exit 1; }
  JDK="$SNOUT_JDK21"
else
  for c in "${JAVA_HOME:-}" "$(/usr/libexec/java_home -v 21 2>/dev/null || true)" \
           "$HOME"/Library/Caches/SnoutCode/build-jdk/*/Contents/Home; do
    if is_jdk21 "$c"; then JDK="$c"; break; fi
  done
fi
[[ -n "$JDK" ]] || { echo "No JDK 21 to build the engine with: set SNOUT_JDK21, or run ./snout.sh once." >&2; exit 1; }

(cd engine && JAVA_HOME="$JDK" ./gradlew --no-daemon --quiet :sync-cli:snoutDist -PsnoutDistDir="$ENGINE_DIST")

# snout-server for Linux, x86_64 and aarch64 (static musl builds, made in Docker).
"$ROOT/scripts/build-server-linux.sh"
SERVER_DIST="$ROOT/target/snout-server-dist"

# --- The bundle -----------------------------------------------------------------------------

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/snout-code"
cp assets/icon/snout.icns "$APP/Contents/Resources/snout.icns"
# The third-party licences: Lucide, Inter, and Ghostty's terminal engine and what it includes.
cp NOTICE LICENSE "$APP/Contents/Resources/"

# What never runs here: jdtls's launch scripts, its feature archives (Equinox runs without
# them; one carries Eclipse's own native launchers), the configurations for Windows and for
# jdtls's syntax server, and the native launcher fragments for Linux and Windows. The Linux
# configurations stay: a remote machine runs the engine's jars with them. The
# runtime stays whole: it is also the JDK that Gradle, run configurations and the debugger fall
# back to when the machine has none, and jdtls's default system library.
ENGINE="$APP/Contents/Resources/engine"
rsync -a \
  --exclude '/jdtls/bin/' \
  --exclude '/jdtls/features/' \
  --exclude '/jdtls/config_win/' --exclude '/jdtls/config_ss_*/' \
  --exclude '/jdtls/plugins/org.eclipse.equinox.launcher.gtk.*' \
  --exclude '/jdtls/plugins/org.eclipse.equinox.launcher.win32.*' \
  "$ENGINE_DIST/" "$ENGINE/"
cp engine/NOTICE "$ENGINE/NOTICE"
mkdir -p "$APP/Contents/Resources/server"
cp "$SERVER_DIST"/snout-server-linux-x86_64 "$SERVER_DIST"/snout-server-linux-aarch64 "$SERVER_DIST"/SHA256SUMS \
  "$APP/Contents/Resources/server/"
cp LICENSE "$APP/Contents/Resources/LICENSE"

cat >"$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>SnoutCode</string>
  <key>CFBundleDisplayName</key><string>SnoutCode</string>
  <key>CFBundleIdentifier</key><string>com.snoutdata.code</string>
  <key>CFBundleExecutable</key><string>snout-code</string>
  <key>CFBundleIconFile</key><string>snout</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
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
plutil -lint -s "$APP/Contents/Info.plist"
xattr -cr "$APP"

# --- Signing --------------------------------------------------------------------------------

IDENTITY=""
KC=""
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT
if [[ -n "${SNOUT_APPLE_DIR:-}" ]]; then
  # The .p12 holds only the leaf certificate. codesign needs the chain to Apple's root, so put the
  # leaf, its key and the Developer ID G2 intermediate in a keychain of our own, searched while
  # we sign and removed after (the search list is put back as it was).
  [[ -f "$SNOUT_APPLE_DIR/developerID.p12" ]] || { echo "No developerID.p12 in $SNOUT_APPLE_DIR" >&2; exit 1; }
  INTER="$SNOUT_APPLE_DIR/DeveloperIDG2CA.cer"
  [[ -f "$INTER" ]] || curl -fsSL -o "$INTER" https://www.apple.com/certificateauthority/DeveloperIDG2CA.cer
  KC="$HOME/Library/Keychains/snout-code-signing.keychain-db"
  KCPW="snout-code-$$"
  ORIG_KEYCHAINS="$(security list-keychains -d user | sed -e 's/"//g' -e 's/^[[:space:]]*//')"
  cleanup() {
    # shellcheck disable=SC2086
    security list-keychains -d user -s $ORIG_KEYCHAINS >/dev/null 2>&1 || true
    security delete-keychain "$KC" 2>/dev/null || true
    rm -rf "$WORK"
  }
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

# What a JVM needs under the hardened runtime: its JIT writes and runs code, and the runtime
# loads native libraries signed by others or not at all (Gradle's native integration in the
# daemon, the JNI libraries of the programs it runs and debugs, JNA's library unpacked by jdtls).
JVM_ENTITLEMENTS="$WORK/jvm.entitlements"
cat >"$JVM_ENTITLEMENTS" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.security.cs.allow-jit</key><true/>
  <key>com.apple.security.cs.allow-unsigned-executable-memory</key><true/>
  <key>com.apple.security.cs.disable-library-validation</key><true/>
</dict>
</plist>
PLIST

# sign [--entitlements <file>] <path>...: hardened runtime, timestamped; ad hoc without an identity.
sign() {
  local args=(--force --options runtime)
  if [[ "${1:-}" == "--entitlements" ]]; then args+=(--entitlements "$2"); shift 2; fi
  if [[ -n "$IDENTITY" ]]; then
    args+=(--timestamp --keychain "$KC" --sign "$IDENTITY")
  else
    args+=(--sign -)
  fi
  codesign "${args[@]}" "$@"
}
is_macho() { file -b "$1" | grep -q '^Mach-O'; }

# Native code inside jars, and inside the jars in jars, is code to notarization too: unpack it,
# sign it, put it back. A jar signed by Eclipse loses its jar signature, which would no longer
# match (Equinox does not check them). sign_jar sets JAR_CHANGED.
sign_jar() {
  local jar="$1" x f rel entries=()
  x="$(mktemp -d "$WORK/jar.XXXXXX")"
  unzip -qq "$jar" -x '*.class' -d "$x" || [[ $? -eq 11 ]]
  while IFS= read -r -d '' f; do
    rel="${f#"$x"/}"
    if [[ "$f" == *.jar ]]; then
      sign_jar "$f"
      if $JAR_CHANGED; then entries+=("$rel"); fi
    elif is_macho "$f"; then
      (cd "$x" && sign "$rel")
      entries+=("$rel")
    fi
  done < <(find "$x" -type f -print0)
  JAR_CHANGED=false
  if [[ ${#entries[@]} -gt 0 ]]; then
    (cd "$x" && zip -q -X "$jar" "${entries[@]}")
    zip -q -d "$jar" 'META-INF/*.SF' 'META-INF/*.RSA' 'META-INF/*.DSA' 'META-INF/*.EC' >/dev/null 2>&1 || true
    echo "Signed in $(basename "$jar"): ${entries[*]}"
    JAR_CHANGED=true
  fi
  rm -rf "$x"
}
while IFS= read -r -d '' jar; do
  sign_jar "$jar"
done < <(find "$ENGINE" -name '*.jar' -print0)

# The runtime's libraries, then its executables with the JVM's entitlements.
libs=()
exes=()
while IFS= read -r -d '' f; do
  if is_macho "$f"; then
    if file -b "$f" | grep -q 'executable'; then exes+=("$f"); else libs+=("$f"); fi
  fi
done < <(find "$ENGINE" -type f ! -name '*.jar' -print0)
[[ ${#libs[@]} -gt 0 ]] && sign "${libs[@]}"
[[ ${#exes[@]} -gt 0 ]] && sign --entitlements "$JVM_ENTITLEMENTS" "${exes[@]}"
echo "Signed ${#libs[@]} libraries and ${#exes[@]} executables in the engine."

sign "$APP/Contents/MacOS/snout-code"
sign "$APP"
codesign --verify --deep --strict "$APP"
"$ENGINE/runtime/bin/java" -version >/dev/null 2>&1 || { echo "The signed runtime does not start." >&2; exit 1; }

# notarize <file>: submits it and waits; fails with Apple's log unless it is accepted.
notarize() {
  local result id status
  result="$WORK/notary.json"
  xcrun notarytool submit "$1" --key "$KEY" --key-id "$KEY_ID" --issuer "$SNOUT_NOTARY_ISSUER" \
    --wait --output-format json >"$result"
  id="$(plutil -extract id raw -o - "$result")"
  status="$(plutil -extract status raw -o - "$result")"
  if [[ "$status" != "Accepted" ]]; then
    echo "Notarization of $(basename "$1") ended $status:" >&2
    xcrun notarytool log "$id" --key "$KEY" --key-id "$KEY_ID" --issuer "$SNOUT_NOTARY_ISSUER" >&2 || true
    exit 1
  fi
}

NOTARIZE=false
if [[ -n "$IDENTITY" ]]; then
  KEY="$(ls "$SNOUT_APPLE_DIR"/AuthKey_*.p8 2>/dev/null | head -1)"
  if [[ -n "$KEY" && -n "${SNOUT_NOTARY_ISSUER:-}" ]]; then
    NOTARIZE=true
    KEY_ID="$(basename "$KEY" .p8)"
    KEY_ID="${KEY_ID#AuthKey_}"
  else
    echo "Signed, not notarized: no AuthKey_*.p8 in $SNOUT_APPLE_DIR, or SNOUT_NOTARY_ISSUER unset." >&2
  fi
fi

if $NOTARIZE; then
  echo "Notarizing SnoutCode.app (a few minutes)…"
  ditto -c -k --keepParent "$APP" "$WORK/SnoutCode.zip"
  notarize "$WORK/SnoutCode.zip"
  xcrun stapler staple -q "$APP"
  spctl --assess --type execute -vv "$APP"
fi

# --- The disk image -------------------------------------------------------------------------

DMG="$OUT/SnoutCode-$VERSION-$ARCH.dmg"
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/SnoutCode.app"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -quiet -volname "SnoutCode" -srcfolder "$STAGE" -ov -format UDZO "$DMG"

if [[ -n "$IDENTITY" ]]; then
  codesign --force --timestamp --keychain "$KC" --sign "$IDENTITY" "$DMG"
  if $NOTARIZE; then
    echo "Notarizing $(basename "$DMG") (a few minutes)…"
    notarize "$DMG"
    xcrun stapler staple -q "$DMG"
    spctl --assess --type open --context context:primary-signature -vv "$DMG"
  fi
fi

echo "Built $APP ($(du -sh "$APP" | cut -f1))"
echo "Built $DMG ($(du -sh "$DMG" | cut -f1))"
