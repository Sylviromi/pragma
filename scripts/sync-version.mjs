#!/usr/bin/env node
// Read or set the APP version across its four home locations:
//   package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml,
//   src-tauri/Cargo.lock (the pragma [[package]] block).
// The pragma-app npm wrapper keeps its own independent version (see RELEASING.md).
//
// This syncs the lockfile directly (a precise text edit of the `pragma`
// [[package]] block) instead of running a full `cargo check` just to refresh
// the version stamp — so bumping takes milliseconds, not minutes.
//
// Usage:
//   node sync-version.mjs --get            # print current version (from Cargo.toml)
//   node sync-version.mjs --set 1.2.3      # write the version to all four files
import { readFileSync, writeFileSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
const ROOT = join(HERE, '..')
const VER_RE = /^\d+\.\d+\.\d+$/

function readVersion(file) {
  const raw = readFileSync(join(ROOT, file), 'utf8')
  if (file.endsWith('.json')) return JSON.parse(raw).version
  if (file.includes('Cargo.toml')) return /^version = "([^"]+)"/m.exec(raw)?.[1]
  if (file.includes('Cargo.lock')) return /name = "pragma"\nversion = "([^"]+)"/.exec(raw)?.[1]
  throw new Error(`unknown version file: ${file}`)
}

function setVersion(file, next) {
  const p = join(ROOT, file)
  const raw = readFileSync(p, 'utf8')
  let out
  if (file.endsWith('.json')) {
    const j = JSON.parse(raw)
    j.version = next
    out = JSON.stringify(j, null, 2) + '\n'
  } else if (file.includes('Cargo.toml')) {
    out = raw.replace(/^version = "[^"]+"/m, `version = "${next}"`)
  } else if (file.includes('Cargo.lock')) {
    out = raw.replace(
      /(name = "pragma"\nversion = ")[^"]+(")/,
      `$1${next}$2`
    )
  }
  if (out === undefined || out === raw) {
    throw new Error(`could not update version in ${file}`)
  }
  writeFileSync(p, out)
}

const [, , flag, value] = process.argv
if (flag === '--get') {
  console.log(readVersion('src-tauri/Cargo.toml'))
} else if (flag === '--set') {
  if (!VER_RE.test(value)) {
    console.error(`invalid version '${value}' — expected major.minor.patch`)
    process.exit(1)
  }
  ['package.json', 'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock']
    .forEach((f) => setVersion(f, value))
  console.log(`✓ app version set to ${value} (package.json, tauri.conf.json, Cargo.toml, Cargo.lock)`)
} else {
  console.error('usage: sync-version.mjs --get | --set <major.minor.patch>')
  process.exit(1)
}
