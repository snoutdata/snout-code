#!/usr/bin/env bash
# Build (if needed) and start SnoutCode.
#
#   ./snout.sh                 start with the repositories you had open last time
#   ./snout.sh <path>          open a repository (any folder inside it), or a folder to search/init
#   ./snout.sh --pick          choose an existing repository with a folder dialog
#   ./snout.sh --fg [...]      stay in the foreground (logs go to the terminal)
#   ./snout.sh --old [...]     start the earlier Git-only app instead
#
# The Java engine (engine/build/dist) is built first when it is missing or older than its
# sources, with JDK 21: SNOUT_JDK21=<path>, else JAVA_HOME if it is 21, else one that
# /usr/libexec/java_home finds, else Temurin 21 downloaded to ~/Library/Caches/SnoutCode.
# If that fails the app starts anyway, without Java support.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PKG="snout-app"
BIN="$ROOT/target/release/snout-code"
LOG="${TMPDIR:-/tmp}"; LOG="${LOG%/}/snout-code.log"

usage() { sed -n '2,8p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

foreground=false
repo=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --fg) foreground=true ;;
    --old) PKG="snout-gui"; BIN="$ROOT/target/release/snout"; LOG="${LOG%/*}/snout.log" ;;
    -p|--pick)
      repo="$(osascript -e 'POSIX path of (choose folder with prompt "Open Git repository")' 2>/dev/null)" \
        || { echo "No folder chosen." >&2; exit 1; }
      ;;
    -*) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    *) repo="$1" ;;
  esac
  shift
done

if [[ -n "$repo" ]]; then
  [[ -d "$repo" ]] || { echo "Not a folder: $repo" >&2; exit 1; }
  # Inside a repository: open its root. Otherwise snout offers to search the folder
  # for repositories or to initialize one there.
  repo="$(git -C "$repo" rev-parse --show-toplevel 2>/dev/null || (cd "$repo" && pwd))"
fi

# Make cargo available even in shells that haven't sourced rustup's env yet.
if ! command -v cargo >/dev/null 2>&1 && [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
command -v cargo >/dev/null 2>&1 || { echo "cargo not found; install Rust from https://rustup.rs" >&2; exit 1; }

# The engine distribution (Java support): built when missing or stale. Java is never
# required to start the app: without the engine it runs without Java support.
ENGINE="$ROOT/engine"
DIST="$ENGINE/build/dist"
BUILD_JDK_DIR="$HOME/Library/Caches/SnoutCode/build-jdk"
java_major() { "$1/bin/java" -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java.specification.version = //p'; }
is_jdk21() { [[ -n "$1" && -x "$1/bin/java" && -x "$1/bin/jlink" && "$(java_major "$1")" == "21" ]]; }
find_jdk21() {
  local c
  if [[ -n "${SNOUT_JDK21:-}" ]]; then
    is_jdk21 "$SNOUT_JDK21" && { echo "$SNOUT_JDK21"; return 0; }
    echo "SNOUT_JDK21 is not a JDK 21: $SNOUT_JDK21" >&2
    return 1
  fi
  for c in "${JAVA_HOME:-}" "$(/usr/libexec/java_home -v 21 2>/dev/null || true)" \
           "$BUILD_JDK_DIR"/*/Contents/Home; do
    is_jdk21 "$c" && { echo "$c"; return 0; }
  done
  return 1
}
# Eclipse Temurin 21 from api.adoptium.net into the build JDK folder, checked against its SHA-256.
download_jdk21() {
  local arch json link sum part
  case "$(uname -m)" in arm64|aarch64) arch=aarch64 ;; *) arch=x64 ;; esac
  mkdir -p "$BUILD_JDK_DIR" || return 1
  json="$BUILD_JDK_DIR/assets.json"; part="$BUILD_JDK_DIR/jdk21.tar.gz.part"
  curl -fsSL --max-time 60 -o "$json" \
    "https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=$arch&image_type=jdk&os=mac&vendor=eclipse" || return 1
  link="$(plutil -extract 0.binary.package.link raw -o - "$json" 2>/dev/null)" || return 1
  sum="$(plutil -extract 0.binary.package.checksum raw -o - "$json" 2>/dev/null)" || return 1
  curl -fsSL -o "$part" "$link" || { rm -f "$part"; return 1; }
  [[ "$(shasum -a 256 "$part" | cut -d' ' -f1)" == "$sum" ]] || { rm -f "$part"; return 1; }
  tar -xzf "$part" -C "$BUILD_JDK_DIR" || { rm -f "$part"; return 1; }
  rm -f "$part" "$json"
}
engine_stale() {
  [[ -x "$DIST/runtime/bin/java" && -f "$DIST/dist.properties" ]] || return 0
  [[ -n "$(find "$ENGINE" -path "$ENGINE/build" -prune -o -path '*/build' -prune -o -path '*/.gradle' -prune \
    -o -type f \( -path '*/src/main/*' -o -name '*.gradle' -o -name 'gradle-wrapper.properties' \) \
    -newer "$DIST/dist.properties" -print 2>/dev/null | head -1)" ]]
}
build_engine() {
  local jdk
  if ! jdk="$(find_jdk21)"; then
    [[ -n "${SNOUT_JDK21:-}" ]] && return 1
    echo "Downloading JDK 21 (Eclipse Temurin) to build the Java engine…"
    download_jdk21 && jdk="$(find_jdk21)" || return 1
  fi
  echo "Building the Java engine…"
  (cd "$ENGINE" && JAVA_HOME="$jdk" ./gradlew --no-daemon --quiet :sync-cli:snoutDist) >&2
}
if [[ "$PKG" == "snout-app" ]] && engine_stale; then
  if ! build_engine; then
    if [[ -x "$DIST/runtime/bin/java" ]]; then
      echo "Warning: the Java engine could not be rebuilt; starting with the previous build." >&2
    else
      echo "Warning: the Java engine could not be built; starting without Java support." >&2
    fi
  fi
fi

# Incremental: a no-op when nothing changed.
cargo build --release --quiet -p "$PKG" --manifest-path "$ROOT/Cargo.toml"

args=()
[[ -n "$repo" ]] && args+=("$repo")

if $foreground; then
  exec "$BIN" "${args[@]+"${args[@]}"}"
else
  nohup "$BIN" "${args[@]+"${args[@]}"}" >"$LOG" 2>&1 &
  echo "SnoutCode started${repo:+ with $repo} (log: $LOG)"
fi
