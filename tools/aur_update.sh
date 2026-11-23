#!/usr/bin/env bash
# Sets version and checksum in a copy of packaging/aur/PKGBUILD.
#   tools/aur_update.sh <version> <tarball> <PKGBUILD> [--local]
# --local: source is the tarball next to the PKGBUILD (CI job arch-package)
# instead of the release download (CI job aur). Writes nothing else and pushes
# nothing; .SRCINFO comes from `makepkg --printsrcinfo` afterwards.
set -euo pipefail
ver="$1"
tarball="$2"
pkgbuild="$3"
mode="${4:-}"
[[ "$ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version must be X.Y.Z, got $ver" >&2; exit 1; }
sum="$(sha256sum "$tarball" | cut -d' ' -f1)"
sed -i -E "s/^pkgver=.*/pkgver=${ver}/; s/^pkgrel=.*/pkgrel=1/; s/^sha256sums=.*/sha256sums=('${sum}')/" "$pkgbuild"
if [ "$mode" = "--local" ]; then
  sed -i -E "s|^source=.*|source=(\"$(basename "$tarball")\")|" "$pkgbuild"
fi
grep -E '^(pkgver|sha256sums|source)=' "$pkgbuild"
