#!/usr/bin/env node
// Ensure the APP version is consistent across package.json, tauri.conf.json,
// Cargo.toml and Cargo.lock — and, optionally, that it equals an expected
// value (e.g. the version derived from a release tag). Exits non-zero on any
// mismatch with a clear message, so a drift fails loudly instead of silently
// producing a mislabeled release.
//
// Usage:
//   node verify-version.mjs            # check all four files agree
//   node verify-version.mjs 1.2.3      # also require the version == 1.2.3
import { readFileSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
const ROOT = join(HERE, '..')
const FILES = [
  'package.json',
  'src-tauri/tauri.conf.json',
  'src-tauri/Cargo.toml',
  'src-tauri/Cargo.lock',
]

function readVersion(file) {
  const raw = readFileSync(join(ROOT, file), 'utf8')
  if (file.endsWith('.json')) return JSON.parse(raw).version
  if (file.includes('Cargo.toml')) return /^version = "([^"]+)"/m.exec(raw)?.[1]
  if (file.includes('Cargo.lock')) return /name = "pragma"\nversion = "([^"]+)"/.exec(raw)?.[1]
  return undefined
}

const expected = process.argv[2]
const versions = {}
let bad = false

for (const f of FILES) {
  versions[f] = readVersion(f)
  if (!versions[f]) {
    console.error(`✗ ${f}: could not read version`)
    bad = true
  }
}
const base = versions['src-tauri/Cargo.toml']
for (const f of FILES) {
  if (bad) break
  if (versions[f] !== base) {
    console.error(`✗ ${f} is ${versions[f]} but Cargo.toml is ${base}`)
    bad = true
  }
}
if (!bad && expected && base !== expected) {
  console.error(`✗ app version is ${base} but expected ${expected}`)
  bad = true
}

if (!bad) {
  console.log(`✓ version ${base} is consistent across all four files${expected ? ` and matches ${expected}` : ''}`)
  process.exit(0)
}
process.exit(1)
