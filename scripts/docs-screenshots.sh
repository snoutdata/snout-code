#!/bin/bash
# Retake every screenshot of the user guide (docs/images) from the real app.
#
#   scripts/docs-screenshots.sh [parts]
#
# It makes the demo projects (scripts/docs-screenshots/make-demos.sh) in a scratch folder,
# renders the app headless on them with the real Java engine (the env-gated docs part of
# crates/snout-app/tests/screenshots.rs, see tests/screenshots/docs.rs), connects to a Linux
# container with sshd for Remote SSH (crates/snout-server/tests/docker, removed afterwards),
# then crops, resizes (1600 px wide at most) and optimises the shots into docs/images.
#
# `parts` takes only some of them, separated by commas: ledger, java, javatools, debug, welcome,
# wizard, settings, structure, git, blame, claude, palette, remote. The images of parts not taken stay.
#
# Needs: Docker (for remote), the network the first time (Gradle, Spring Boot, a JDK 21 and
# Pillow), and a JDK 21 (SNOUT_JDK21, JAVA_HOME, an installed one, or the one snout.sh
# downloads). The app runs with HOME set to the scratch folder, so paths read as
# ~/projects/shop; your own settings and Gradle caches are only read. Environment:
#   SNOUT_DOCS_SCRATCH  the scratch folder (default $TMPDIR/snoutcode-docs)
#   SNOUT_DOCS_KEEP=1   keep the raw shots and the scratch home afterwards
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HERE="$ROOT/scripts/docs-screenshots"
TMP="${TMPDIR:-/tmp}"
TMP="${TMP%/}"
SCRATCH="${SNOUT_DOCS_SCRATCH:-$TMP/snoutcode-docs}"
PARTS="${1:-ledger,java,javatools,debug,welcome,wizard,settings,structure,git,blame,claude,palette,remote}"
REAL_HOME="$HOME"
DOCS_HOME="$SCRATCH/home"
RAW="$SCRATCH/raw"
# ssh's control sockets need a short path.
SSH_DIR="$TMP/snd-$$"

say() { printf '\033[1m%s\033[0m\n' "$*"; }

# A JDK 21: for the engine's build, Gradle and the demo projects.
jdk=""
for c in "${SNOUT_JDK21:-}" "${JAVA_HOME:-}" "$(/usr/libexec/java_home -v 21 2>/dev/null || true)" \
         "$REAL_HOME"/Library/Caches/SnoutCode/build-jdk/*/Contents/Home; do
  if [[ -n "$c" && -x "$c/bin/java" ]] && "$c/bin/java" -version 2>&1 | grep -q 'version "21'; then
    jdk="$c"; break
  fi
done
[[ -n "$jdk" ]] || { echo "No JDK 21: set SNOUT_JDK21, or run ./snout.sh once to download one." >&2; exit 1; }

# The engine and, for Remote SSH, the Linux server builds.
if [[ ! -x "$ROOT/engine/build/dist/runtime/bin/java" ]]; then
  say "Building the Java engine"
  (cd "$ROOT/engine" && JAVA_HOME="$jdk" ./gradlew --no-daemon --quiet :sync-cli:snoutDist)
fi
if [[ ",$PARTS," == *",remote,"* ]] && ! ls "$ROOT"/target/snout-server-dist/snout-server-linux-* >/dev/null 2>&1; then
  say "Building the Linux server"
  "$ROOT/scripts/build-server-linux.sh"
fi

# The scratch home: the demo projects' pristine copies, a JDK where JDKs are found, Gradle.
say "Making the demo projects in $SCRATCH"
mkdir -p "$SCRATCH" "$DOCS_HOME/Library/Java/JavaVirtualMachines" "$DOCS_HOME/.gradle/wrapper/dists" "$RAW"
bash "$HERE/make-demos.sh" "$SCRATCH/demos" >/dev/null
jdk_bundle="$DOCS_HOME/Library/Java/JavaVirtualMachines/temurin-21.jdk"
if [[ ! -x "$jdk_bundle/Contents/Home/bin/java" ]]; then
  mkdir -p "$jdk_bundle/Contents"
  cp -cR "$jdk/" "$jdk_bundle/Contents/Home" 2>/dev/null || cp -R "$jdk/" "$jdk_bundle/Contents/Home"
fi
for d in "$REAL_HOME"/.gradle/wrapper/dists/gradle-8.10.2-bin; do
  [[ -d "$d" && ! -d "$DOCS_HOME/.gradle/wrapper/dists/$(basename "$d")" ]] && cp -cR "$d" "$DOCS_HOME/.gradle/wrapper/dists/"
done
ro_cache=()
[[ -d "$REAL_HOME/.gradle/caches/modules-2" ]] && ro_cache=(GRADLE_RO_DEP_CACHE="$REAL_HOME/.gradle/caches")

# Pillow, for the crops.
if [[ ! -x "$SCRATCH/venv/bin/python" ]]; then
  python3 -m venv "$SCRATCH/venv"
  "$SCRATCH/venv/bin/pip" install --quiet --disable-pip-version-check pillow
fi

cleanup() {
  # What the run may have left: Gradle daemons of the scratch home, the container, ssh.
  pkill -f "GradleDaemon.*$DOCS_HOME" 2>/dev/null || true
  pkill -f "$DOCS_HOME/.gradle" 2>/dev/null || true
  docker ps -q --filter "name=snout-docs-" 2>/dev/null | xargs docker rm -f >/dev/null 2>&1 || true
  pkill -f "$SSH_DIR" 2>/dev/null || true
  rm -rf "$SSH_DIR"
}
trap cleanup EXIT

# The Ghostty terminal engine, as release builds have it, when Zig 0.15 is here (else the
# Terminal settings page shows Ghostty as unavailable).
features=()
for z in "${SNOUT_ZIG:-}" "$(command -v zig || true)" "$(brew --prefix zig@0.15 2>/dev/null || true)/bin/zig"; do
  if [[ -n "$z" && -x "$z" && "$("$z" version 2>/dev/null)" == 0.15.* ]]; then
    export PATH="$(dirname "$z"):$PATH"
    features=(--features ghostty)
    break
  fi
done

say "Taking the shots: $PARTS"
rm -f "$RAW"/*.png
(cd "$ROOT/crates/snout-app" && env HOME="$DOCS_HOME" GRADLE_USER_HOME="$DOCS_HOME/.gradle" "${ro_cache[@]}" \
  DOCKER_CONFIG="${DOCKER_CONFIG:-$REAL_HOME/.docker}" CARGO_HOME="${CARGO_HOME:-$REAL_HOME/.cargo}" \
  RUSTUP_HOME="${RUSTUP_HOME:-$REAL_HOME/.rustup}" \
  SNOUT_DOCS_SHOTS="$SCRATCH/demos" SNOUT_SCREENSHOT="$RAW" SNOUT_DOCS_ONLY="$PARTS" \
  SNOUT_DOCS_JDK="$jdk_bundle/Contents/Home" SNOUT_DOCS_SSH_DIR="$SSH_DIR" \
  cargo test -p snout-app ${features[@]+"${features[@]}"} --test screenshots -- --nocapture 2>&1 | grep -E '^docs|panicked|error' || true)

say "Writing docs/images"
"$SCRATCH/venv/bin/python" -I "$HERE/process.py" "$RAW" "$ROOT/docs/images"

if [[ "${SNOUT_DOCS_KEEP:-}" != 1 ]]; then
  rm -rf "$DOCS_HOME/projects" "$SCRATCH/demos"
fi
say "Done: docs/images (raw shots in $RAW)"
