#!/usr/bin/env bash
# Build snout-server for Linux: static musl binaries for x86_64 and aarch64, which run on
# any Linux distribution. The app uploads the one matching the remote machine.
#
#   target/snout-server-dist/snout-server-linux-x86_64
#   target/snout-server-dist/snout-server-linux-aarch64
#   target/snout-server-dist/SHA256SUMS
#
# Needs Docker. The build runs in an image with Rust, zig and cargo-zigbuild
# (scripts/remote/build.Dockerfile); the crates it downloads stay in a Docker volume
# between runs, and its build folder is target/server-linux.
#
#   scripts/build-server-linux.sh            both
#   scripts/build-server-linux.sh aarch64    one
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="snout-server-build:2"
ARCHES=("${@:-x86_64 aarch64}")
# shellcheck disable=SC2206
ARCHES=(${ARCHES[*]})
OUT="$ROOT/target/snout-server-dist"

command -v docker >/dev/null || { echo "docker not found" >&2; exit 1; }
if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  docker build -t "$IMAGE" -f "$ROOT/scripts/remote/build.Dockerfile" "$ROOT/scripts/remote"
fi

TARGETS=()
for arch in "${ARCHES[@]}"; do
  case "$arch" in
    x86_64|aarch64) TARGETS+=(--target "$arch-unknown-linux-musl") ;;
    *) echo "unknown architecture: $arch (x86_64 or aarch64)" >&2; exit 1 ;;
  esac
done

docker run --rm \
  -v "$ROOT":/src \
  -v snout-server-build-cargo:/usr/local/cargo/registry \
  -w /src \
  -e CARGO_TARGET_DIR=/src/target/server-linux \
  -e CARGO_PROFILE_RELEASE_STRIP=true \
  -e CARGO_PROFILE_RELEASE_LTO=thin \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  "$IMAGE" \
  sh -c 'cargo zigbuild --locked --release -p snout-server "$@" && chown -R "$HOST_UID:$HOST_GID" /src/target/server-linux' \
  build "${TARGETS[@]}"

mkdir -p "$OUT"
for arch in "${ARCHES[@]}"; do
  cp "$ROOT/target/server-linux/$arch-unknown-linux-musl/release/snout-server" "$OUT/snout-server-linux-$arch"
done
(cd "$OUT" && shasum -a 256 snout-server-linux-* > SHA256SUMS && cat SHA256SUMS)
