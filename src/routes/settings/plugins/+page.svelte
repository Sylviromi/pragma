<script lang="ts">
  import { plugins, loadPlugins, setEnabled, installFromSource, checkUpdates } from '../../../stores/plugins.svelte'
  import { openPluginFolder } from '../../../lib/tauri'
  import { pushToast } from '../../../stores/toast.svelte'
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import type { PluginUpdateInfo } from '../../../lib/tauri'
  import SettingsHeader from '../../../components/SettingsHeader.svelte'
  import CornerMarks from '../../../components/CornerMarks.svelte'

  // Design v2 plugin page (`Pragma UI Kit v2.dc.html`): search + filter pills
  // over a grid of click-through cards (→ plugin detail), and an "Install a
  // plugin" panel at the bottom. Per-plugin actions (update/uninstall) live
  // on the detail page.

  let query = $state('')
  let filter = $state<'all' | 'on' | 'off' | 'updates'>('all')
  let checking = $state(false)
  let installing = $state(false)
  let showInstall = $state(false)
  let sourceInput = $state('')

  onMount(async () => {
    await loadPlugins()
    // If the startup check hasn't run yet (or failed), check now so the
    // badges render — the store dedupes a concurrent in-flight check.
    if (plugins.checkedAt === null) checkUpdates().catch(() => {})
  })

  const updateFor = (id: string): PluginUpdateInfo | undefined =>
    plugins.updates?.find((u) => u.id === id && u.updateAvailable)

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase()
    return plugins.list.filter((p) => {
      if (filter === 'on' && !p.enabled) return false
      if (filter === 'off' && p.enabled) return false
      if (filter === 'updates' && !updateFor(p.id)) return false
      if (!q) return true
      return (p.name + ' ' + p.id + ' ' + p.description + ' ' + (p.author ?? '')).toLowerCase().includes(q)
    })
  })

  const filters = $derived([
    { key: 'all' as const, label: `All ${plugins.list.length}` },
    { key: 'on' as const, label: 'Enabled' },
    { key: 'off' as const, label: 'Disabled' },
    { key: 'updates' as const, label: 'Updates' },
  ])

  async function checkForUpdates() {
    checking = true
    try {
      const updates = await checkUpdates()
      if (!updates.some((u) => u.updateAvailable)) pushToast('All plugins are up to date', 'success')
    } catch (err) {
      pushToast(String(err), 'error')
    } finally {
      checking = false
    }
  }

  async function browseFolder() {
    const path = await open({ multiple: false, directory: true })
    if (path) sourceInput = path
  }

  async function install() {
    const spec = sourceInput.trim()
    if (!spec) return
    installing = true
    try {
      await installFromSource(spec)
      pushToast('Plugin installed — enabled', 'success')
      sourceInput = ''
      showInstall = false
    } catch (err) {
      pushToast(`Install failed: ${String(err)}`, 'error')
    } finally {
      installing = false
    }
  }
</script>

<div class="page">
  <div class="title-row">
    <SettingsHeader
      kicker="Extensions"
      title="Plugins"
      blurb={'Everything Pragma can do beyond the core loop.\nEach plugin brings its own settings and its own skills.'}
    />
    <div class="header-actions">
      <button class="hdr-btn" onclick={checkForUpdates} disabled={checking}>
        <span class="msr" aria-hidden="true">{checking ? 'progress_activity' : 'sync'}</span>
        {checking ? 'Checking…' : 'Check for updates'}
      </button>
      <button class="hdr-btn" onclick={() => (showInstall = !showInstall)} aria-expanded={showInstall}>
        <span class="msr" aria-hidden="true">add</span>
        Install
      </button>
    </div>
  </div>

  <!-- toolbar: search + filter pills -->
  <div class="toolbar">
    <div class="search">
      <span class="msr" aria-hidden="true">search</span>
      <input
        type="search"
        placeholder="Search plugins"
        bind:value={query}
        aria-label="Search plugins"
      />
    </div>
    <div class="pills" role="group" aria-label="Filter plugins">
      {#each filters as f (f.key)}
        <button
          class="pill"
          class:active={filter === f.key}
          onclick={() => (filter = f.key)}
          aria-pressed={filter === f.key}
        >{f.label}</button>
      {/each}
    </div>
  </div>

  {#if showInstall}
    <div class="install-panel">
      <div class="install-head">
        <span class="msr" aria-hidden="true">deployed_code_update</span>
        <span class="install-title">Install from a source</span>
        <span class="install-note">npm package or local folder — hot-reloads instantly</span>
      </div>
      <div class="install-row">
        <span class="msr" aria-hidden="true">search</span>
        <input
          class="source-input"
          bind:value={sourceInput}
          placeholder="npm:pragma-plugin-…  ·  /path/to/folder"
          aria-label="Plugin source"
          onkeydown={(e) => {
            if (e.key === 'Enter') install()
            if (e.key === 'Escape') {
              showInstall = false
              sourceInput = ''
            }
          }}
        />
        <button class="ghost" onclick={browseFolder} title="Pick a local plugin folder">
          <span class="msr" aria-hidden="true">folder_open</span>Browse…
        </button>
        <button class="go" onclick={install} disabled={installing || !sourceInput.trim()}>
          {installing ? 'Installing…' : 'Install'}
        </button>
      </div>
      <p class="install-hint">
        Every Pragma plugin is an npm package — one plugin per package, publishing with
        <code>npm publish</code>. Dropping a folder into
        <code>~/.config/dev.pragma.app/plugins/</code> works too. Or{' '}
        <a href="/settings/marketplace">browse the marketplace</a>.
      </p>
    </div>
  {/if}

  {#if plugins.loading}
    <div class="empty">Loading…</div>
  {:else if plugins.list.length === 0}
    <div class="empty">
      <span class="msr" aria-hidden="true">extension</span>
      <p>No plugins installed yet.</p>
      <p>Restart and the demo plugin will be bootstrapped automatically.</p>
    </div>
  {:else if filtered.length === 0}
    <div class="empty">
      <span class="msr" aria-hidden="true">search_off</span>
      <p>Nothing matches “{query}”.</p>
    </div>
  {:else}
    <div class="grid">
      {#each filtered as p, i (p.id)}
        {@const up = updateFor(p.id)}
        {@const statusColor = !p.enabled ? 'var(--text-muted)' : up ? 'var(--warning)' : 'var(--success)'}
        <a class="card stagger" style={`--i:${Math.min(i, 10)}`} href={`/settings/plugin/${p.id}`} class:off={!p.enabled}>
          <CornerMarks />

          <div class="card-top">
            <span class="initial" aria-hidden="true">{p.name.slice(0, 1).toUpperCase()}</span>
            <div class="name-col">
              <div class="name">{p.name}</div>
              <div class="ver">v{p.version} · {p.publisher || p.author || 'community'}</div>
            </div>
            <span class="status-dot" style={`background:${statusColor}`} aria-hidden="true"></span>
          </div>
          <p class="desc">{p.description || 'No description'}</p>
          <div class="card-foot">
            <span class="skill-tag">{p.skills.length ? `${p.skills.length} skill${p.skills.length > 1 ? 's' : ''}` : 'no skills'}</span>
            <span class="status-text" style={`color:${statusColor}`}>
              {!p.enabled ? 'disabled' : up ? `update available — v${up.latest}` : 'active'}
            </span>
          </div>
        </a>
      {/each}
    </div>
  {/if}
</div>

<style>
  .title-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
  }
  .header-actions {
    display: flex;
    gap: 8px;
    padding-top: 2px;
    flex: none;
  }
  /* Design header actions: bordered mono uppercase buttons. */
  .hdr-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: 1px solid var(--border-default);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 7px 13px;
    cursor: pointer;
    transition: border-color 0.18s ease, color 0.18s ease;
  }
  .hdr-btn:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--text-primary);
  }
  .hdr-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .hdr-btn .msr {
    font-size: 16px;
    line-height: 1;
  }
  .hdr-btn .msr:first-child { font-size: 15px; }

  /* toolbar: search + filter pills */
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 18px;
    flex-wrap: wrap;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 210px;
    border: 1px solid var(--border-default);
    background: var(--bg-base);
    padding: 7px 11px;
  }
  .search .msr {
    font-size: 16px;
    line-height: 1;
    color: var(--text-muted);
    flex: none;
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    outline: none;
    font-family: var(--font-body);
    font-size: 13px;
    color: var(--text-primary);
  }
  .search input::placeholder {
    color: var(--text-muted);
  }
  .pills {
    display: flex;
    border: 1px solid var(--border-default);
    flex: none;
  }
  .pill {
    padding: 7px 12px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.04em;
    cursor: pointer;
    transition: background 0.18s ease, color 0.18s ease;
  }
  .pill + .pill {
    border-left: 1px solid var(--border-default);
  }
  .pill:hover {
    color: var(--text-primary);
  }
  .pill.active {
    background: var(--accent);
    color: var(--accent-on);
  }

  /* card grid (design pluginCards): auto-fill 250px columns */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 16px;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    text-align: left;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    padding: 0;
    cursor: pointer;
    text-decoration: none;
    transition: border-color 0.2s ease;
  }
  .card:hover {
    border-color: var(--accent);
    /* light the corner marks up with the border */
    --cm-color: var(--accent);
  }
  .card.off {
    opacity: 0.62;
  }
  .card-top {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 13px 13px 0;
  }
  .initial {
    width: 28px;
    height: 28px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border-default);
    background: var(--bg-overlay);
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--accent);
  }
  .card.off .initial {
    color: var(--text-muted);
  }
  .name-col {
    flex: 1;
    min-width: 0;
  }
  .name {
    font-family: var(--font-heading);
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ver {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    margin-top: 2px;
  }
  .status-dot {
    width: 7px;
    height: 7px;
    flex: none;
    margin-top: 6px;
    border-radius: 50%;
  }
  .desc {
    margin: 8px 13px 0;
    font-family: var(--font-body);
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-muted);
    text-wrap: pretty;
    flex: 1;
  }
  .card-foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 13px 12px;
    margin-top: auto;
  }
  .skill-tag {
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    padding: 2px 7px;
    background: var(--bg-overlay);
    color: var(--text-secondary);
  }
  .status-text {
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.05em;
    margin-left: auto;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Design stagger entrance: cards fade up 45ms apart. */
  .stagger {
    animation: rowIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
    animation-delay: calc(var(--i) * 45ms);
  }

  /* install panel (design "Install a plugin" registry section) */
  .install-panel {
    margin: 26px 0 18px;
    border: 1px solid var(--border-default);
    background: var(--bg-elevated);
  }
  .install-head {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 9px 14px;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--bg-overlay);
  }
  .install-head .msr {
    font-size: 15px;
    line-height: 1;
    color: var(--accent);
  }
  .install-title {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-primary);
  }
  .install-note {
    margin-left: auto;
    font-family: var(--font-body);
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .install-row {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--border-default);
    background: var(--bg-base);
    margin: 13px 14px 0;
    padding: 8px 11px;
  }
  .install-row > .msr {
    font-size: 16px;
    line-height: 1;
    color: var(--text-muted);
    flex: none;
  }
  .source-input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    outline: none;
    font-family: var(--font-mono);
    font-size: 12.5px;
    color: var(--text-primary);
  }
  .source-input::placeholder {
    color: var(--text-muted);
  }
  .ghost {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: 1px solid var(--border-default);
    color: var(--text-secondary);
    font-family: var(--font-body);
    font-size: 12.5px;
    padding: 7px 12px;
    cursor: pointer;
    flex: none;
    white-space: nowrap;
    transition: border-color 0.18s ease, color 0.18s ease;
  }
  .ghost:hover {
    border-color: var(--accent);
    color: var(--text-primary);
  }
  .ghost .msr {
    font-size: 15px;
    line-height: 1;
  }
  .go {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--accent);
    color: var(--accent-on);
    border: 1px solid var(--accent);
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 12.5px;
    padding: 8px 14px;
    cursor: pointer;
    flex: none;
    transition: background 0.18s ease;
  }
  .go:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .go:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .install-hint {
    margin: 0;
    padding: 11px 14px 13px;
    font-family: var(--font-body);
    font-size: 12px;
    line-height: 1.55;
    color: var(--text-muted);
    max-width: 66ch;
    text-wrap: pretty;
  }
  .install-hint code {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
  }
  .install-hint a {
    color: var(--accent);
    text-decoration: none;
  }
  .install-hint a:hover {
    text-decoration: underline;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 48px 0;
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .empty .msr {
    font-size: 30px;
    color: var(--text-muted);
  }
  .empty p {
    margin: 0;
  }
</style>
