#!/usr/bin/env node
// pragma-app — download & launch the latest Pragma release for this platform.
//
// Assets live on GitHub Releases (https://github.com/Sylviromi/pragma/releases).
// The first `pragma` run resolves the latest release via the GitHub API,
// downloads the matching platform asset, caches it under ~/.pragma-app, and
// launches the app. Later runs use the cache. `pragma --update` re-downloads.
'use strict'

const { execFileSync, spawn } = require('node:child_process')
const { chmodSync, existsSync, mkdirSync, statSync, writeFileSync } = require('node:fs')
const { homedir } = require('node:os')
const { join } = require('node:path')

const REPO = 'Sylviromi/pragma'
const CACHE_ROOT = process.env.PRAGMA_CACHE || join(homedir(), '.pragma-app')

// Platform → asset matcher + how to launch it.
//   exec  — chmod +x and run the file directly
//   tar   — extract the .app bundle, run the inner binary
//   open  — hand the file to the OS (dmg mounts and opens)
const TARGETS = {
  'linux-x64': { re: /\.AppImage$/i, kind: 'exec' },
  'darwin-arm64': { re: /aarch64.*\.app\.tar\.gz$/i, kind: 'tar' },
  'darwin-x64': {
    re: /x64.*\.app\.tar\.gz$/i,
    kind: 'tar',
    fallback: { re: /x64.*\.dmg$/i, kind: 'open' }
  },
  'win32-x64': { re: /x64-setup.*\.exe$/i, kind: 'exec' }
}

function platformKey() {
  const p = process.platform
  const a = process.arch
  if (p === 'linux' && a === 'x64') return 'linux-x64'
  if (p === 'darwin' && a === 'arm64') return 'darwin-arm64'
  if (p === 'darwin' && a === 'x64') return 'darwin-x64'
  if (p === 'win32' && (a === 'x64' || a === 'ia32')) return 'win32-x64'
  return null
}

async function latestRelease() {
  const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
    headers: { 'User-Agent': 'pragma-app', Accept: 'application/vnd.github+json' }
  })
  if (res.status === 404) {
    throw new Error(`no published release found — the v0.1.0 release may still be a draft`)
  }
  if (!res.ok) throw new Error(`GitHub API failed (HTTP ${res.status})`)
  return res.json()
}

function pickAsset(release, target) {
  const match = (re) => (release.assets || []).find((a) => re.test(a.name))
  const found = match(target.re) || (target.fallback && match(target.fallback.re))
  const kind = found && target.fallback && target.fallback.re.test(found.name) ? target.fallback.kind : target.kind
  return found ? { asset: found, kind } : null
}

async function download(url, dest, size) {
  console.log(`   downloading ${url}`)
  const res = await fetch(url)
  if (!res.ok) throw new Error(`download failed (HTTP ${res.status})`)
  const buf = Buffer.from(await res.arrayBuffer())
  if (buf.length !== size) throw new Error(`download size mismatch (got ${buf.length}, expected ${size})`)
  writeFileSync(dest, buf)
}

function extractTar(file, dir) {
  console.log(`   extracting ${file}`)
  execFileSync('tar', ['-xzf', file, '-C', dir], { stdio: 'inherit' })
}

function launch(kind, path) {
  console.log('   launching Pragma…')
  if (kind === 'open') {
    const cmd = process.platform === 'darwin' ? 'open' : 'start'
    const child = spawn(cmd, [path], { detached: true, stdio: 'ignore' })
    child.unref()
    return
  }
  const child = spawn(path, [], { detached: true, stdio: 'ignore' })
  child.unref()
}

async function main() {
  const key = platformKey()
  if (!key) {
    console.error(
      `pragma-app does not support ${process.platform}/${process.arch} yet — ` +
        `download the release from https://github.com/${REPO}/releases`
    )
    process.exit(1)
  }
  const target = TARGETS[key]
  const update = process.argv.includes('--update')

  console.log('Pragma — resolving the latest release…')
  const release = await latestRelease()
  const pick = pickAsset(release, target)
  if (!pick) {
    throw new Error(`no asset for ${key} in release ${release.tag_name} — check the release assets`)
  }
  const { asset, kind } = pick

  const dir = join(CACHE_ROOT, release.tag_name)
  if (update && existsSync(dir)) {
    console.log(`   clearing cached ${release.tag_name}`)
    rmSync(dir, { recursive: true, force: true })
  }
  mkdirSync(dir, { recursive: true })
  const file = join(dir, asset.name)
  if (!existsSync(file) || statSync(file).size !== asset.size) {
    await download(asset.browser_download_url, file, asset.size)
  } else {
    console.log(`   using cached ${asset.name}`)
  }

  if (kind === 'tar') {
    extractTar(file, dir)
    launch('exec', join(dir, 'Pragma.app', 'Contents', 'MacOS', 'pragma'))
  } else if (kind === 'exec') {
    chmodSync(file, 0o755)
    launch('exec', file)
  } else {
    launch('open', file)
  }
}

main().catch((e) => {
  console.error(`pragma-app: ${e.message}`)
  process.exit(1)
})
