#!/usr/bin/env bash
# Install the Linux packages in clean containers and start the app from them: the .deb on
# Debian 13, Ubuntu 24.04 and Ubuntu 22.04, the Arch Linux package on Arch Linux (whose image
# exists for x86_64 only). No Java is installed: each opens a Maven project under Xvfb and
# must offer to download a JDK for its build; then, with Temurin 21 where the app keeps the
# JDKs it downloads (taken from the build image, as the Download button would fetch it), its
# Java engine must start on the runtime the package brings. Screenshots and logs go to
# target/linux-check/install-<distro>-<arch>/.
#
#   scripts/linux/install-test.sh [x86_64|aarch64] [debian:trixie ubuntu:24.04 ...]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PKGS="$ROOT/target/linux-packages"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
case "$(uname -m)" in arm64|aarch64) arch=aarch64 ;; *) arch=x86_64 ;; esac
case "${1:-}" in x86_64|aarch64) arch="$1"; shift ;; esac
case "$arch" in x86_64) platform=linux/amd64 deb_arch=amd64 ;; *) platform=linux/arm64 deb_arch=arm64 ;; esac
distros=("$@")
if [[ ${#distros[@]} -eq 0 ]]; then
  distros=(debian:trixie ubuntu:24.04 ubuntu:22.04)
  [[ $arch == x86_64 ]] && distros+=(archlinux:latest)
fi

# Temurin 21 for the build, from the build image (scripts/package-linux.sh made it).
JDK="$ROOT/target/linux-check/temurin-21-$arch"
if [[ ! -x "$JDK/bin/java" ]]; then
  c="$(docker create --platform "$platform" "snout-linux-build:1-$arch")"
  rm -rf "$JDK"
  mkdir -p "$(dirname "$JDK")"
  docker cp "$c:/opt/jdk21" "$JDK" >/dev/null
  docker rm "$c" >/dev/null
fi

failed=()
for distro in "${distros[@]}"; do
  name="install-${distro//[:.]/-}-$arch"
  out="$ROOT/target/linux-check/$name"
  mkdir -p "$out"
  case "$distro" in
    archlinux*)
      pkg="snout-code-$VERSION-1-$arch.pkg.tar.zst"
      install='sed -i -e "s/^DownloadUser/#DownloadUser/" -e "s/^#DisableSandboxSyscalls/DisableSandboxSyscalls/" /etc/pacman.conf
        pacman -Syu --noconfirm >/dev/null
        pacman -S --noconfirm --needed xorg-server-xvfb xorg-xdpyinfo xdotool imagemagick git dbus vulkan-swrast procps-ng >/dev/null
        pacman -U --noconfirm "/pkgs/'"$pkg"'" >/dev/null'
      ;;
    *)
      pkg="snout-code_${VERSION}_$deb_arch.deb"
      install='export DEBIAN_FRONTEND=noninteractive; apt-get update -qq
        apt-get install -y -qq xvfb x11-utils xdotool imagemagick git dbus-x11 procps >/dev/null
        apt-get install -y -qq "/pkgs/'"$pkg"'" >/dev/null'
      ;;
  esac
  [[ -f "$PKGS/$pkg" ]] || { echo "No $pkg in $PKGS: run scripts/package-linux.sh $arch first." >&2; exit 1; }
  echo "== $distro ($arch): installing $pkg"
  if docker run --rm --platform "$platform" --init \
      -v "$PKGS":/pkgs:ro -v "$ROOT/scripts/linux":/scripts:ro -v "$out":/out -v "$JDK":/opt/temurin-21:ro \
      -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
      "$distro" bash -c "set -e; $install
        test -x /usr/bin/snout-code; test -x /usr/lib/snout-code/engine/runtime/bin/java
        ! command -v java >/dev/null || { echo 'java is installed: the check needs a machine without it'; exit 1; }
        s=0; /scripts/smoke.sh --x11 --out /out && /scripts/smoke.sh --x11 --out /out --jdk /opt/temurin-21 || s=\$?
        chown -R \$HOST_UID:\$HOST_GID /out; exit \$s"; then
    echo "== $distro ($arch): passed"
  else
    echo "== $distro ($arch): FAILED (see $out)"
    failed+=("$distro")
  fi
done
[[ ${#failed[@]} -eq 0 ]] || { echo "Failed: ${failed[*]}" >&2; exit 1; }
