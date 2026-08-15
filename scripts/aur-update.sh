#!/usr/bin/env bash
# Keep the Arch/AUR package (`distrib/aur/PKGBUILD`) in sync with a release.
# Run AFTER the release tag is pushed (it needs the tag's source tarball,
# which only exists once the tag is up):
#
#   ./scripts/aur-update.sh 0.1.3
#
# Then commit + push the updated files:
#   git add distrib/aur scripts/aur-update.sh && git commit -m "chore(aur): 0.1.3"
#
# Updates pkgver, the source tarball URL, its sha256, resets pkgrel, and
# regenerates .SRCINFO (via makepkg when available).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-}"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $0 <major.minor.patch>   e.g. $0 0.1.3" >&2
  exit 1
fi

PKGBUILD=distrib/aur/PKGBUILD
URL="https://github.com/Sylviromi/pragma/archive/refs/tags/v${VERSION}.tar.gz"

echo "==> Fetching source tarball for v$VERSION"
TMP="$(mktemp)"
curl -sfL -o "$TMP" "$URL" || { echo "!! could not fetch $URL (tag pushed yet?)" >&2; rm -f "$TMP"; exit 1; }
SHA="$(sha256sum "$TMP" | cut -d' ' -f1)"
rm -f "$TMP"

echo "==> Updating $PKGBUILD to $VERSION (sha256 $SHA)"
sed -i "s/^pkgver=.*/pkgver='$VERSION'/" "$PKGBUILD"
sed -i "s/^pkgrel=.*/pkgrel=1/" "$PKGBUILD"
sed -i "s|refs/tags/v[0-9]\+\.[0-9]\+\.[0-9]\+\.tar\.gz|refs/tags/v${VERSION}.tar.gz|" "$PKGBUILD"
sed -i "s/^sha256sums=.*/sha256sums=('$SHA')/" "$PKGBUILD"

if command -v makepkg >/dev/null 2>&1; then
  (cd distrib/aur && makepkg --printsrcinfo > .SRCINFO)
  echo "✓ AUR package synced to $VERSION (pkgver, source, sha256, .SRCINFO)"
else
  echo "✓ PKGBUILD synced to $VERSION (makepkg not found — regenerate .SRCINFO on an Arch box)"
fi
