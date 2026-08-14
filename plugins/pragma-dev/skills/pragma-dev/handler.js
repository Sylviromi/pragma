// Pragma Dev skill sidecar — build/verify tooling for developing Pragma
// from within itself. JSON-RPC 2.0 over newline-delimited JSON on stdio
// (the sidecar protocol in src-tauri/src/tools/sidecar.rs).
//
// One process owns everything: long-running dev-server child, one-shot
// check/test runners, and repo convention verifiers. Depends only on Node
// built-ins.
'use strict'

import { spawn, spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import readline from 'node:readline'

const __dirname = import.meta.dirname

const HOME = os.homedir()

// ── binary resolution ─────────────────────────────────────────────────────
// The app may launch with a minimal PATH (desktop launcher), so resolve npm /
// cargo against PATH first, then common install locations.
function expandGlobs(p) {
  if (!p.includes('*')) return [p]
  const star = p.indexOf('*')
  const dir = p.slice(0, star)
  const rest = p.slice(star + 1)
  const base = dir.slice(0, dir.lastIndexOf('/'))
  const pat = dir.slice(dir.lastIndexOf('/') + 1)
  const out = []
  try {
    for (const e of fs.readdirSync(base)) {
      if (pat && !e.startsWith(pat.replace('*', ''))) continue
      out.push(path.join(base, e, rest))
    }
  } catch {}
  return out
}

function findBin(name, fallbacks) {
  if (process.env.PATH) {
    for (const d of process.env.PATH.split(path.delimiter)) {
      const p = path.join(d, name + (process.platform === 'win32' ? '.cmd' : ''))
      if (fs.existsSync(p)) return p
    }
  }
  const candidates = []
  for (const f of fallbacks) {
    candidates.push(...expandGlobs(f.replace(/^~/, HOME)))
  }
  for (const p of candidates) {
    if (fs.existsSync(p)) return p
  }
  return name // let spawn fail with a clear ENOENT
}

// npm is a sibling of the node binary in most installs (nvm, brew, fnm).
const npmBin = findBin('npm', [
  path.join(path.dirname(process.execPath), 'npm'),
  '~/.nvm/versions/node/*/bin/npm',
  '/usr/local/bin/npm',
  '/opt/homebrew/bin/npm',
  '/usr/bin/npm',
])
const cargoBin = findBin('cargo', [
  '~/.cargo/bin/cargo',
  '/usr/local/bin/cargo',
  '/opt/homebrew/bin/cargo',
  '/usr/bin/cargo',
])

// ── generic one-shot runner ───────────────────────────────────────────────
// Runs a command to completion with a timeout, capturing full output (capped)
// plus the tail. Resolves { ok, exitCode, timedOut, elapsedSecs, output, tail }.
function runOnce(cmd, args, opts) {
  const { cwd, timeoutSecs = 600, maxChars = 160000 } = opts || {}
  return new Promise((resolve) => {
    const started = Date.now()
    let out = ''
    let killed = false
    const child = spawn(cmd, args, {
      cwd,
      env: process.env,
      stdio: ['ignore', 'pipe', 'pipe'],
      detached: process.platform !== 'win32',
    })
    const timer = setTimeout(() => {
      killed = true
      try {
        if (process.platform === 'win32') {
          spawnSync('taskkill', ['/pid', String(child.pid), '/T', '/F'])
        } else {
          process.kill(-child.pid, 'SIGTERM')
        }
      } catch {}
    }, (timeoutSecs || 600) * 1000)
    child.stdout.on('data', (d) => {
      out += d.toString()
      if (out.length > maxChars) out = out.slice(-maxChars)
    })
    child.stderr.on('data', (d) => {
      out += d.toString()
      if (out.length > maxChars) out = out.slice(-maxChars)
    })
    child.on('error', (e) => {
      clearTimeout(timer)
      resolve({ ok: false, exitCode: null, timedOut: killed, elapsedSecs: (Date.now() - started) / 1000, output: out, tail: tailOf(out), error: e.message })
    })
    child.on('close', (code) => {
      clearTimeout(timer)
      resolve({ ok: !killed && code === 0, exitCode: code, timedOut: killed, elapsedSecs: (Date.now() - started) / 1000, output: out, tail: tailOf(out) })
    })
  })
}

function tailOf(out, n = 300) {
  const lines = out.split('\n')
  return lines.slice(-n).join('\n')
}

// ── long-running dev server ───────────────────────────────────────────────
const LOG_CAP = 2000
const dev = {
  child: null,
  mode: null,
  dir: null,
  startedAt: null,
  logs: [],
  state: 'idle', // idle | starting | compiling | running | error | exited
  errorCount: 0,
  warningCount: 0,
  port: null,
  exited: null, // {code, at}
}

function devLog(line) {
  dev.logs.push(`[${new Date().toISOString().slice(11, 19)}] ${line}`)
  if (dev.logs.length > LOG_CAP) dev.logs.shift()
}

function classifyLine(line) {
  const l = line.trim()
  if (/^error(\[|:)|error: (could not compile|failed|process didn't exit)|Error: |✗/.test(l)) {
    dev.errorCount++
    dev.state = 'error'
  } else if (/^warning:|warning: unused/.test(l)) {
    dev.warningCount++
  } else if (/^Compiling pragma|^Building \[|cargo:warning/.test(l)) {
    dev.state = 'compiling'
  } else if (/Finished `dev` profile/.test(l)) {
    dev.state = 'running'
  } else if (/Running `target\/debug\/pragma`|tauri::.*ready|app has been started/.test(l)) {
    dev.state = 'running'
  } else if (/VITE v|Local:\s+http:\/\/localhost:(\d+)|ready in \d/.test(l)) {
    const m = l.match(/localhost:(\d+)/)
    if (m) dev.port = m[1]
    dev.state = 'running'
  } else if (/✓ built in|built in \d/.test(l)) {
    dev.state = 'running'
  }
}

function startDev(mode, dir) {
  if (dev.child && dev.child.exitCode === null) {
    return { ok: true, alreadyRunning: true, mode: dev.mode, pid: dev.child.pid, dir: dev.dir }
  }
  dev.child = null
  dev.logs = []
  dev.mode = mode
  dev.dir = dir
  dev.startedAt = new Date().toISOString()
  dev.state = 'starting'
  dev.errorCount = 0
  dev.warningCount = 0
  dev.port = null
  dev.exited = null

  const script = mode === 'vite' ? 'dev' : mode === 'tauri' ? 'tauri' : 'dev:app'
  const child = spawn(npmBin, ['run', script], {
    cwd: dir,
    env: process.env,
    stdio: ['ignore', 'pipe', 'pipe'],
    detached: process.platform !== 'win32',
  })
  dev.child = child
  const pipe = (chunk) => {
    const text = chunk.toString()
    for (const line of text.split('\n')) {
      if (line.length) {
        classifyLine(line)
        devLog(line)
      }
    }
  }
  child.stdout.on('data', pipe)
  child.stderr.on('data', pipe)
  child.on('error', (e) => {
    dev.state = 'error'
    dev.errorCount++
    devLog(`spawn error: ${e.message}`)
  })
  child.on('exit', (code) => {
    dev.exited = { code, at: new Date().toISOString() }
    dev.state = 'exited'
    devLog(`dev server exited (code ${code})`)
    if (dev.child === child) dev.child = null
  })
  return { ok: true, pid: child.pid, mode, dir, state: dev.state }
}

function stopDev() {
  const child = dev.child
  if (!child || child.exitCode !== null) {
    dev.state = 'exited'
    return { ok: true, alreadyStopped: true, dir: dev.dir }
  }
  try {
    if (process.platform === 'win32') {
      spawnSync('taskkill', ['/pid', String(child.pid), '/T', '/F'])
    } else {
      process.kill(-child.pid, 'SIGTERM')
    }
  } catch (e) {
    return { ok: false, error: e.message }
  }
  dev.state = 'exited'
  devLog('dev server stopped by request')
  return { ok: true, pid: child.pid, dir: dev.dir }
}

function devStatus() {
  const alive = dev.child && dev.child.exitCode === null
  return {
    ok: true,
    running: !!alive,
    mode: alive ? dev.mode : null,
    dir: alive ? dev.dir : null,
    pid: alive ? dev.child.pid : null,
    startedAt: dev.startedAt,
    state: dev.state,
    port: dev.port,
    errorCount: dev.errorCount,
    warningCount: dev.warningCount,
    logLines: dev.logs.length,
    exited: dev.exited,
  }
}

function devLogs(filter, limit) {
  const n = Math.max(1, Math.min(LOG_CAP, (limit || 100) | 0))
  let lines = dev.logs
  if (filter === 'errors') lines = lines.filter((l) => /error|failed|✗/i.test(l))
  else if (filter === 'warnings') lines = lines.filter((l) => /warning/i.test(l))
  else if (filter === 'vite') lines = lines.filter((l) => /vite|➜|local:|ready in|built in/i.test(l))
  return { ok: true, filter, lines: lines.slice(-n) }
}

// ── repo helpers ──────────────────────────────────────────────────────────
function repoRoot(dir) {
  return path.resolve(dir || sessionDir || process.cwd())
}
function srcTauriDir(dir) {
  return path.join(repoRoot(dir), 'src-tauri')
}

function grep(file, re) {
  if (!fs.existsSync(file)) return []
  const out = []
  const lines = fs.readFileSync(file, 'utf8').split('\n')
  for (let i = 0; i < lines.length; i++) {
    if (re.test(lines[i])) {
      out.push({ line: i + 1, text: lines[i].trim() })
      re.lastIndex = 0
    }
  }
  return out
}

function grepDir(dir, re, maxFiles = 200) {
  const hits = []
  const walk = (d) => {
    if (hits.length >= maxFiles) return
    let entries
    try {
      entries = fs.readdirSync(d, { withFileTypes: true })
    } catch {
      return
    }
    for (const e of entries) {
      if (hits.length >= maxFiles) return
      const p = path.join(d, e.name)
      if (e.isDirectory()) {
        if (/^(node_modules|target|\.git|\.svelte-kit|dist|build|\.turbo)$/.test(e.name)) continue
        walk(p)
      } else if (/\.(rs|ts|svelte|js|json|md)$/.test(e.name)) {
        const lines = fs.readFileSync(p, 'utf8').split('\n')
        for (let i = 0; i < lines.length; i++) {
          if (re.test(lines[i])) {
            hits.push({ file: p, line: i + 1, text: lines[i].trim().slice(0, 140) })
            re.lastIndex = 0
            if (hits.length >= maxFiles) return
          }
        }
      }
    }
  }
  walk(dir)
  return hits
}

// ── tool implementations ──────────────────────────────────────────────────
function parseSvelteCheck(output) {
  const errors = []
  const warnings = []
  const re = /([^\n]+):(\d+):(\d+)\s*\n\s*(Error|Warning):\s*([^\n]*)/g
  let m
  while ((m = re.exec(output)) !== null) {
    const item = { file: path.basename(m[1]), full: m[1], line: +m[2], col: +m[3], message: m[5] }
    if (m[4] === 'Error') errors.push(item)
    else warnings.push(item)
  }
  const summary = output.match(/svelte-check found \d+ error[s]? and \d+ warning[s]?/i)
  return { errors, warnings, summary: summary ? summary[0] : null }
}

function parseCargoJson(stdout) {
  const errors = []
  const warnings = []
  const fresh = []
  for (const line of stdout.split('\n')) {
    let msg
    try {
      msg = JSON.parse(line)
    } catch {
      continue
    }
    if (msg.reason === 'compiler-message' && msg.message) {
      const level = msg.message.level || ''
      const span = (msg.message.spans || []).find((s) => s.is_primary) || (msg.message.spans || [])[0]
      const item = {
        level,
        message: msg.message.message || '',
        file: span ? path.basename(span.file_name) : null,
        full: span ? span.file_name : null,
        line: span ? span.line_start : null,
        col: span ? span.column_start : null,
        code: msg.message.code ? msg.message.code.code : null,
      }
      if (level === 'error') errors.push(item)
      else if (level === 'warning') warnings.push(item)
    } else if (msg.reason === 'compiler-artifact') {
      fresh.push(msg)
    }
  }
  return { errors, warnings, freshCount: fresh.length }
}

function cargoFailure(stdout, stderr) {
  const m = stderr.match(/error: could not compile `([^`]+)`/)
  if (m) return `could not compile \`${m[1]}\``
  const m2 = stderr.match(/error: (.*)/)
  if (m2) return m2[1]
  return null
}

async function pdCheck(args) {
  const dir = repoRoot(args.dir)
  const r = await runOnce(npmBin, ['run', 'check'], { cwd: dir, timeoutSecs: args.timeoutSecs || 300, maxChars: 200000 })
  const parsed = parseSvelteCheck(r.output)
  const ok = r.ok && parsed.errors.length === 0
  return {
    ok,
    exitCode: r.exitCode,
    timedOut: r.timedOut,
    elapsedSecs: +r.elapsedSecs.toFixed(1),
    summary: parsed.summary,
    errors: parsed.errors.slice(0, 50),
    warnings: parsed.warnings.slice(0, 30),
    errorCount: parsed.errors.length,
    warningCount: parsed.warnings.length,
    tail: tailOf(r.output, 120),
  }
}

async function pdCargoCheck(args) {
  const dir = srcTauriDir(args.dir)
  const r = await runOnce(cargoBin, ['check', '--message-format=json'], {
    cwd: dir,
    timeoutSecs: args.timeoutSecs || 600,
    maxChars: 300000,
  })
  const parsed = parseCargoJson(r.output)
  const fail = cargoFailure(r.output, r.output)
  const ok = r.ok && parsed.errors.length === 0
  return {
    ok,
    exitCode: r.exitCode,
    timedOut: r.timedOut,
    elapsedSecs: +r.elapsedSecs.toFixed(1),
    errorCount: parsed.errors.length,
    warningCount: parsed.warnings.length,
    errors: parsed.errors.slice(0, 40),
    warnings: parsed.warnings.slice(0, 20),
    failure: fail,
    tail: tailOf(r.output, 120),
  }
}

async function pdTest(args) {
  const dir = repoRoot(args.dir)
  const r = await runOnce(npmBin, ['run', 'test'], { cwd: dir, timeoutSecs: args.timeoutSecs || 300, maxChars: 160000 })
  const failed = []
  const re = /(?:×|✗|FAIL)\s+([^\n]+)|failed:?\s+(\d+)/g
  let m
  while ((m = re.exec(r.output)) !== null) {
    if (m[1]) failed.push(m[1].trim())
  }
  const summary = r.output.match(/Test Files\s+(\d+) (passed|failed)|Tests\s+(\d+) (passed|failed)/g)
  return {
    ok: r.ok,
    exitCode: r.exitCode,
    timedOut: r.timedOut,
    elapsedSecs: +r.elapsedSecs.toFixed(1),
    summary: summary ? summary.join('; ') : null,
    failed: failed.slice(0, 40),
    tail: tailOf(r.output, 150),
  }
}

async function pdCargoTest(args) {
  const dir = srcTauriDir(args.dir)
  const r = await runOnce(cargoBin, ['test'], { cwd: dir, timeoutSecs: args.timeoutSecs || 900, maxChars: 300000 })
  const failed = []
  const re = /test ([^\s]+) \.\.\. FAILED/g
  let m
  while ((m = re.exec(r.output)) !== null) failed.push(m[1])
  const summary = r.output.match(/test result: [^\n]+/g)
  const failSummary = r.output.match(/failures:\n([\s\S]*?)\n\n/) || r.output.match(/failures:\n([\s\S]*?)(?=\n\ntest result|$)/)
  return {
    ok: r.ok,
    exitCode: r.exitCode,
    timedOut: r.timedOut,
    elapsedSecs: +r.elapsedSecs.toFixed(1),
    summary: summary ? summary.join(' | ') : null,
    failed,
    failureDetail: failSummary ? failSummary[1].trim().slice(0, 4000) : null,
    tail: tailOf(r.output, 120),
  }
}

async function pdReloadApp(args) {
  const dir = repoRoot(args.dir)
  let manifest = path.join(dir, 'src-tauri', 'Cargo.toml')
  if (!fs.existsSync(manifest)) manifest = path.join(dir, 'Cargo.toml')
  if (!fs.existsSync(manifest)) {
    return { ok: false, reloadApp: false, error: `no Cargo manifest found under ${dir}` }
  }
  const r = await runOnce(cargoBin, ['build', '--manifest-path', manifest], {
    cwd: dir,
    timeoutSecs: args.timeoutSecs || 900,
    maxChars: 300000,
  })
  const tail = tailOf(r.output, 200)
  if (r.ok) {
    return {
      ok: true,
      reloadApp: true, // relaunch convention — the host exits(23) after this run
      exitCode: r.exitCode,
      elapsedSecs: +r.elapsedSecs.toFixed(1),
      message: 'Build succeeded — a full app reload was requested; the app will restart with the new Rust code, so this run ends here. Your conversation is persisted and will be restored after the restart.',
    }
  }
  const errTail = tail.split('\n').slice(-40).join('\n')
  return { ok: false, reloadApp: false, exitCode: r.exitCode, timedOut: r.timedOut, elapsedSecs: +r.elapsedSecs.toFixed(1), error: `cargo build FAILED:\n${errTail}` }
}

function pdVerifyCommand(args) {
  const { name } = args
  if (!name) return { ok: false, error: 'missing "name" — the command to verify' }
  const root = repoRoot(args.dir)
  const results = []
  const impl = grepDir(path.join(root, 'src-tauri/src/commands'), new RegExp(`pub (async )?fn ${name}\\b`))
  if (impl.length) {
    results.push({ check: 'implementation', ok: true, evidence: `${path.relative(root, impl[0].file)}:${impl[0].line} — ${impl[0].text}` })
  } else {
    results.push({ check: 'implementation', ok: false, evidence: `no "pub async fn ${name}" under src-tauri/src/commands/` })
  }
  // module wiring
  const modFile = impl.length ? path.relative(path.join(root, 'src-tauri/src/commands'), impl[0].file) : null
  const modName = modFile ? modFile.replace(/\.rs$/, '') : null
  if (modName) {
    const inMod = grep(path.join(root, 'src-tauri/src/commands/mod.rs'), new RegExp(`pub mod ${modName}\\b`))
    results.push(inMod.length
      ? { check: 'commands/mod.rs', ok: true, evidence: `mod.rs:${inMod[0].line} — ${inMod[0].text}` }
      : { check: 'commands/mod.rs', ok: false, evidence: `missing "pub mod ${modName};" in commands/mod.rs` })
    const inHandler = grep(path.join(root, 'src-tauri/src/lib.rs'), new RegExp(`commands::${modName}::${name}\\b`))
    results.push(inHandler.length
      ? { check: 'lib.rs generate_handler!', ok: true, evidence: `lib.rs:${inHandler[0].line} — ${inHandler[0].text}` }
      : { check: 'lib.rs generate_handler!', ok: false, evidence: `missing "commands::${modName}::${name}" in lib.rs invoke_handler` })
  }
  const inTs = grep(path.join(root, 'src/lib/tauri.ts'), new RegExp('[\'"`]' + name + '[\'"`]'))
  results.push(inTs.length
    ? { check: 'tauri.ts wrapper', ok: true, evidence: `tauri.ts:${inTs[0].line} — ${inTs[0].text}` }
    : { check: 'tauri.ts wrapper', ok: false, evidence: `no tauri.ts wrapper invoking '${name}'` })
  const allOk = results.every((r) => r.ok)
  return { ok: allOk, name, results }
}

function pdVerifyTool(args) {
  const { name } = args
  if (!name) return { ok: false, error: 'missing "name" — the tool to verify' }
  const root = repoRoot(args.dir)
  const results = []
  const re = new RegExp(`name:\\s*"${name}"`)
  const def = grep(path.join(root, 'src-tauri/src/tools/definitions.rs'), re)
  results.push(def.length
    ? { check: 'tool definition', ok: true, evidence: `definitions.rs:${def[0].line} — ${def[0].text}` }
    : { check: 'tool definition', ok: false, evidence: `no ToolDefinition named "${name}" in tools/definitions.rs` })
  const armRe = new RegExp(`"${name}"\\s*=>|"${name}"\\s*\\|`)
  const inExec = grep(path.join(root, 'src-tauri/src/tools/mod.rs'), armRe)
    .concat(grep(path.join(root, 'src-tauri/src/commands/agentic.rs'), armRe))
  results.push(inExec.length
    ? { check: 'executor dispatch', ok: true, evidence: `${inExec[0].file === path.join(root, 'src-tauri/src/tools/mod.rs') ? 'tools/mod.rs' : 'agentic.rs'}:${inExec[0].line} — ${inExec[0].text}` }
    : { check: 'executor dispatch', ok: false, evidence: `no match arm for "${name}" in tools/mod.rs or agentic.rs` })
  const safe = grep(path.join(root, 'src/stores/chat.svelte.ts'), new RegExp('[\'"`]' + name + '[\'"`]'))
  results.push(safe.length
    ? { check: 'SAFE_TOOLS (read-only set)', ok: true, evidence: `chat.svelte.ts:${safe[0].line}`, note: 'listed in SAFE_TOOLS — assumed read-only; remove if the tool mutates state' }
    : { check: 'SAFE_TOOLS (read-only set)', ok: true, evidence: 'not in SAFE_TOOLS — will require approval (fine for mutating tools)' })
  const allOk = results.every((r) => r.ok)
  return { ok: allOk, name, results }
}

function pdTodo(args) {
  const root = repoRoot(args.dir)
  const hits = grepDir(path.join(root, 'src'), /TODO|FIXME|HACK|XXX/)
    .concat(grepDir(path.join(root, 'src-tauri/src'), /TODO|FIXME|HACK|XXX/))
  hits.sort((a, b) => a.file.localeCompare(b.file))
  const limit = Math.max(1, (args.limit || 100) | 0)
  const total = hits.length
  const shown = hits.slice(0, limit)
  const byFile = {}
  for (const h of hits) {
    const rel = path.relative(root, h.file)
    byFile[rel] = (byFile[rel] || 0) + 1
  }
  return {
    ok: true,
    total,
    truncated: total > limit,
    byFile,
    items: shown.map((h) => `${path.relative(root, h.file)}:${h.line} — ${h.text}`),
  }
}

function pdArch(args) {
  const root = repoRoot(args.dir)
  const out = {}
  // Tauri commands from generate_handler! in lib.rs
  const lib = fs.readFileSync(path.join(root, 'src-tauri/src/lib.rs'), 'utf8')
  const genMatch = lib.match(/generate_handler!\[([\s\S]*?)\]/)
  if (genMatch) {
    out.tauriCommands = genMatch[1]
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
      .sort()
  } else {
    out.tauriCommands = []
  }
  // tools from definitions.rs
  const def = fs.readFileSync(path.join(root, 'src-tauri/src/tools/definitions.rs'), 'utf8')
  out.coreTools = [...def.matchAll(/name:\s*"([a-z_]+)"/g)].map((m) => m[1])
  // components + routes
  try {
    out.components = fs.readdirSync(path.join(root, 'src/components')).filter((f) => f.endsWith('.svelte')).sort()
  } catch {
    out.components = []
  }
  try {
    out.routes = fs.readdirSync(path.join(root, 'src/routes'), { recursive: true })
      .filter((f) => typeof f === 'string' && (f.endsWith('+page.svelte') || f.endsWith('+layout.svelte')))
      .sort()
  } catch {
    out.routes = []
  }
  return { ok: true, dir: root, ...out }
}

// ── JSON-RPC plumbing ─────────────────────────────────────────────────────
let sessionDir = null

function reply(id, payload, isError) {
  const result = {
    content: [{ type: 'text', text: typeof payload === 'string' ? payload : JSON.stringify(payload, null, 2) }],
    isError: !!isError,
  }
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\n')
}

function replyErr(id, message) {
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, error: { code: -1, message } }) + '\n')
}

const rl = readline.createInterface({ input: process.stdin })
rl.on('line', async (line) => {
  let msg
  try {
    msg = JSON.parse(line)
  } catch {
    return
  }
  const { id } = msg
  if (msg.method === 'initialize') {
    sessionDir = (msg.params && msg.params.workingDir) || null
    try {
      const tools = JSON.parse(fs.readFileSync(path.join(__dirname, 'tools.json'), 'utf8'))
      reply(id, { protocolVersion: 1, tools }, false)
    } catch (e) {
      reply(id, { protocolVersion: 1, tools: [] }, false)
    }
  } else if (msg.method === 'tools/call') {
    const { name, arguments: args } = msg.params
    const a = args || {}
    try {
      switch (name) {
        case 'pd_dev_start': {
          const dir = a.dir ? repoRoot(a.dir) : sessionDir
          if (!dir) return reply(id, { ok: false, error: 'no working directory — call change_directory first, or pass "dir"' }, true)
          const mode = a.mode === 'vite' ? 'vite' : a.mode === 'tauri' ? 'tauri' : 'app'
          reply(id, startDev(mode, dir), false)
          break
        }
        case 'pd_dev_status':
          reply(id, devStatus(), false)
          break
        case 'pd_dev_logs':
          reply(id, devLogs(a.filter, a.limit), false)
          break
        case 'pd_dev_stop':
          reply(id, stopDev(), false)
          break
        case 'pd_check':
          reply(id, await pdCheck(a), false)
          break
        case 'pd_cargo_check':
          reply(id, await pdCargoCheck(a), false)
          break
        case 'pd_test':
          reply(id, await pdTest(a), false)
          break
        case 'pd_cargo_test':
          reply(id, await pdCargoTest(a), false)
          break
        case 'pd_reload_app':
          reply(id, await pdReloadApp(a), false)
          break
        case 'pd_reload_frontend':
          // Frontend-only reload: the host reloads the webview at the end of
          // the run (marker `reloadFrontend`), leaving the Rust process and
          // the loop untouched — unlike pd_reload_app's full relaunch.
          reply(id, { ok: true, reloadFrontend: true }, false)
          break
        case 'pd_verify_command':
          reply(id, pdVerifyCommand(a), false)
          break
        case 'pd_verify_tool':
          reply(id, pdVerifyTool(a), false)
          break
        case 'pd_todo':
          reply(id, pdTodo(a), false)
          break
        case 'pd_arch':
          reply(id, pdArch(a), false)
          break
        default:
          replyErr(id, `unknown tool: ${name}`)
      }
    } catch (e) {
      replyErr(id, `handler error: ${e.message}`)
    }
  }
})

process.on('exit', () => {
  if (dev.child && dev.child.exitCode === null) {
    try {
      if (process.platform === 'win32') spawnSync('taskkill', ['/pid', String(dev.child.pid), '/T', '/F'])
      else process.kill(-dev.child.pid, 'SIGTERM')
    } catch {}
  }
})
