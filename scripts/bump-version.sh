#!/usr/bin/env bash
# Sync a release version across the places the APP version lives:
#   package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml
# The pragma-app npm wrapper keeps its OWN version (decoupled — see
# RELEASING.md; publishable separately via scripts/publish-app.mjs).
# Usage: ./scripts/bump-version.sh 1.0.0
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-}"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $0 <major.minor.patch>   e.g. $0 1.0.0" >&2
  exit 1
fi

node -e "
const fs = require('fs');
for (const f of ['package.json', 'src-tauri/tauri.conf.json']) {
  const j = JSON.parse(fs.readFileSync(f, 'utf8'));
  j.version = '$VERSION';
  fs.writeFileSync(f, JSON.stringify(j, null, 2) + '\n');
}
const toml = fs.readFileSync('src-tauri/Cargo.toml', 'utf8');
fs.writeFileSync(
  'src-tauri/Cargo.toml',
  toml.replace(/^version = \"[^\"]+\"/m, 'version = \"$VERSION\"')
);
"

# Keep Cargo.lock in sync with the new Cargo.toml version.
(cd src-tauri && cargo check --quiet)

echo "✓ bumped to $VERSION (package.json, tauri.conf.json, Cargo.toml + Cargo.lock)"
