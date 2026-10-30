#!/usr/bin/env bash
# Make the Arch Linux packages with makepkg, in the archlinux image (scripts/package-linux.sh
# runs it there): from /out/arch/PKGBUILD and the trees /out/snout-code-<version>-linux-<arch>.tar.zst,
# for each architecture of $ARCHES, into /out. makepkg runs as a user of its own, as it must.
set -euo pipefail
# pacman's download sandbox needs system calls an emulated x86_64 container may not have.
sed -i -e 's/^DownloadUser/#DownloadUser/' -e 's/^#DisableSandboxSyscalls/DisableSandboxSyscalls/' /etc/pacman.conf
pacman -Sy --noconfirm --needed base-devel >/dev/null
useradd -m builder 2>/dev/null || true
for arch in $ARCHES; do
  work="/home/builder/$arch"
  rm -rf "$work"
  mkdir -p "$work"
  cp /out/arch/PKGBUILD "/out/snout-code-$VERSION-linux-$arch.tar.zst" "$work/"
  # The architecture to package for: the files are only copied, so any can be made here.
  sed -e "s/^CARCH=.*/CARCH=\"$arch\"/" -e "s/^CHOST=.*/CHOST=\"$arch-pc-linux-gnu\"/" \
    -e "s/^PKGEXT=.*/PKGEXT='.pkg.tar.zst'/" /etc/makepkg.conf >"$work/makepkg.conf"
  chown -R builder: "$work"
  su builder -c "cd '$work' && makepkg --config makepkg.conf --nodeps --force --noconfirm" >/dev/null
  cp "$work"/snout-code-"$VERSION"-*-"$arch".pkg.tar.zst /out/
  (cd "$work" && su builder -c "makepkg --config makepkg.conf --printsrcinfo" >/out/arch/.SRCINFO)
  echo "Built $(ls /out/snout-code-"$VERSION"-*-"$arch".pkg.tar.zst)"
done
chown -R "$HOST_UID:$HOST_GID" /out
