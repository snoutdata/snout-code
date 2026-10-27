#!/usr/bin/env bash
# Build (if needed) and start SnoutGIT.
#
#   ./snout.sh                 start with the repositories you had open last time
#   ./snout.sh <path>          open a repository (any folder inside it), or a folder to search/init
#   ./snout.sh --pick          choose an existing repository with a folder dialog
#   ./snout.sh --fg [...]      stay in the foreground (logs go to the terminal)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$ROOT/target/release/snout"
LOG="${TMPDIR:-/tmp}"; LOG="${LOG%/}/snout.log"

usage() { sed -n '2,7p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

foreground=false
repo=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --fg) foreground=true ;;
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

# Incremental: a no-op when nothing changed.
cargo build --release --quiet -p snout-gui --manifest-path "$ROOT/Cargo.toml"

args=()
[[ -n "$repo" ]] && args+=("$repo")

if $foreground; then
  exec "$BIN" "${args[@]+"${args[@]}"}"
else
  nohup "$BIN" "${args[@]+"${args[@]}"}" >"$LOG" 2>&1 &
  echo "SnoutGIT started${repo:+ with $repo} (log: $LOG)"
fi
