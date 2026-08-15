# Changelog

All notable changes to Pragma are documented here. Format based on
[Keep a Changelog](https://keepachangelog.com/), versions follow
[SemVer](https://semver.org/).

## [0.1.1] - 2026-08-15

### Fixed

- **GNOME (Wayland): the window could not be moved or fullscreened.** The
  GTK title bar was being removed on every Wayland session, but GNOME draws
  its own client-side decorations — the decorations are now hidden only on
  compositors that provide their own frame (Hyprland, Sway, …), so GNOME
  keeps the title bar with its move / maximize / fullscreen controls.

## [0.1.0] - 2026-08-15

### Fixed

- Agent links (markdown `target="_blank"`) now open in the system browser
  instead of being swallowed by the WebView (`tauri-plugin-opener`).

### Added

- Release pipeline: CI gates (frontend checks/tests, Rust fmt/clippy/tests on
  Linux/macOS/Windows) and an automated draft-release workflow for version
  tags (`v*`). See [RELEASING.md](RELEASING.md).
- `scripts/bump-version.sh` — syncs the version across `package.json`,
  `tauri.conf.json`, and `Cargo.toml` in one step.
- **npm-only plugin distribution** (the pi/OpenCode model): one plugin = one
  npm package. `npm:` sources install with a real `npm install` into the
  shared npm prefix, so sidecar handlers can `require` dependencies; the
  plugins-dir entry is a symlink to the package. Local folders stay for the
  dev workflow. Git/zip sources, the single-repo registry (`#subpath`
  sparse checkout, `index.json` + generator) and the marketplace registry
  URLs were removed.
- **Marketplace = npm search**: Settings → Marketplace queries the npm
  registry search API (`keywords:pragma-plugin`), one-click installs;
  installed plugins are matched by their recorded source.
- **Plugin updates**: automatic check at startup (silent, throttled to once
  per 10 min) plus a manual "Check for updates"; badges in the Settings
  sidebar and per-plugin cards with in-place Update. Version-pinned sources
  (`npm:pkg@ver`) are skipped (pi semantics).
- **Publisher namespacing**: `publisher` field in the manifest — first-party
  plugins (`publisher: "pragma"`) get a Pragma badge in Settings; ids stay
  flat while npm scopes namespace at the distribution level.
- MIT license + attribution metadata (`author`, `publisher`, `repository`,
  `homepage`) shown in Settings → Plugins.

## [Unreleased]
