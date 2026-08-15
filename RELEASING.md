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

## One-time setup

1. **Build-finished notifications** (optional) — GitHub emails you about
   workflow runs if you watch the repo (Settings → Notification settings →
   enable "Actions"). For a push notification / chat message, add a secret
   **`NOTIFY_WEBHOOK_URL`**: an `https://ntfy.sh/<topic>` URL (zero-setup
   push notifications, free) or any Slack / Discord webhook. A completion
   message is posted automatically when the run finishes.
2. **Manual review** (optional) — by default the release is auto-published.
   If you want to smoke-test the draft before it goes live, set the repo
   **variable** `MANUAL_RELEASE_REVIEW=true`; the workflow then keeps the
   release as a draft for you to review before publishing.

### Auto-update & code-signing secrets

Auto-update is wired in (`tauri-plugin-updater`, `createUpdaterArtifacts` in the
workflow, and an in-app "Update & restart" banner). The updater artifacts are
**signed with an ed25519 key** — without it the Release job errors. Add these
two GitHub secrets (Settings → Secrets and variables → Actions):

- `TAURI_SIGNING_PRIVATE_KEY` — the updater private key (one-time, via
  `tauri signer generate -w pragma-updater.key`; a generated copy is at
  `~/pragma-updater.key` — keep it safe).
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — its password.

The matching **public** key is already in `src-tauri/tauri.conf.json`
(`plugins.updater.pubkey`). **Add these two before the next tag push, or the
release build fails.**

OS code signing (optional — needed for *smooth* macOS/Windows updates, i.e.
no Gatekeeper/SmartScreen prompts when updating):

- Windows: `CSC_LINK` (base64 of the `.pfx`), `CSC_KEY_PASSWORD`.
- macOS: `APPLE_SIGNING_IDENTITY`, `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
  plus notarization `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID`.

Empty values are skipped, so releases still build without them.

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

## Arch Linux (AUR)

`distrib/aur/` holds a source-built Arch package (`pragma`) that installs a
proper desktop entry + hicolor icons — real desktop integration, unlike the
portable AppImage. To publish it after a release (the tag must be pushed
first, so this is a follow-up to `release.sh`):

```bash
./scripts/aur-deploy.sh 0.1.3        # bump PKGBUILD + .SRCINFO and push to AUR
./scripts/aur-deploy.sh 0.1.3 --clone # first time on a machine (clones the AUR repo)
```

This needs your AUR SSH key. The very first package is created on the AUR
website first (aur.archlinux.org → Submit, then add your SSH key). Full
details in `distrib/aur/README.md`.

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

With `MANUAL_RELEASE_REVIEW=true` the release waits as a draft so you can
run this checklist before hitting **Publish**; otherwise it's already live.

## Notes / follow-ups

- **Code signing**: currently unsigned on all platforms (users may see OS
  warnings). Add `bundle.macOS.signingIdentity`, Windows certificate env vars,
  or a signing service before distributing broadly.
- **Auto-update**: wired in — the app checks for a newer release on launch
  (via `releases/latest/download/latest.json`, generated by the workflow's
  `createUpdaterArtifacts`) and offers an in-app "Update & restart". It's most
  polished on Windows (NSIS) and Linux (AppImage, which self-replaces); macOS
  updates need the OS code-signing secrets above to be smooth (otherwise
  Gatekeeper still prompts).
- **Plugin updates**: built in. Settings → Plugins → "Check for updates"
  polls each installed plugin's recorded source (npm `latest` dist-tag via
  `npm view`), and updates re-install through the same path. Pinned sources
  (`npm:pkg@ver`) are skipped by design. See PLUGINS.md.
- **Plugin publish token**: the plugin registry publishes to npm; its token
  lives in `pragma-plugins/.env` (`NPM_TOKEN=…`, gitignored) and is rotated
  with `node scripts/publish.mjs --token npm_<new>`. It has no bearing on app
  releases (the app no longer publishes to npm).
- **CI gates**: every push/PR runs `npm run check`, vitest, `cargo fmt --check`,
  `cargo clippy -D warnings`, and `cargo test` on Linux/macOS/Windows — keep
  those green so a tag push never wastes a release build.
- The dev-only `scripts/dev-app.sh` launcher and `plugins/pragma-dev/` reference
  plugin are intentionally part of the repo (the latter is the architecture
  reference for this codebase, see CLAUDE.md).
