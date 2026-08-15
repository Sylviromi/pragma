# pragma-app

The npm distribution of [Pragma](https://github.com/Sylviromi/pragma) — an
agentic AI coding harness (Tauri desktop app).

```bash
npm install -g pragma-app
pragma
```

On first run it resolves the **latest published release** from GitHub
Releases, downloads the matching asset for your platform
(Linux AppImage, macOS .app bundle, Windows NSIS installer), caches it under
`~/.pragma-app/`, and launches the app. Later runs use the cache.

- `pragma --update` — re-download the current release (e.g. after a corrupted cache)
- `PRAGMA_CACHE=/path` — override the cache directory

Requires Node 18+. The GitHub release must be published (not a draft) for the
latest-release lookup to work.

Platforms: Linux x64, macOS (Apple Silicon + Intel), Windows x64.
