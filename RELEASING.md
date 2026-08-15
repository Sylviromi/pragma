# Releasing Pragma

Pragma's release pipeline is fully automated: pushing a version tag makes
GitHub Actions build the native bundles for every platform and attach them to a
**draft** GitHub Release, which you review and publish.

## Prerequisites (first release only)

1. **Add a `LICENSE`** file (MIT / Apache-2.0 / … — your call) and reference it
   from `package.json` (`"license": "MIT"`), `Cargo.toml` (`license = "MIT"`),
   and `tauri.conf.json` → `bundle.copyright` if you want the string baked into
   the installers.
2. Make sure the repo is on GitHub with **Actions enabled**.
3. (Optional) Customize `bundle` metadata in `src-tauri/tauri.conf.json`
   (`publisher`, `category`, `deb.maintainer` — deb/rpm packaging reads these;
   the current fallbacks are generic and fine for a first release).

## Release steps

```bash
# 1. Decide the version (semver). For a first public release: 1.0.0.

# 2. Sync the version everywhere (package.json + tauri.conf.json + Cargo.toml).
./scripts/bump-version.sh 1.0.0

# 3. Update CHANGELOG.md — move "Unreleased" into a dated section
#    (keep the format: ## [1.0.0] - 2025-XX-XX).

# 4. Commit and tag. Tags must match v* (workflow trigger).
git add -A && git commit -m "Release 1.0.0"
git tag v1.0.0
git push origin main --tags
```

## What happens next

- The **Release** workflow builds four bundles:
  Linux `.deb` / `.rpm` / `.AppImage` (ubuntu-22.04), macOS `.dmg` +
  `.app` for both Apple Silicon and Intel, Windows `.msi` / `.exe` (NSIS).
- Everything lands on a **draft release** named `Pragma v1.0.0`.
- Smoke-test the artifacts (see below), then hit **Publish** on the release.

## Smoke-test checklist

- [ ] Linux `.AppImage`: runs from a clean machine (no system deps needed)
- [ ] Linux `.deb`: installs with `sudo apt install ./pragma_1.0.0_amd64.deb`,
      launches from the app menu
- [ ] macOS `.dmg`: drag-to-Applications install works, Gatekeeper prompt
      appears (unsigned builds — fine for a first release; see below)
- [ ] Windows `.msi`: installs, app launches
- [ ] In-app: create a session, run an agentic loop against a provider, click
      an agent link → opens in the system browser

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
  polls each installed plugin's recorded source (git tags via `ls-remote`,
  npm registry `latest`), and updates re-install through the same atomic
  path. Pinned sources (`@tag`/`@ver`) are skipped by design. See PLUGINS.md.
- **CI gates**: every push/PR runs `npm run check`, vitest, `cargo fmt --check`,
  `cargo clippy -D warnings`, and `cargo test` on Linux/macOS/Windows — keep
  those green so a tag push never wastes a release build.
- The dev-only `scripts/dev-app.sh` launcher and `plugins/pragma-dev/` reference
  plugin are intentionally part of the repo (the latter is the architecture
  reference for this codebase, see CLAUDE.md).
