<script lang="ts">
  import { plugins, loadPlugins, installFromSource } from '../../../stores/plugins.svelte'
  import { pushToast } from '../../../stores/toast.svelte'
  import { onMount } from 'svelte'
  import Icon from '../../../components/Icon.svelte'

  interface RegistryPlugin {
    id: string
    name: string
    version: string
    description?: string
    author?: string
    publisher?: string
    source: string
  }
  interface Registry {
    url: string
    label: string
    plugins: RegistryPlugin[]
    loading: boolean
    error: string | null
  }

  // The official first-party registry: pragma/plugins hosts all builtin
  // plugins; any user can add their own repo as another registry.
  const DEFAULT_REGISTRY = 'https://raw.githubusercontent.com/pragma/plugins/main/index.json'
  const STORAGE_KEY = 'pragma.marketplace.registries'

  function labelFor(url: string): string {
    try {
      const raw = /raw\.githubusercontent\.com\/([^/]+\/[^/]+)\//.exec(url)
      if (raw) return raw[1]
      const gh = /github\.com\/([^/]+\/[^/]+)/.exec(url)
      if (gh) return gh[1]
      return new URL(url).hostname
    } catch {
      return url.slice(0, 40)
    }
  }

  let registries = $state<Registry[]>([])
  let showAdd = $state(false)
  let newUrl = $state('')
  let search = $state('')
  let installingId = $state<string | null>(null)
  let refreshing = $state(false)

  onMount(() => {
    loadPlugins()
    let saved: string | null = null
    try {
      saved = localStorage.getItem(STORAGE_KEY)
    } catch {
      /* storage unavailable (e.g. restricted webview) — use the default */
    }
    if (saved) {
      try {
        const urls: string[] = JSON.parse(saved)
        if (urls.length > 0) {
          registries = urls.map((url) => ({
            url,
            label: labelFor(url),
            plugins: [],
            loading: false,
            error: null
          }))
          refreshAll()
          return
        }
      } catch {
        /* corrupt storage — fall through to the default */
      }
    }
    registries = [
      { url: DEFAULT_REGISTRY, label: labelFor(DEFAULT_REGISTRY), plugins: [], loading: false, error: null }
    ]
    refreshAll()
  })

  function persist() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(registries.map((r) => r.url)))
    } catch {
      /* storage unavailable — registries just won't survive restart */
    }
  }

  async function loadRegistry(r: Registry) {
    r.loading = true
    r.error = null
    try {
      const res = await fetch(r.url)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json()
      r.plugins = Array.isArray(data.plugins) ? data.plugins : []
      if (r.plugins.length === 0) r.error = 'No plugins found in this index'
    } catch (err) {
      r.error = String(err)
      r.plugins = []
    } finally {
      r.loading = false
    }
  }

  async function refreshAll() {
    refreshing = true
    try {
      await Promise.all(registries.map(loadRegistry))
    } finally {
      refreshing = false
    }
  }

  function addRegistry() {
    const url = newUrl.trim()
    if (!url) return
    if (registries.some((r) => r.url === url)) {
      pushToast('That registry is already added', 'info')
      return
    }
    const r: Registry = { url, label: labelFor(url), plugins: [], loading: false, error: null }
    registries = [...registries, r]
    persist()
    newUrl = ''
    showAdd = false
    loadRegistry(r)
  }

  function removeRegistry(url: string) {
    if (registries.length <= 1) {
      pushToast("Can't remove the last registry", 'info')
      return
    }
    registries = registries.filter((r) => r.url !== url)
    persist()
  }

  // All plugins across every registry, deduped by id (first registry wins).
  const allPlugins = $derived.by(() => {
    const seen = new Set<string>()
    const out: (RegistryPlugin & { registry: string })[] = []
    for (const r of registries) {
      for (const p of r.plugins) {
        if (seen.has(p.id)) continue
        seen.add(p.id)
        out.push({ ...p, registry: r.label })
      }
    }
    return out
  })

  const filtered = $derived.by(() => {
    const q = search.trim().toLowerCase()
    if (!q) return allPlugins
    return allPlugins.filter((p) =>
      [p.name, p.id, p.description ?? '', p.author ?? ''].join(' ').toLowerCase().includes(q)
    )
  })

  const installed = $derived(new Map(plugins.list.map((p) => [p.id, p.version])))

  async function install(p: RegistryPlugin) {
    installingId = p.id
    try {
      await installFromSource(p.source)
      pushToast(`Installed ${p.name} — enabled`, 'success')
    } catch (err) {
      pushToast(`Install failed: ${String(err)}`, 'error')
    } finally {
      installingId = null
    }
  }
</script>

<div class="page">
  <div class="title-row">
    <div class="title">Marketplace</div>
    <button class="install-btn" onclick={refreshAll} disabled={refreshing}>
      <Icon name="sync" size={14} />
      {refreshing ? 'Refreshing…' : 'Refresh'}
    </button>
  </div>
  <div class="desc-lead">
    Every plugin available across your registries. A registry is any git repo
    hosting one or more plugins — the official <code>pragma/plugins</code> repo
    is pre-added; add your own (or a friend's) with “Add registry”.
  </div>

  <div class="registries">
    {#each registries as r (r.url)}
      <div class="registry" class:error={!!r.error}>
        <span class="registry-label">{r.label}</span>
        <span class="registry-meta">
          {r.loading ? 'loading…' : (r.error ?? `${r.plugins.length} plugin${r.plugins.length === 1 ? '' : 's'}`)}
        </span>
        {#if registries.length > 1}
          <button class="registry-remove" onclick={() => removeRegistry(r.url)} title="Remove registry">
            <Icon name="close" size={13} />
          </button>
        {/if}
      </div>
    {/each}
    {#if showAdd}
      <div class="add-row">
        <input
          class="source-input"
          bind:value={newUrl}
          placeholder="https://raw.githubusercontent.com/you/my-plugins/main/index.json"
          onkeydown={(e) => {
            if (e.key === 'Enter') addRegistry()
          }}
        />
        <button class="btn" onclick={addRegistry} disabled={!newUrl.trim()}>Add</button>
      </div>
    {/if}
    <button class="link add-link" onclick={() => (showAdd = !showAdd)}>
      <Icon name="add" size={14} />
      {showAdd ? 'Cancel' : 'Add registry'}
    </button>
  </div>

  <input class="search" bind:value={search} placeholder="Search plugins…" />

  {#if filtered.length === 0}
    <div class="empty">
      <Icon name="extension" size={30} />
      <p>{search ? 'No plugins match your search.' : 'No plugins loaded — refresh or add a registry.'}</p>
    </div>
  {:else}
    <ul class="grid">
      {#each filtered as p (p.id)}
        <li class="card">
          <div class="card-head">
            <span class="name">{p.name}</span>
            {#if p.publisher === 'pragma'}<span class="badge first-party" title="Built into Pragma">Pragma</span>{/if}
            <span class="ver">v{p.version}</span>
          </div>
          <div class="card-id">{p.id} · {p.registry}</div>
          {#if p.description}<p class="desc">{p.description}</p>{/if}
          <div class="card-foot">
            {#if installed.has(p.id)}
              <span class="installed-tag"><Icon name="check" size={13} /> Installed v{installed.get(p.id)}</span>
            {:else}
              <button class="btn" onclick={() => install(p)} disabled={installingId === p.id}>
                {installingId === p.id ? 'Installing…' : 'Install'}
              </button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .page {
    max-width: 860px;
  }
  .title {
    font-family: var(--font-heading);
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 2px;
  }
  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 2px;
  }
  .install-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-elevated);
    color: var(--text-primary);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    padding: 5px 12px;
    font-size: 12px;
    font-family: var(--font-heading);
    font-weight: 600;
    cursor: pointer;
  }
  .install-btn:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }
  .install-btn:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .desc-lead {
    color: var(--text-muted);
    font-size: 12.5px;
    margin-bottom: 16px;
  }
  .desc-lead code {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-overlay);
    padding: 1px 5px;
    border-radius: 2px;
  }
  .registries {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 14px;
  }
  .registry {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 4px;
    padding: 5px 10px;
    font-size: 12px;
  }
  .registry.error {
    border-color: color-mix(in srgb, var(--danger, #e5484d) 45%, transparent);
  }
  .registry-label {
    font-family: var(--font-mono);
    font-weight: 600;
    color: var(--text-primary);
  }
  .registry-meta {
    color: var(--text-muted);
    font-size: 11px;
  }
  .registry-remove {
    display: flex;
    align-items: center;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 2px;
    border-radius: 2px;
  }
  .registry-remove:hover {
    color: var(--danger, #e5484d);
    background: color-mix(in srgb, var(--danger, #e5484d) 10%, transparent);
  }
  .add-link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--accent);
    cursor: pointer;
  }
  .add-row {
    display: flex;
    gap: 8px;
    flex: 1 1 100%;
  }
  .source-input {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-primary);
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    padding: 7px 10px;
  }
  .source-input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .search {
    width: 100%;
    max-width: 420px;
    font-size: 13px;
    color: var(--text-primary);
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    padding: 7px 12px;
    margin-bottom: 16px;
  }
  .search:focus {
    outline: none;
    border-color: var(--accent);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 48px 0;
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .empty p {
    margin: 0;
  }
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 10px;
  }
  .card {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .card-head {
    display: flex;
    align-items: baseline;
    gap: 7px;
    min-width: 0;
  }
  .name {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ver {
    font-size: 11px;
    color: var(--text-muted);
    font-family: var(--font-mono);
    margin-left: auto;
  }
  .badge {
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    border-radius: 2px;
    padding: 1px 6px;
    flex: none;
  }
  .badge.first-party {
    color: var(--accent);
    border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .card-id {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .desc {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.45;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    min-height: 52px;
  }
  .card-foot {
    margin-top: auto;
    display: flex;
    justify-content: flex-end;
    padding-top: 4px;
  }
  .btn {
    background: var(--accent);
    color: var(--accent-on);
    border: none;
    border-radius: var(--radius-sm);
    padding: 5px 14px;
    font-size: 12px;
    font-family: var(--font-heading);
    font-weight: 600;
    cursor: pointer;
  }
  .btn:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .installed-tag {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--success);
    font-family: var(--font-mono);
  }
</style>
