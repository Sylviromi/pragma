#!/usr/bin/env bash
# DO THE WHOLE RELEASE in one command. This is the ONLY step you run:
#
#   1. syncs the APP version across every home (package.json, tauri.conf.json,
#      Cargo.toml, Cargo.lock),
#   2. moves the CHANGELOG "Unreleased" section into a dated release entry at
#      the top (newest first, fresh "Unreleased" above it),
#   3. verifies every file agrees (and matches the tag — impossible to drift),
#   4. commits, tags v<version> and pushes — all with the version as a single
#      input, so the tag and the code version can never be out of sync.
#
# The pragma-app npm wrapper keeps its own independent version (RELEASING.md);
# it is published separately and this script does not touch it.
#
# Usage:
#   ./scripts/release.sh 1.2.3            # bump + changelog + commit + tag + push
#   ./scripts/release.sh 1.2.3 --dry-run  # preview everything, change nothing
#   ./scripts/release.sh 1.2.3 --no-push  # bump + changelog + commit + tag, no push
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-}"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: $0 <major.minor.patch> [--dry-run|--no-push]   e.g. $0 1.2.3" >&2
  exit 1
fi

DRY=0 PUSH=1
for a in "${@:2}"; do
  case "$a" in
    --dry-run) DRY=1 ;;
    --no-push) PUSH=0 ;;
    *) echo "unknown flag: $a" >&2; exit 1 ;;
  esac
done

TAG="v$VERSION"
TODAY="$(date +%F)"

echo "==> Preparing release $VERSION (tag $TAG)"
echo "    touches: package.json, tauri.conf.json, Cargo.toml, Cargo.lock, CHANGELOG.md"

# Refuse to reuse an existing tag (local or on origin).
if git rev-parse --verify --quiet "$TAG" >/dev/null 2>&1; then
  echo "!! tag $TAG already exists locally — bump to a new version first" >&2
  exit 1
fi
if [ "$DRY" -eq 0 ] && git ls-remote --exit-code --tags origin "$TAG" >/dev/null 2>&1; then
  echo "!! tag $TAG already exists on origin — bump to a new version first" >&2
  exit 1
fi

# Warn (not fail) if the Unreleased section is empty and would produce
# placeholder release notes.
if ! grep -q '^## \[Unreleased\]' CHANGELOG.md; then
  echo "!! CHANGELOG.md has no '## [Unreleased]' section — add one or add entries,"
  echo "   else the release notes fall back to a placeholder."
fi

run() { if [ "$DRY" -eq 1 ]; then echo "    [dry-run] $*"; else "$@"; fi }

# 1) Sync the app version everywhere (fast — edits Cargo.lock directly).
run node scripts/sync-version.mjs --set "$VERSION"

# 2) Rewrite CHANGELOG: lift whatever was under '## [Unreleased]' into a dated
#    release section placed at the TOP (newest first), with a fresh empty
#    '## [Unreleased]' above it. Version + date are passed via env so the JS
#    needs no shell escaping at all.
export PRA_VER="$VERSION" PRA_DATE="$TODAY"
CHANGELOG_JS='
const fs = require("fs");
const p = "CHANGELOG.md";
const lines = fs.readFileSync(p,"utf8").split("\n");
const h = lines.findIndex(l => l.trim() === "## [Unreleased]");
if (h < 0) { console.error("no ## [Unreleased] header in CHANGELOG.md"); process.exit(2); }
let e = h + 1;
while (e < lines.length && !/^## /.test(lines[e])) e++;
const released = lines.slice(h+1, e).map(s => s.replace(/ +$/,"")).join("\n").trim();
const rest = lines.slice(0, h).concat(lines.slice(e));
const firstSec = rest.findIndex(l => /^## /.test(l));
const lead = rest.slice(0, firstSec).join("\n");
const sections = rest.slice(firstSec).join("\n");
const rel = "## [" + process.env.PRA_VER + "] - " + process.env.PRA_DATE;
let out = lead + "\n## [Unreleased]\n\n" + rel + "\n";
if (released) out += released + "\n\n"; else out += "\n";
out += sections;
fs.writeFileSync(p, out);
console.log("    CHANGELOG.md: " + rel + " placed at top, fresh Unreleased above");
'
run node -e "$CHANGELOG_JS"

# 3) Verify every file agrees and matches the tag version (the drift guard).
run node scripts/verify-version.mjs "$VERSION"

# Hard stop: a dry run must NEVER reach the commit/tag/push step, even if a
# future refactor breaks run() — exit here no matter what.
if [ "$DRY" -eq 1 ]; then
  echo
  echo "==> DRY RUN complete — nothing was changed. Re-run without --dry-run to go live."
  exit 0
fi

# 4) Commit + tag (tag derived from the version, so they can't drift).
git add -A
if git diff --cached --quiet; then
  echo "!! no file changes staged — is $VERSION already the app version?" >&2
  exit 1
fi
git commit -m "Release $VERSION"
git tag "$TAG"
echo "==> committed 'Release $VERSION' and tagged $TAG"

if [ "$PUSH" -eq 1 ]; then
  git push origin main --tags
  echo "==> pushed main + $TAG — the Release workflow builds and publishes."
else
  echo "==> --no-push: run this when ready:  git push origin main --tags"
fi
