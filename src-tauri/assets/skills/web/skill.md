# Web skill

- `web_search` — DuckDuckGo search (no API key needed). Returns titles, URLs, snippets. Use when you need current external information: library docs, API changes, version numbers, known bugs, setup guides.
- `fetch_url` — fetch a page and return its text (HTML stripped). Use it to read the actual documentation or article behind a search hit.

## When to use

- Prefer project files and local documentation first — the repo is the source of truth for its own code.
- Use the web when you need something the project can't tell you: an external API's current behavior, a crate/package's docs, error messages from third-party tools, or the latest stable versions.
- Search first, then fetch the single most relevant result — don't fetch URLs blindly by guessing.

## Rules

- `fetch_url` truncates long pages (`max_chars`, default 8000) — for large docs, prefer targeted section URLs (`#anchor` where possible) or raise `max_chars` deliberately.
- Both tools are read-only and never require a working directory.
