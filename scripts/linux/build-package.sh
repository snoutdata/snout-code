#!/usr/bin/env bash
# Build SnoutCode for Linux and its .deb, inside the build container (scripts/linux/build.Dockerfile).
# scripts/package-linux.sh runs it; it is not meant to be run on its own.
#
#   build-package.sh <src> <out> <server-dist>
#
# <src> is the repository (read only), <out> where the packages go, <server-dist> the folder of
# the snout-server builds for Remote SSH (scripts/build-server-linux.sh). Writes, for this
# machine's architecture:
#   <out>/snout-code_<version>_<amd64|arm64>.deb
#   <out>/snout-code-<version>-linux-<x86_64|aarch64>.tar.zst   the installed tree (usr/...),
#                                                                which the Arch package is made of
set -euo pipefail

SRC="$1"
OUT="$2"
SERVER_DIST="$3"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$SRC/Cargo.toml" | head -1)"
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64) DEB_ARCH=amd64 ;;
  aarch64) DEB_ARCH=arm64 ;;
  *) echo "unsupported architecture: $ARCH" >&2; exit 1 ;;
esac
WORK="/work/$ARCH"
rm -rf "$WORK"
mkdir -p "$WORK" "$OUT"

# --- The binary ---------------------------------------------------------------------------

features=()
if [[ "${SNOUT_NO_GHOSTTY:-}" != 1 ]]; then
  [[ "$(zig version 2>/dev/null)" == 0.15.* ]] || { echo "No Zig 0.15 for the Ghostty terminal engine." >&2; exit 1; }
  features=(--features ghostty)
fi
echo "== Building snout-code ($ARCH)"
CARGO_PROFILE_RELEASE_STRIP=true cargo build --locked --release -p snout-app ${features[@]+"${features[@]}"} \
  --manifest-path "$SRC/Cargo.toml"
BIN="${CARGO_TARGET_DIR:-$SRC/target}/release/snout-code"

# --- The engine ---------------------------------------------------------------------------

# Built from a copy, so the source tree's engine/build (a Mac's development build) is left alone.
echo "== Building the Java engine ($ARCH)"
rsync -a --delete --exclude build/ --exclude .gradle/ "$SRC/engine/" "$WORK/engine-src/"
(cd "$WORK/engine-src" && ./gradlew --no-daemon --quiet :sync-cli:snoutDist -PsnoutDistDir="$WORK/engine-dist")

# --- The tree -----------------------------------------------------------------------------

ROOT="$WORK/root"
LIB="$ROOT/usr/lib/snout-code"
mkdir -p "$LIB" "$ROOT/usr/bin" "$ROOT/usr/share/applications" "$ROOT/usr/share/doc/snout-code" \
  "$ROOT/usr/share/licenses/snout-code" "$ROOT/usr/share/metainfo"
install -m 755 "$BIN" "$LIB/snout-code"
ln -s ../lib/snout-code/snout-code "$ROOT/usr/bin/snout-code"

# What never runs on Linux: jdtls's launch scripts, its feature archives, the configurations
# for macOS and Windows and for jdtls's syntax server, and the native launcher fragments of
# macOS and Windows. The runtime stays whole: it is also the JDK that Gradle, run
# configurations and the debugger fall back to when the machine has none.
rsync -a \
  --exclude '/jdtls/bin/' \
  --exclude '/jdtls/features/' \
  --exclude '/jdtls/config_win/' --exclude '/jdtls/config_mac*/' --exclude '/jdtls/config_ss_*/' \
  --exclude '/jdtls/plugins/org.eclipse.equinox.launcher.cocoa.*' \
  --exclude '/jdtls/plugins/org.eclipse.equinox.launcher.win32.*' \
  "$WORK/engine-dist/" "$LIB/engine/"
cp "$SRC/engine/NOTICE" "$LIB/engine/NOTICE"
"$LIB/engine/runtime/bin/java" -version >/dev/null 2>&1 || { echo "The engine's runtime does not start." >&2; exit 1; }

mkdir -p "$LIB/server"
cp "$SERVER_DIST"/snout-server-linux-x86_64 "$SERVER_DIST"/snout-server-linux-aarch64 "$SERVER_DIST"/SHA256SUMS "$LIB/server/"
chmod 755 "$LIB/server"/snout-server-linux-*

# Icons: the scalable one, and PNGs in the sizes desktops look for.
for size in 16 24 32 48 64 128 256 512; do
  d="$ROOT/usr/share/icons/hicolor/${size}x${size}/apps"
  mkdir -p "$d"
  rsvg-convert -w "$size" -h "$size" "$SRC/assets/icon/snout.svg" -o "$d/snout-code.png"
done
mkdir -p "$ROOT/usr/share/icons/hicolor/scalable/apps"
cp "$SRC/assets/icon/snout.svg" "$ROOT/usr/share/icons/hicolor/scalable/apps/snout-code.svg"

cat >"$ROOT/usr/share/applications/snout-code.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=SnoutCode
GenericName=Integrated Development Environment
Comment=A native IDE for Java, Gradle, Maven and Spring
Exec=snout-code %F
Icon=snout-code
Terminal=false
Categories=Development;IDE;Java;
Keywords=java;gradle;maven;spring;git;ide;
StartupNotify=true
StartupWMClass=snout-code
MimeType=inode/directory;
DESKTOP
desktop-file-validate "$ROOT/usr/share/applications/snout-code.desktop" 2>/dev/null || true

cat >"$ROOT/usr/share/metainfo/com.snoutdata.code.metainfo.xml" <<XML
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>com.snoutdata.code</id>
  <name>SnoutCode</name>
  <summary>A native IDE for Java, Gradle, Maven and Spring</summary>
  <metadata_license>MIT</metadata_license>
  <project_license>MIT</project_license>
  <launchable type="desktop-id">snout-code.desktop</launchable>
  <url type="homepage">https://github.com/snoutdata/snout-code</url>
  <developer id="com.snoutdata"><name>SNOUTDATA INC.</name></developer>
  <releases><release version="$VERSION"/></releases>
</component>
XML

cp "$SRC/LICENSE" "$ROOT/usr/share/licenses/snout-code/LICENSE"
cp "$SRC/NOTICE" "$ROOT/usr/share/doc/snout-code/NOTICE"
{
  echo "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/"
  echo "Upstream-Name: SnoutCode"
  echo "Source: https://github.com/snoutdata/snout-code"
  echo
  echo "Files: *"
  echo "Copyright: SNOUTDATA INC."
  echo "License: MIT"
  sed 's/^/ /; s/^ $/ ./' "$SRC/LICENSE"
  echo
  echo "Files: usr/lib/snout-code/engine/*"
  echo "Comment: The Java engine: jdtls, Eclipse and the other components named in"
  echo " usr/lib/snout-code/engine/NOTICE, under their own licences, and a Java runtime"
  echo " linked from Eclipse Temurin (GPL-2.0 with the Classpath Exception)."
  echo "License: various"
} >"$ROOT/usr/share/doc/snout-code/copyright"

# --- The .deb -----------------------------------------------------------------------------

# The libraries the binary links, as Debian packages (dpkg-shlibdeps), and those GPUI loads at
# run time: Vulkan (or OpenGL through EGL) to draw, xkbcommon for the keyboard; and a font
# GPUI can stand in for the system's own with (it cannot start without one).
mkdir -p "$WORK/shlibs/debian"
echo "Source: snout-code" >"$WORK/shlibs/debian/control"
linked="$(cd "$WORK/shlibs" && dpkg-shlibdeps -O -e"$LIB/snout-code" 2>/dev/null | sed -n 's/^shlibs:Depends=//p')"
depends="$linked"
for dep in libvulkan1 libegl1 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libfontconfig1 libfreetype6; do
  [[ ", $depends," == *", $dep "* || ", $depends," == *", $dep,"* ]] || depends="${depends:+$depends, }$dep"
done
depends="$depends, fonts-dejavu-core | fonts-noto-core | fonts-cantarell"
SIZE_KB="$(du -sk "$ROOT/usr" | cut -f1)"
mkdir -p "$ROOT/DEBIAN"
cat >"$ROOT/DEBIAN/control" <<CONTROL
Package: snout-code
Version: $VERSION
Architecture: $DEB_ARCH
Maintainer: SnoutCode developers <295991524+snout-dev@users.noreply.github.com>
Installed-Size: $SIZE_KB
Depends: $depends
Recommends: git, mesa-vulkan-drivers | vulkan-icd, xdg-utils, curl
Suggests: zenity
Section: devel
Priority: optional
Homepage: https://github.com/snoutdata/snout-code
Description: native IDE for Java, Gradle, Maven and Spring
 SnoutCode is a native integrated development environment for Java projects
 built with Gradle or Maven, with Spring support, a Git panel, a terminal and
 coding agents. It brings its own Java runtime for the Java engine, so no JDK
 is needed to start it.
CONTROL
DEB="$OUT/snout-code_${VERSION}_${DEB_ARCH}.deb"
dpkg-deb --root-owner-group -Zxz --build "$ROOT" "$DEB" >/dev/null
rm -rf "$ROOT/DEBIAN"
echo "Built $DEB ($(du -h "$DEB" | cut -f1))"
dpkg-deb --info "$DEB" | sed -n '/Depends/p'

# --- The tree, for the Arch package -------------------------------------------------------

TREE="$OUT/snout-code-$VERSION-linux-$ARCH.tar.zst"
tar --owner=0 --group=0 --numeric-owner -C "$ROOT" -cf - usr | zstd -q -19 -T0 -o "$TREE" -f
chmod 644 "$TREE"
echo "Built $TREE ($(du -h "$TREE" | cut -f1))"
