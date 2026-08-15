# Changelog

All notable changes to Pragma are documented here. Format based on
[Keep a Changelog](https://keepachangelog.com/), versions follow
[SemVer](https://semver.org/).

## [Unreleased]

### Fixed

- Agent links (markdown `target="_blank"`) now open in the system browser
  instead of being swallowed by the WebView (`tauri-plugin-opener`).

### Added

- Release pipeline: CI gates (frontend checks/tests, Rust fmt/clippy/tests on
  Linux/macOS/Windows) and an automated draft-release workflow for version
  tags (`v*`). See [RELEASING.md](RELEASING.md).
- `scripts/bump-version.sh` — syncs the version across `package.json`,
  `tauri.conf.json`, and `Cargo.toml` in one step.
- **Source-based plugin installs** (the pi/OpenCode model): install plugins
  from `git:` repos, `npm:` packages, or local folders instead of hand-made
  archives. Sources are recorded in `plugin_sources.json`; re-installing an
  id replaces the folder atomically.
- **Plugin updates**: automatic check at startup (silent, throttled to once
  per 10 min) plus a manual "Check for updates"; badges in the Settings
  sidebar and per-plugin cards with in-place Update. Version-pinned sources
  are skipped.
- **Publisher namespacing**: `publisher` field in the manifest — first-party
  plugins (`publisher: "pragma"`) get a Pragma badge in Settings; ids stay
  flat while the source (npm scope / git owner) namespaces.
- MIT license + attribution metadata (`author`, `publisher`, `repository`,
  `homepage`) shown in Settings → Plugins.
