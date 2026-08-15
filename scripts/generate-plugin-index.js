#!/usr/bin/env node
// Generate the Pragma plugin registry index for a single-repo marketplace.
//
// Scans <repo>/plugins/*/manifest.json and emits an `index.json` that the app
// can fetch to discover + install every plugin. Each entry's `source` encodes
// everything the installer needs: git URL + per-plugin tag + subpath, e.g.
//
//   git:https://github.com/pragma/plugins@alpha-v1.2.3#plugins/alpha
//
// Usage:
//   node scripts/generate-plugin-index.js [pluginsDir] [owner/repo] [outFile]
//
// Defaults: plugins/  pragma/plugins  index.json
//
// In the plugins repo, wire this into CI so index.json is always fresh:
//
//   on: { push: { branches: [main] } }
//   jobs: { index: { runs-on: ubuntu-latest, steps:
//     - uses: actions/checkout@v4
//     - run: node scripts/generate-plugin-index.js . plugins pragma/plugins index.json
//     - uses: stefanzweifel/git-auto-commit-action@v5 } }

import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const [pluginsDir = 'plugins', repo = 'pragma/plugins', outFile = 'index.json'] = process.argv.slice(2)

function readManifest(dir) {
  try {
    return JSON.parse(readFileSync(join(dir, 'manifest.json'), 'utf8'))
  } catch {
    return null
  }
}

const entries = []
for (const id of readdirSync(pluginsDir)) {
  const dir = join(pluginsDir, id)
  if (!statSync(dir).isDirectory()) continue
  const m = readManifest(dir)
  if (!m || m.id !== id) {
    console.warn(`skip ${id}: missing or invalid manifest.json`)
    continue
  }
  const tag = `${id}-v${m.version}`
  entries.push({
    id: m.id,
    name: m.name,
    version: m.version,
    description: m.description ?? '',
    author: m.author ?? '',
    publisher: m.publisher ?? '',
    repository: m.repository ?? '',
    homepage: m.homepage ?? '',
    // What the app's install_plugin accepts, pinned to this exact version.
    source: `git:https://github.com/${repo}@${tag}#plugins/${id}`,
  })
}

entries.sort((a, b) => a.id.localeCompare(b.id))
const index = {
  $schema: 'https://pragma.dev/schemas/plugin-index.json',
  generatedAt: new Date().toISOString(),
  repo: `https://github.com/${repo}`,
  pluginCount: entries.length,
  plugins: entries,
}

writeFileSync(outFile, JSON.stringify(index, null, 2) + '\n')
console.log(`✓ wrote ${entries.length} plugins to ${outFile}`)
