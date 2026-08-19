#!/usr/bin/env bash
# Link the in-repo first-party dev plugin (plugins/pragma-dev) into the app's
# plugin directory so a dev build of Pragma picks it up.
#
# Portable for any checkout location and any user: the repo and app config
# paths are derived at runtime — nothing is hardcoded to this machine. Run
# once after cloning (or after `git clean`), re-run to repair the link.
#
#   bash scripts/link-dev-plugin.sh
#
# Override the config dir with PRAGMA_CONFIG_DIR if your Tauri config path
# differs from the platform default (~/.config/dev.pragma.app on Linux).
set -eu

# Repo root = the script's parent's parent (scripts/..), whatever the cwd.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CFG="${PRAGMA_CONFIG_DIR:-$HOME/.config/dev.pragma.app}"
PLUGINS_DIR="$CFG/plugins"
SRC="$ROOT/plugins/pragma-dev"
LINK="$PLUGINS_DIR/pragma-dev"

if [ ! -f "$SRC/manifest.json" ]; then
  echo "error: no manifest at $SRC — is this the pragma repo?" >&2
  exit 1
fi

mkdir -p "$PLUGINS_DIR"

if [ -L "$LINK" ]; then
  target="$(readlink -f "$LINK")"
  if [ "$target" = "$(cd "$SRC" && pwd)" ]; then
    echo "ok: pragma-dev already linked -> $target"
  else
    echo "warn: $LINK points at $target, expected $SRC" >&2
    echo "      replace with: ln -sfn \"$SRC\" \"$LINK\"" >&2
    exit 1
  fi
  exit 0
fi

if [ -e "$LINK" ]; then
  echo "error: $LINK exists but is not a symlink (an installed copy?)" >&2
  echo "       remove it first, then re-run this script" >&2
  exit 1
fi

ln -s "$SRC" "$LINK"
echo "linked $LINK -> $SRC"