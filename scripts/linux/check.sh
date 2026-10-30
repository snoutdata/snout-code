#!/usr/bin/env bash
# The Linux checks, inside the build container (scripts/test-linux.sh runs it): clippy, the
# workspace's tests on a Linux build of the Java engine, and the real app started under Xvfb
# (X11) and sway (Wayland). Screenshots and logs go to <out>.
#
#   check.sh <src> <out> [clippy] [tests] [smoke]   (all three by default)
set -euo pipefail

SRC="$1"
OUT="$2"
shift 2
STEPS="${*:-clippy tests smoke}"
ARCH="$(uname -m)"
mkdir -p "$OUT"
git config --global user.email test@example.com
git config --global user.name Test
git config --global --add safe.directory '*'

# The Java engine for Linux, built from a copy (the tree's engine/build may be a Mac's).
WORK="/work/$ARCH"
mkdir -p "$WORK"
rsync -a --delete --exclude build/ --exclude .gradle/ "$SRC/engine/" "$WORK/engine-src/"
echo "== Building the Java engine ($ARCH)"
(cd "$WORK/engine-src" && ./gradlew --no-daemon --quiet :sync-cli:snoutDist -PsnoutDistDir="$WORK/engine-dist")
export SNOUT_ENGINE_DIST="$WORK/engine-dist"

cd "$SRC"
for step in $STEPS; do
  case "$step" in
    clippy)
      echo "== clippy ($ARCH)"
      cargo clippy --locked --workspace --all-targets -- -D warnings
      cargo clippy --locked --workspace --all-targets --features snout-app/ghostty -- -D warnings
      ;;
    tests)
      echo "== tests ($ARCH)"
      # As on a machine with no Java installed: the tests that need a JDK for their builds get
      # the image's (SNOUT_TEST_JDK); the app finds none of its own.
      PATH="${PATH//\/opt\/jdk21\/bin:/}" SNOUT_TEST_JDK=/opt/jdk21 JAVA_HOME= \
        cargo test --locked --workspace --no-fail-fast 2>&1 | tee "$OUT/tests-$ARCH.log"
      [[ "${PIPESTATUS[0]}" == 0 ]]
      ;;
    smoke)
      echo "== the app under X11 and Wayland ($ARCH)"
      cargo build --locked --release -p snout-app --features ghostty
      BIN="${CARGO_TARGET_DIR:-$SRC/target}/release/snout-code"
      "$SRC/scripts/linux/smoke.sh" --x11 --bin "$BIN" --out "$OUT/$ARCH"
      "$SRC/scripts/linux/smoke.sh" --x11 --bin "$BIN" --out "$OUT/$ARCH" --jdk /opt/jdk21
      "$SRC/scripts/linux/smoke.sh" --wayland --bin "$BIN" --out "$OUT/$ARCH" --jdk /opt/jdk21
      ;;
    *) echo "unknown step: $step" >&2; exit 2 ;;
  esac
done
