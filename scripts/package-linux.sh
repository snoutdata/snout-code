#!/usr/bin/env bash
# Build SnoutCode's Linux packages, a .deb and an Arch Linux package, in Docker.
#
#   scripts/package-linux.sh                 this machine's architecture
#   scripts/package-linux.sh x86_64 aarch64  those (`all` for both)
#
# Writes to target/linux-packages/:
#   snout-code_<version>_<amd64|arm64>.deb              for Debian 12, Ubuntu 22.04 and later
#   snout-code-<version>-1-<x86_64|aarch64>.pkg.tar.zst for Arch Linux
#   arch/PKGBUILD                                       the Arch package's recipe
#   snout-code-<version>-linux-<arch>.tar.zst           the installed tree both are made of
#   SHA256SUMS
#
# The app is built in an Ubuntu 22.04 image (scripts/linux/build.Dockerfile), so it needs no
# glibc newer than 2.35, with the Ghostty terminal engine (SNOUT_NO_GHOSTTY=1 leaves it out).
# Its Java engine is bundled with a Java 21 runtime jlinked from Temurin for the same
# architecture, so Java is never needed on the user's machine; the Linux servers for Remote SSH
# come from scripts/build-server-linux.sh. An architecture other than this machine's builds
# under emulation (slowly). The Arch package is made with makepkg in the archlinux image (which
# exists for x86_64 only; it packages the aarch64 tree too, as the files are only copied).
#
# Docker keeps, between runs: the images snout-linux-build:1-<arch>, and the volumes
# snout-linux-cargo (crates), snout-linux-target-<arch> (the build), snout-linux-gradle-<arch>
# and snout-linux-zig-<arch> (Gradle's and Zig's caches). `docker volume rm` them to start over.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/target/linux-packages"
IMAGE="snout-linux-build:1"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"

host_arch() { case "$(uname -m)" in arm64|aarch64) echo aarch64 ;; *) echo x86_64 ;; esac; }
ARCHES=()
for a in "${@:-$(host_arch)}"; do
  case "$a" in
    all) ARCHES+=(x86_64 aarch64) ;;
    x86_64|aarch64) ARCHES+=("$a") ;;
    -h|--help) sed -n '2,25p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown architecture: $a (x86_64, aarch64 or all)" >&2; exit 2 ;;
  esac
done

command -v docker >/dev/null || { echo "docker not found" >&2; exit 1; }
docker_platform() { case "$1" in x86_64) echo linux/amd64 ;; aarch64) echo linux/arm64 ;; esac; }

mkdir -p "$OUT"

# The servers Remote SSH installs, for both architectures (static musl builds).
"$ROOT/scripts/build-server-linux.sh"

for arch in "${ARCHES[@]}"; do
  platform="$(docker_platform "$arch")"
  tag="$IMAGE-$arch"
  echo "== The build image for $arch"
  docker build -q --platform "$platform" -t "$tag" -f "$ROOT/scripts/linux/build.Dockerfile" "$ROOT/scripts/linux" >/dev/null
  echo "== Building for $arch"
  docker run --rm --platform "$platform" \
    -v "$ROOT":/src:ro \
    -v "$OUT":/out \
    -v snout-linux-cargo:/usr/local/cargo/registry \
    -v "snout-linux-target-$arch":/target \
    -v "snout-linux-gradle-$arch":/root/.gradle \
    -v "snout-linux-zig-$arch":/root/.cache \
    -e CARGO_TARGET_DIR=/target \
    -e SNOUT_NO_GHOSTTY="${SNOUT_NO_GHOSTTY:-}" \
    -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
    "$tag" \
    bash -c '/src/scripts/linux/build-package.sh /src /out /src/target/snout-server-dist; s=$?; chown -R "$HOST_UID:$HOST_GID" /out; exit $s'
done

# --- The Arch Linux package ----------------------------------------------------------------

sha_of() {
  local f="$OUT/snout-code-$VERSION-linux-$1.tar.zst"
  if [[ -f "$f" ]]; then
    if command -v sha256sum >/dev/null; then sha256sum "$f"; else shasum -a 256 "$f"; fi | cut -d' ' -f1
  else
    echo SKIP
  fi
}
mkdir -p "$OUT/arch"
sed -e "s/@VERSION@/$VERSION/" -e "s/@SHA256_X86_64@/$(sha_of x86_64)/" -e "s/@SHA256_AARCH64@/$(sha_of aarch64)/" \
  "$ROOT/scripts/linux/PKGBUILD.in" >"$OUT/arch/PKGBUILD"

echo "== The Arch Linux package"
docker run --rm --platform linux/amd64 \
  -v "$OUT":/out \
  -v "$ROOT/scripts/linux":/scripts:ro \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -e ARCHES="${ARCHES[*]}" -e VERSION="$VERSION" \
  archlinux:latest bash /scripts/makepkg.sh

(cd "$OUT" && if command -v sha256sum >/dev/null; then sha256sum ./*.deb ./*.pkg.tar.zst; else shasum -a 256 ./*.deb ./*.pkg.tar.zst; fi >SHA256SUMS)
echo
ls -lh "$OUT"/*.deb "$OUT"/*.pkg.tar.zst
