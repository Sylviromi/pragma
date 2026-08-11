// Pragma Dev — status-bar widget.
// Shows the pragma dev-app/sidecar state (via the skill_sidecar_status /
// call_skill_tool bridge commands) with Run/Stop controls. Plain ES module;
// the app wraps the default export in a custom element and mounts it into the
// `status-bar` slot with data-context = active session working directory.
const STYLES = `
  :host {
    display: inline-flex; align-items: center; gap: 8px;
    font-family: var(--font-mono, monospace);
    font-size: 11px; color: var(--text-secondary, #9aa7b2);
    white-space: nowrap;
  }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: #5a6672; flex: none; }
  .dot.running { background: var(--success, #3fb950); box-shadow: 0 0 6px var(--success, #3fb950); }
  .dot.compiling { background: var(--accent, #58a6ff); animation: blink 1s infinite; }
  .dot.error { background: var(--danger, #f85149); }
  .dot.idle { background: #5a6672; }
  .dot.exited { background: #5a6672; }
  @keyframes blink { 50% { opacity: .3; } }
  .mode { color: var(--accent, #58a6ff); }
  .meta { color: var(--text-secondary, #9aa7b2); }
  .count { color: var(--warning, #d29922); }
  .count.err { color: var(--danger, #f85149); }
  .btn {
    background: var(--bg-elevated, #21262d); color: var(--text-secondary, #e6edf3);
    border: 1px solid var(--border-default, #30363d); border-radius: 4px;
    font: inherit; cursor: pointer; padding: 1px 8px; line-height: 16px;
  }
  .btn:hover { border-color: var(--accent, #1f6feb); }
  .btn:disabled { opacity: .45; cursor: default; }
  .tiny { opacity: .65; }
`

const SKILL = 'pragma-dev/pragma-dev'

export default class PragmaDevStatus extends HTMLElement {
  connectedCallback() {
    this.attachShadow({ mode: 'open' })
    this.shadowRoot.innerHTML = `<style>${STYLES}</style><div class="row"></div>`
    this._row = this.shadowRoot.querySelector('.row')
    this._timer = setInterval(() => this.poll(), 2500)
    this.poll()
  }

  disconnectedCallback() {
    clearInterval(this._timer)
  }

  dir() {
    return this.getAttribute('data-context') || ''
  }

  hasTauri() {
    return typeof window !== 'undefined' && !!window.__TAURI__?.core?.invoke
  }

  async invoke(cmd, args) {
    return window.__TAURI__.core.invoke(cmd, args || {})
  }

  async devStatus() {
    // Auto-ensures the sidecar (spawns the handler) on first call.
    const raw = await this.invoke('call_skill_tool', { skillId: SKILL, name: 'dev_status', arguments: {} })
    try {
      const v = JSON.parse(raw)
      return v && typeof v === 'object' ? v : null
    } catch {
      return null
    }
  }

  async poll() {
    if (!this.hasTauri()) { this._render({ mode: 'mock', state: 'idle' }); return }
    try {
      const st = await this.invoke('skill_sidecar_status')
      const entry = (st || []).find((s) => s.skillId === SKILL)
      if (!entry) { this._render({ mode: 'app', state: 'unloaded' }); return }
      const d = await this.devStatus()
      this._render({ mode: (entry && entry.tools.length) ? 'app' : 'app', state: d?.state || 'starting', ...(d || {}) })
    } catch (e) {
      this._render({ mode: 'app', state: 'error', _err: String(e).slice(0, 80) })
    }
  }

  _render(v) {
    const dir = this.dir()
    const parts = []
    const dot = `<span class="dot ${v.state}"></span>`
    parts.push(dot)
    parts.push(`<span class="mode">pragma·dev</span>`)
    if (v.state === 'unloaded') {
      parts.push(`<span class="tiny">skill not loaded — start to spawn</span>`)
    } else if (v.state === 'error' && v._err) {
      parts.push(`<span class="count err">${v._err}</span>`)
    } else if (v.state === 'mock') {
      parts.push(`<span class="tiny">bridge unavailable (browser mock)</span>`)
    } else {
      const state = v.state || 'idle'
      parts.push(`<span class="meta">${state}</span>`)
      if (v.running) {
        parts.push(`<span class="meta">${v.mode || ''}</span>`)
        if (v.port) parts.push(`<span class="meta">:${v.port}</span>`)
        if (v.errorCount > 0) parts.push(`<span class="count err">err ${v.errorCount}</span>`)
        else if (v.warningCount > 0) parts.push(`<span class="count">warn ${v.warningCount}</span>`)
        parts.push(`<span class="tiny">${v.logLines ?? 0} lines</span>`)
      } else if (v.exited) {
        parts.push(`<span class="tiny">exited ${v.exited.code ?? '?'}</span>`)
      }
    }
    if (!dir) {
      parts.push(`<span class="tiny">no working dir</span>`)
    }
    const canRun = this.hasTauri() && dir && v.state !== 'running' && v.state !== 'starting' && v.state !== 'compiling' && v.state !== 'unloaded'
    const canStop = this.hasTauri() && v.running
    const startBtn = `<button class="btn" id="pb-start" ${canRun ? '' : 'disabled'}>run app</button>`
    const stopBtn = `<button class="btn" id="pb-stop" ${canStop ? '' : 'disabled'}>stop</button>`
    this._row.innerHTML = parts.join(' ') + ` ${startBtn} ${stopBtn}`
    const start = this.shadowRoot.querySelector('#pb-start')
    const stop = this.shadowRoot.querySelector('#pb-stop')
    if (start) start.addEventListener('click', () => this._run('pd_dev_start', { mode: 'app', dir }))
    if (stop) stop.addEventListener('click', () => this._run('pd_dev_stop'))
  }

  async _run(name, args) {
    try {
      await this.invoke('call_skill_tool', { skillId: SKILL, name, arguments: args })
      setTimeout(() => this.poll(), 600)
    } catch (e) {
      this._row.innerHTML = `<span class="count err">${String(e).slice(0, 90)}</span>`
    }
  }
}