#!/usr/bin/env bash
# Low-level bump: sync the APP version across package.json, tauri.conf.json,
# Cargo.toml + Cargo.lock only. Does NOT touch the changelog, commit, tag or
# push — for the full one-command release (bump + changelog + commit + tag +
# push) use:  ./scripts/release.sh <version>
set -euo pipefail
cd "$(dirname "$0")/.."
exec node scripts/sync-version.mjs --set "${1:?usage: bump-version.sh <major.minor.patch>}"
