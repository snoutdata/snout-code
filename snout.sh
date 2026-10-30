#!/usr/bin/env bash
# Build (if needed) and start SnoutCode.
#
#   ./snout.sh                 start with the repositories you had open last time
#   ./snout.sh <path>          open a folder as the project (Git finds its repository above it)
#   ./snout.sh --pick          choose an existing repository with a folder dialog
#   ./snout.sh --fg [...]      stay in the foreground (logs go to the terminal)
#
# When nothing changed since the last build, the app starts at once, without asking cargo
# (whose check can wait behind another build's lock). Otherwise "Building SnoutCode…" says
# why it waits, and cargo's progress follows. SNOUT_STARTUP_TRACE=1 times the way to the
# first window, here and in the app's log.
#
# The Java engine (engine/build/dist) is built when it is missing or older than its sources,
# beside the app, which starts first and shows the build's progress; with JDK 21:
# SNOUT_JDK21=<path>, else JAVA_HOME if it is 21, else one that /usr/libexec/java_home finds
# (macOS) or one in /usr/lib/jvm (Linux), else Temurin 21 downloaded to the cache
# (~/Library/Caches/SnoutCode on a Mac, ~/.cache/SnoutCode on Linux). If that fails the app
# runs on the previous build, or without Java support.
#
# It runs on macOS and on Linux (GNU or BSD tools alike).
set -euo pipefail

case "$(uname -s)" in
  Darwin) OS=mac ;;
  *) OS=linux ;;
esac
# `stat` of GNU coreutils (Linux) or of BSD (macOS).
if stat --version >/dev/null 2>&1; then GNU_STAT=true; else GNU_STAT=false; fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
[[ "$TARGET" == /* ]] || TARGET="$PWD/$TARGET"
BIN="$TARGET/release/snout-code"
LOG="${TMPDIR:-/tmp}"; LOG="${LOG%/}/snout-code.log"

trace=false
[[ -n "${SNOUT_STARTUP_TRACE:-}" && "${SNOUT_STARTUP_TRACE}" != 0 ]] && trace=true
now_ms() {
  local t
  t="$(date +%s%3N 2>/dev/null)"
  if [[ "$t" =~ ^[0-9]+$ ]]; then echo "$t"; else perl -MTime::HiRes=time -e 'printf "%d", time() * 1000'; fi
}
$trace && started_ms="$(now_ms)"
step() { $trace && echo "snout.sh $(( $(now_ms) - started_ms )) ms  $1" >&2; return 0; }

usage() { sed -n '2,7p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

foreground=false
repo=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --fg) foreground=true ;;
    -p|--pick)
      if [[ $OS == mac ]]; then
        repo="$(osascript -e 'POSIX path of (choose folder with prompt "Open Git repository")' 2>/dev/null)" || repo=""
      elif command -v zenity >/dev/null 2>&1; then
        repo="$(zenity --file-selection --directory --title="Open Git repository" 2>/dev/null)" || repo=""
      elif command -v kdialog >/dev/null 2>&1; then
        repo="$(kdialog --getexistingdirectory "$HOME" --title "Open Git repository" 2>/dev/null)" || repo=""
      else
        echo "No folder dialog (zenity or kdialog): give the folder instead, ./snout.sh <path>." >&2; exit 1
      fi
      [[ -n "$repo" ]] || { echo "No folder chosen." >&2; exit 1; }
      ;;
    -*) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    *) repo="$1" ;;
  esac
  shift
done

if [[ -n "$repo" ]]; then
  [[ -d "$repo" ]] || { echo "Not a folder: $repo" >&2; exit 1; }
  # The folder is the project, as given; Git finds the repository at or above it. Outside
  # a repository, snout offers to initialize one there.
  repo="$(cd "$repo" && pwd)"
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
if [[ $OS == mac ]]; then
  BUILD_JDK_DIR="$HOME/Library/Caches/SnoutCode/build-jdk"
else
  cache="${XDG_CACHE_HOME:-}"; [[ "$cache" == /* ]] || cache="$HOME/.cache"
  BUILD_JDK_DIR="$cache/SnoutCode/build-jdk"
fi
java_major() { "$1/bin/java" -XshowSettings:properties -version 2>&1 | sed -n 's/^ *java.specification.version = //p'; }
is_jdk21() { [[ -n "$1" && -x "$1/bin/java" && -x "$1/bin/jlink" && "$(java_major "$1")" == "21" ]]; }
find_jdk21() {
  local c
  if [[ -n "${SNOUT_JDK21:-}" ]]; then
    is_jdk21 "$SNOUT_JDK21" && { echo "$SNOUT_JDK21"; return 0; }
    echo "SNOUT_JDK21 is not a JDK 21: $SNOUT_JDK21" >&2
    return 1
  fi
  local mac_jdk=""
  [[ -x /usr/libexec/java_home ]] && mac_jdk="$(/usr/libexec/java_home -v 21 2>/dev/null || true)"
  for c in "${JAVA_HOME:-}" "$mac_jdk" /usr/lib/jvm/*21* \
           "$BUILD_JDK_DIR"/*/Contents/Home "$BUILD_JDK_DIR"/*; do
    is_jdk21 "$c" && { echo "$c"; return 0; }
  done
  return 1
}
sha256() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }
# A field of the first asset in Adoptium's answer (`binary.package.link`).
asset_field() {
  if command -v python3 >/dev/null 2>&1; then
    python3 -I -c 'import json, sys; v = json.load(open(sys.argv[1]))[0]
for k in sys.argv[2].split("."): v = v[k]
print(v)' "$1" "$2"
  else
    plutil -extract "0.$2" raw -o - "$1"
  fi
}
# Eclipse Temurin 21 from api.adoptium.net into the build JDK folder, checked against its SHA-256.
download_jdk21() {
  local arch json link sum part
  case "$(uname -m)" in arm64|aarch64) arch=aarch64 ;; *) arch=x64 ;; esac
  mkdir -p "$BUILD_JDK_DIR" || return 1
  json="$BUILD_JDK_DIR/assets.json"; part="$BUILD_JDK_DIR/jdk21.tar.gz.part"
  curl -fsSL --max-time 60 -o "$json" \
    "https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=$arch&image_type=jdk&os=$OS&vendor=eclipse" || return 1
  link="$(asset_field "$json" binary.package.link 2>/dev/null)" || return 1
  sum="$(asset_field "$json" binary.package.checksum 2>/dev/null)" || return 1
  curl -fsSL -o "$part" "$link" || { rm -f "$part"; return 1; }
  [[ "$(sha256 "$part")" == "$sum" ]] || { rm -f "$part"; return 1; }
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
    echo "stage Downloading JDK 21 to build snout-engine"
    download_jdk21 && jdk="$(find_jdk21)" || return 1
  fi
  echo "stage Building snout-engine"
  (cd "$ENGINE" && JAVA_HOME="$jdk" ./gradlew --no-daemon --quiet :sync-cli:snoutDist) >&2
}
# The engine builds beside the app, which starts first: the build writes a line per step to a
# status file (`stage …`, then `done` or `failed …`), which the app shows as a background task;
# its Java servers start once the build is done.
STATUS=""
if engine_stale; then
  STATUS="${TMPDIR:-/tmp}"; STATUS="${STATUS%/}/snout-engine-build.$$.status"
  echo "stage Building snout-engine" >"$STATUS"
fi
build_in_background() {
  local log="${STATUS%.status}.log"
  {
    if build_engine 2>"$log"; then
      echo "done"
    elif [[ -x "$DIST/runtime/bin/java" ]]; then
      echo "failed The engine could not be rebuilt (see $log); the previous build runs."
    else
      echo "failed The engine could not be built (see $log); Java support is off."
    fi
  } >>"$STATUS"
}

step "engine checked"

# The Ghostty terminal engine needs Zig 0.15 (SNOUT_ZIG, zig on PATH, or Homebrew's zig@0.15);
# without one the app builds with the built-in engine only. SNOUT_NO_GHOSTTY=1 skips it.
# A zig found to be 0.15 is remembered (by its file) so later starts need not run it.
ZIG_SEEN="$TARGET/.snout-zig"
file_id() { if $GNU_STAT; then stat -L -c '%i:%Y:%s' "$1"; else stat -L -f '%i:%m:%z' "$1"; fi 2>/dev/null; }
is_zig015() {
  local id
  [[ -n "$1" && -x "$1" ]] || return 1
  id="$(file_id "$1")"
  grep -qxF "$1 $id" "$ZIG_SEEN" 2>/dev/null && return 0
  [[ "$("$1" version 2>/dev/null)" == 0.15.* ]] || return 1
  mkdir -p "$TARGET" && echo "$1 $id" >>"$ZIG_SEEN"
}
features=()
if [[ "${SNOUT_NO_GHOSTTY:-}" != 1 ]]; then
  # Homebrew's keg, where it would be (no `brew` run: it is slow to start).
  for c in "${SNOUT_ZIG:-}" "$(command -v zig || true)" \
           "${HOMEBREW_PREFIX:-/opt/homebrew}/opt/zig@0.15/bin/zig" /usr/local/opt/zig@0.15/bin/zig; do
    if is_zig015 "$c"; then
      export PATH="$(dirname "$c"):$PATH"
      features=(--features ghostty)
      break
    fi
  done
fi
step "zig checked"

# The build is skipped when the binary is the one the last build here made, with the same
# features, and nothing it is built from (the crates, the manifests, the files they include
# from assets/ and engine/) changed since that build began. Otherwise cargo builds; it puts
# the new binary in place as a new file, never writing over the old one, so a SnoutCode
# running from it carries on.
STAMP="$TARGET/release/.snout-code.stamp"
want="features:${features[*]:-none}"
# The binary by its time and size (cargo copies it in again, as a new file, on each build).
bin_id() { if $GNU_STAT; then stat -c '%y:%s' "$BIN"; else stat -f '%Fm:%z' "$BIN"; fi 2>/dev/null; }
built_from() {
  local p
  for p in "$ROOT/crates" "$ROOT/Cargo.toml" "$ROOT/Cargo.lock" "$ROOT/assets" "$ROOT/.cargo" \
           "$ROOT/engine/gradlew" "$ROOT/engine/gradlew.bat" "$ROOT/engine/gradle/wrapper"; do
    [[ -e "$p" ]] && echo "$p"
  done
}
up_to_date() {
  [[ -x "$BIN" && -f "$STAMP" ]] || return 1
  [[ "$(cat "$STAMP")" == "$want $(bin_id)" ]] || return 1
  local paths=()
  while IFS= read -r p; do paths+=("$p"); done < <(built_from)
  [[ -z "$(find "${paths[@]}" -newer "$STAMP" -print -quit 2>/dev/null)" ]]
}
if up_to_date; then
  step "up to date, no build"
else
  echo "Building SnoutCode…"
  mkdir -p "$TARGET/release"
  mark="$STAMP.began"
  touch "$mark"
  cargo build --release -p snout-app "${features[@]+"${features[@]}"}" --manifest-path "$ROOT/Cargo.toml"
  echo "$want $(bin_id)" >"$STAMP"
  touch -r "$mark" "$STAMP"
  rm -f "$mark"
  step "built"
fi

args=()
[[ -n "$repo" ]] && args+=("$repo")
if [[ -n "$STATUS" ]]; then
  export SNOUT_ENGINE_BUILD_STATUS="$STATUS"
  build_in_background &
fi

step "starting the app"
if $foreground; then
  exec "$BIN" "${args[@]+"${args[@]}"}"
else
  nohup "$BIN" "${args[@]+"${args[@]}"}" >"$LOG" 2>&1 &
  echo "SnoutCode started${repo:+ with $repo} (log: $LOG)"
fi
