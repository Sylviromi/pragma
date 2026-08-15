# Releasing Pragma

Pragma's release pipeline is fully automated: pushing a version tag makes
GitHub Actions build the native bundles for every platform and publish the
GitHub Release — no manual steps. A manual
**quick debug** path (workflow_dispatch) builds only the platforms you pick,
for when you need a fix out in minutes.

## One command: release.sh

Releasing is now a **single command** that can't get the version wrong:

```bash
./scripts/release.sh 1.0.0
```

It does the whole job from one version input, so the git tag and the version
in the code can never drift apart:

1. syncs the version across `package.json`, `tauri.conf.json`, `Cargo.toml`
   **and** `Cargo.lock` (fast — edits the lockfile directly, no `cargo build`);
2. lifts whatever is under `## [Unreleased]` in `CHANGELOG.md` into a dated
   `## [1.0.0] - YYYY-MM-DD` section at the top (newest first, fresh empty
   `## [Unreleased]` above it), so the release-notes extraction always finds it;
3. verifies every file agrees and matches the tag (`verify-version.mjs`) —
   fails loudly on any mismatch;
4. commits `Release 1.0.0`, tags `v1.0.0`, pushes `main` + the tag.

Flags:

- `--dry-run` — preview every step, **change nothing**, and never commit/tag/push.
- `--no-push` — bump + changelog + verify + commit + tag, but don't push.

It refuses to reuse an existing tag and refuses to commit with no staged
changes, so ``release.sh`` *cannot* produce a mislabeled release by accident.

> **Wrapper versioning:** `pragma-app` (the npm installer) versions *independently*
> of the app — `release.sh` doesn't touch it. The workflow publishes whatever
> version `packages/pragma-app/package.json` declares (skipping versions already
> on npm), so wrapper fixes ride along with app releases or stand alone. Do not
> manually publish wrapper versions between app releases unless you bump it in
> the repo too.

## One-time setup

1. **npm token** — the pipeline auto-publishes the `pragma-app` npm wrapper.
   Add the npm token as a GitHub secret: Settings → Secrets and variables →
   Actions → **`NPM_TOKEN`** (same token that lives in `pragma-plugins/.env`).
   Until it's set, the workflow skips npm and you publish locally with
   `node scripts/publish-app.mjs`.
2. **Build-finished notifications** (optional) — GitHub emails you about
   workflow runs if you watch the repo (Settings → Notification settings →
   enable "Actions"). For a push notification / chat message, add a secret
   **`NOTIFY_WEBHOOK_URL`**: an `https://ntfy.sh/<topic>` URL (zero-setup
   push notifications, free) or any Slack / Discord webhook. A completion
   message is posted automatically when the run finishes.
3. **Manual review** (optional) — by default the release is auto-published.
   If you want to smoke-test the draft before it goes live, set the repo
   **variable** `MANUAL_RELEASE_REVIEW=true`; the workflow then keeps the
   release as a draft and skips npm (the wrapper resolves `releases/latest`,
   which only counts published releases).

## Full release (all platforms)

```bash
# The one command. Bumps every version home, moves the changelog entry,
# verifies, then commits + tags v1.0.0 + pushes. Nothing else to remember.
./scripts/release.sh 1.0.0
```

That's it — the pipeline builds and publishes the release. You get a
notification when it's done.

If a version gate fails (tag already exists, files mismatched, nothing
staged), it tells you exactly why and refuses — never pushes a broken state.

## Quick debug update (fast, Linux-only by default)

For a hotfix that only needs to be out quickly (e.g. a Linux/GNOME fix): make
the code change and add its line to `## [Unreleased]` in `CHANGELOG.md`, then
run the **same** one-command release for the new patch version:

```bash
./scripts/release.sh 0.1.3
```

That bumps, commits, tags `v0.1.3` and pushes like a normal release. The
**Release** workflow then builds — pick `ubuntu-22.04` only (default) via
Actions → Release → "Run workflow" → platforms, and it does a single-platform
build in a few minutes instead of the full matrix. Rust builds are cached, so
it's quick.

Don't do both a tag push and a manual run for the same version — they race on
the same release.

## What happens next (pipeline detail)

- The **Release** workflow:
  - builds the bundles for the platforms in the matrix (Linux `.deb` /
    `.rpm` / `.AppImage` on ubuntu-22.04, macOS `.dmg` + `.app` for both
    Apple Silicon and Intel, Windows `.msi` / `.exe`) — parallel across
    runners, with npm + Rust caches so follow-up builds are minutes;
  - writes the release notes: install preamble + **this version's
    `CHANGELOG.md` section, embedded directly** (no link-out); missing
    entries fall back to a placeholder;
  - attaches them to the release, then **auto-publishes** it (unless
    `MANUAL_RELEASE_REVIEW=true`);
  - publishes the `pragma-app` npm wrapper (at its own repo version, only if
    not already on npm);
  - posts a completion message to `NOTIFY_WEBHOOK_URL` (if configured) —
    including a link to the release.
- The **Pages** workflow keeps the download page in sync on every push.

## Smoke-test checklist

- [ ] Linux `.AppImage`: runs from a clean machine (no system deps needed)
- [ ] Linux `.deb`: installs with `sudo apt install ./pragma_1.0.0_amd64.deb`,
      launches from the app menu
- [ ] macOS `.dmg`: drag-to-Applications install works, Gatekeeper prompt
      appears (unsigned builds — fine for a first release; see below)
- [ ] Windows `.msi`: installs, app launches
- [ ] In-app: create a session, run an agentic loop against a provider, click
      an agent link → opens in the system browser
- [ ] `npm install -g pragma-app && pragma` downloads and launches the app

With `MANUAL_RELEASE_REVIEW=true` the release waits as a draft so you can
run this checklist before hitting **Publish**; otherwise it's already live.

## Notes / follow-ups

- **Code signing**: currently unsigned on all platforms (users may see OS
  warnings). Add `bundle.macOS.signingIdentity`, Windows certificate env vars,
  or a signing service before distributing broadly.
- **Auto-update**: not wired up yet, but the release pipeline is already
  shaped for it. When you're ready: add `tauri-plugin-updater`, host a static
  `latest.json` (tauri-action can generate it via the `createUpdaterArtifacts`
  input), and point `tauri.conf.json` → `plugins.updater.endpoints` at it. The
  release assets are already produced in the right shape.
- **Plugin updates**: built in. Settings → Plugins → "Check for updates"
  polls each installed plugin's recorded source (npm `latest` dist-tag via
  `npm view`), and updates re-install through the same path. Pinned sources
  (`npm:pkg@ver`) are skipped by design. See PLUGINS.md.
- **npm publish tokens expire after 90 days** (npm's security cap, no way to
  extend). The token lives in `pragma-plugins/.env` (`NPM_TOKEN=…`, gitignored
  — the single source of truth for local publishes) and as the `NPM_TOKEN`
  GitHub secret for the pipeline. Change both with
  `node scripts/publish.mjs --token npm_<new>` (plugins) — the wrapper publish
  (`node scripts/publish-app.mjs`) reads the same .env. Both scripts use a
  temp userconfig so the token never lands in an npmrc.
- **CI gates**: every push/PR runs `npm run check`, vitest, `cargo fmt --check`,
  `cargo clippy -D warnings`, and `cargo test` on Linux/macOS/Windows — keep
  those green so a tag push never wastes a release build.
- The dev-only `scripts/dev-app.sh` launcher and `plugins/pragma-dev/` reference
  plugin are intentionally part of the repo (the latter is the architecture
  reference for this codebase, see CLAUDE.md).
