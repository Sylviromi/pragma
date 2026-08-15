#!/usr/bin/env bash
# One-command Arch publish: bump the AUR package to <version> and push it to
# aur.archlinux.org. Run it AFTER ./scripts/release.sh (the tag must exist so
# the source-tarball checksum can be computed).
#
#   ./scripts/aur-deploy.sh 0.1.3              # bump + push (AUR repo already cloned)
#   ./scripts/aur-deploy.sh 0.1.3 --clone       # first time: clone the AUR repo too
#
# Env:
#   AUR_DIR=/path/to/aur-repo   where the AUR git checkout lives (default ~/aur/pragma)
#
# Needs your AUR SSH key registered in your AUR account. For the very first
# package, create the empty repo on the AUR website first (aur.archlinux.org →
# Submit) — then --clone will pull it in.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-}"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $0 <major.minor.patch> [--clone]   e.g. $0 0.1.3" >&2
  exit 1
fi
CLONE=0
[ "${2:-}" = "--clone" ] && CLONE=1

AUR_REPO="ssh://aur@aur.archlinux.org/pragma.git"
AUR_DIR="${AUR_DIR:-$HOME/aur/pragma}"

# 1) Prepare PKGBUILD + .SRCINFO in this repo (updates pkgver, source sha256).
./scripts/aur-update.sh "$VERSION"

# 2) Make sure we have an AUR checkout.
if [ ! -d "$AUR_DIR/.git" ]; then
  if [ "$CLONE" -eq 0 ]; then
    echo "!! no AUR checkout at $AUR_DIR — first time? re-run with --clone" >&2
    exit 1
  fi
  echo "==> cloning $AUR_REPO"
  mkdir -p "$(dirname "$AUR_DIR")"
  git clone "$AUR_REPO" "$AUR_DIR"
fi

# 3) Sync the prepared files (AUR only needs PKGBUILD + .SRCINFO).
cp distrib/aur/PKGBUILD distrib/aur/.SRCINFO "$AUR_DIR/"
cd "$AUR_DIR"
git add PKGBUILD .SRCINFO
if git diff --cached --quiet; then
  echo "==> AUR package already at $VERSION — nothing to push."
else
  git commit -m "upgpkg: pragma $VERSION"
  git push origin HEAD
  echo "✓ pushed AUR package $VERSION → aur.archlinux.org/pragma"
fi
