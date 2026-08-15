#!/usr/bin/env node
// Publish the pragma-app npm wrapper, using the NPM_TOKEN from the plugins
// repo's .env (the single source of truth for the npm credential).
//
// Usage: node scripts/publish-app.mjs
// Override the .env path: PRAGMA_PLUGINS_ENV=/path/to/.env node scripts/publish-app.mjs

import { readFileSync, writeFileSync, rmSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { tmpdir } from 'node:os'
import { execSync } from 'node:child_process'

const HERE = dirname(fileURLToPath(import.meta.url))
const PLUGINS_ENV = process.env.PRAGMA_PLUGINS_ENV || '/mnt/Projects/pragma-plugins/.env'
const PKG_DIR = join(HERE, '..', 'packages', 'pragma-app')

let token
try {
  const text = readFileSync(PLUGINS_ENV, 'utf8')
  token = /^NPM_TOKEN=(.*)$/m.exec(text)?.[1]
} catch {
  /* fall through */
}
if (!token) {
  console.error(`no NPM_TOKEN found in ${PLUGINS_ENV}`)
  process.exit(1)
}

const tmp = join(tmpdir(), `pragma-npmrc-${process.pid}`)
writeFileSync(tmp, `//registry.npmjs.org/:_authToken=${token}\n`, { mode: 0o600 })
try {
  execSync(`npm publish --userconfig ${tmp}`, { cwd: PKG_DIR, stdio: 'inherit' })
} finally {
  rmSync(tmp, { force: true })
}
