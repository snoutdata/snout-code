#!/usr/bin/env bash
# Check SnoutCode on Linux, in Docker: clippy, the workspace's tests, and the real app
# started under X11 (Xvfb) and Wayland (sway, headless), with screenshots.
#
#   scripts/test-linux.sh [x86_64|aarch64] [clippy] [tests] [smoke]
#
# The architecture defaults to this machine's (another one runs under emulation, slowly); the
# steps to all three. Screenshots and logs go to target/linux-check/. The build image and the
# Docker volumes are the ones scripts/package-linux.sh uses.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
case "$(uname -m)" in arm64|aarch64) arch=aarch64 ;; *) arch=x86_64 ;; esac
case "${1:-}" in x86_64|aarch64) arch="$1"; shift ;; esac
case "$arch" in x86_64) platform=linux/amd64 ;; *) platform=linux/arm64 ;; esac
tag="snout-linux-build:1-$arch"
OUT="$ROOT/target/linux-check"
mkdir -p "$OUT"

docker build -q --platform "$platform" -t "$tag" -f "$ROOT/scripts/linux/build.Dockerfile" "$ROOT/scripts/linux" >/dev/null
docker run --rm --platform "$platform" --init \
  -v "$ROOT":/src \
  -v snout-linux-cargo:/usr/local/cargo/registry \
  -v "snout-linux-target-$arch":/target \
  -v "snout-linux-gradle-$arch":/root/.gradle \
  -v "snout-linux-zig-$arch":/root/.cache \
  -e CARGO_TARGET_DIR=/target \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  "$tag" \
  bash -c '/src/scripts/linux/check.sh /src /out-tmp "$@"; s=$?; mkdir -p /src/target/linux-check; cp -r /out-tmp/. /src/target/linux-check/ 2>/dev/null; chown -R "$HOST_UID:$HOST_GID" /src/target/linux-check; exit $s' \
  check "$@"
