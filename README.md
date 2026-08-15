# Pragma

An agentic AI coding harness — a Tauri 2 desktop app with a Rust agentic tool loop,
Svelte 5 frontend, SQLite persistence, multi-provider streaming, and browser-style tabs.

## Stack

- **Frontend:** Svelte 5 (runes), TypeScript, SvelteKit (SPA mode via adapter-static)
- **Backend:** Rust, Tauri 2, sqlx (SQLite), reqwest (SSE streaming), tokio
- **Agentic loop:** Runs entirely in Rust — sends messages → parses `tool_use` →
  executes tools (after frontend approve/deny) → feeds results back → loops until done

## Download

Installers for every platform are attached to each
[release](https://github.com/Sylviromi/pragma/releases) — the download page
lives at **https://sylviromi.github.io/pragma/** (Linux `.deb`/`.AppImage`/`.rpm`,
macOS `.dmg` for Apple Silicon + Intel, Windows `.msi`/`.exe`).

Plugins are **npm packages** (`pragma-plugin-*`): install them from the app's
**Settings → Marketplace**, or with `npm:pragma-plugin-<id>`.

## Development

```bash
npm install
npm run tauri dev
```

Rust tests: `cd src-tauri && cargo test`
Frontend tests: `npm run test` (vitest)

### Headless / VM note

On virtualized displays WebKitGTK can render a black window. If that happens, run with:

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 npm run tauri dev
```

### Adding routes while the dev server is running

New route files (`src/routes/...`) require a **dev-server restart** to reach the
app: `svelte-kit sync` regenerates `.svelte-kit/generated`, but vite's watcher
deliberately ignores that directory (to avoid HMR storms from agent edits), so
the running server keeps serving a stale client route manifest — navigation to
the new route falls back to a hard reload and can surface `hydration_mismatch`
(server renders the route, the stale client doesn't). Restart `npm run dev`
(or `npm run tauri`) after adding a route.

## Building from source

```bash
npm install
npm run tauri build
```

Produces installable bundles in `src-tauri/target/release/bundle/` (`.deb` / `.rpm` /
`.AppImage` on Linux, `.dmg` / `.app` on macOS, `.msi` / `.exe` on Windows).

## Releasing

Releases are driven by Git tags: push `vX.Y.Z` and GitHub Actions builds every
platform's installers and attaches them to a draft release. Full runbook —
including the version-bump script (`scripts/bump-version.sh`), changelog
conventions, and the smoke-test checklist — lives in
[RELEASING.md](RELEASING.md).

## Tools available to the agent

- File ops: `read_file`, `write_file`, `edit_file`, `delete_file`, `list_files`,
  `search_files`
- Shell: `run_command`
- Git: `git_status`, `git_diff`, `git_log`, `git_add`, `git_commit`
- Web: `web_search` (DuckDuckGo Lite, no key), `fetch_url`

## API keys

Settings → Providers (`~/.config/dev.pragma.app/settings.json`). Built-in providers:
Anthropic (Claude), DeepSeek, and a local OpenAI-compatible endpoint (`localhost:4000`).

## Project layout

```
src/                      Svelte 5 frontend (stores, components, routes, styles)
src-tauri/src/db/         SQLite schema + migrations
src-tauri/src/commands/   Tauri IPC commands (sessions, messages, providers, tools)
src-tauri/src/providers/  Anthropic + OpenAI-compatible streaming providers
src-tauri/src/tools/      The tool executor suite
scripts/                  Dev launcher + release version-bump script
plugins/pragma-dev/       First-party reference plugin (architecture docs for this codebase)
```

## License

[MIT](LICENSE) — © 2025 Pragma contributors. Redistributions must retain the
copyright notice above; see the license file for the full terms.