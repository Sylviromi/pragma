#!/usr/bin/env bash
# Dev launcher WITHOUT Rust auto-reload.
#
# Why: `npm run tauri dev` rebuilds and RESTARTS the app the moment any Rust
# file changes — exactly when the coding agent inside is editing Rust, so it
# killed the agent's own run mid-flight.
#
# This launcher instead:
#   1. starts Vite dev on :1420 (if not already running),
#   2. runs the app binary directly via `cargo run`,
#   3. only relaunches when the app exits with the special "reload" code (23),
#      which is set by the `reload_app` tool after a successful rebuild.
#
# Rust edits therefore NEVER hot-restart you; they take effect only when the
# agent (or you) triggers a safe reload.
set -u
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
RELOAD_CODE=23

port_open() {
  # Try IPv4 first, then IPv6 — vite may bind either (usually ::1).
  (exec 3<>/dev/tcp/127.0.0.1/1420) 2>/dev/null && exec 3>&- && return 0
  (exec 3<>/dev/tcp/::1/1420) 2>/dev/null && exec 3>&- && return 0
  return 1
}

if ! port_open; then
  echo "[dev-app] starting vite dev server…"
  (npm run dev >/tmp/pragma-vite.log 2>&1 &)
  for _ in $(seq 1 40); do
    port_open && break
    sleep 0.5
  done
  if ! port_open; then
    echo "[dev-app] vite did not come up; tail of /tmp/pragma-vite.log:"
    tail -20 /tmp/pragma-vite.log
    exit 1
  fi
fi

trap 'echo "[dev-app] stopped"; kill 0 2>/dev/null' INT TERM

while true; do
  echo "[dev-app] launching app…"
  cargo run --manifest-path "$ROOT/src-tauri/Cargo.toml"
  code=$?
  if [ "$code" -eq "$RELOAD_CODE" ]; then
    echo "[dev-app] reload requested — rebuilding & relaunching in 3s"
    sleep 3
    continue
  fi
  echo "[dev-app] app exited with code $code — bye"
  exit 0
done