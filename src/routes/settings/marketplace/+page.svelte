<script lang="ts">
  import { plugins, loadPlugins, installFromSource } from '../../../stores/plugins.svelte'
  import { pushToast } from '../../../stores/toast.svelte'
  import { onMount } from 'svelte'
  import Icon from '../../../components/Icon.svelte'

  // Plugins are npm packages (one plugin = one package, tagged pragma-plugin).
  // Discovery is the npm registry search API — no git, no index files.
  const DEFAULT_QUERY = 'keywords:pragma-plugin'
  const SEARCH_URL = 'https://registry.npmjs.org/-/v1/search'

  interface NpmPlugin {
    name: string
    displayName: string
    version: string
    description?: string
    author?: string
    source: string // npm:<name>
  }

  let query = $state(DEFAULT_QUERY)
  let results = $state<NpmPlugin[]>([])
  let searching = $state(false)
  let searched = $state(false)
  let installingId = $state<string | null>(null)

  onMount(() => {
    loadPlugins()
    search()
  })

  async function search() {
    const term = query.trim()
    // The keyword qualifier is ALWAYS applied, so the marketplace never
    // wanders into unrelated npm packages — the input is free text, not a
    // raw npm query. Results are additionally gated on the pragma-plugin-
    // naming convention, so only real plugins can ever be listed.
    const q = term ? `${term} keywords:pragma-plugin` : DEFAULT_QUERY
    searching = true
    try {
      const res = await fetch(`${SEARCH_URL}?text=${encodeURIComponent(q)}&size=50`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json()
      results = (data.objects ?? [])
        .map((o: { package: any }) => o.package)
        .filter((p: any) => /(^|\/)pragma-plugin-/.test(p.name))
        .map((p: any) => ({
          name: p.name,
          displayName: p.name.replace(/^(?:.*\/)?pragma-plugin-/, ''),
          version: p.version,
          description: p.description ?? '',
          author: typeof p.author === 'string' ? p.author : (p.author?.name ?? ''),
          source: `npm:${p.name}`
        }))
      searched = true
    } catch (err) {
      pushToast(`Search failed: ${String(err)}`, 'error')
      searched = true // leave the loading state — empty state with toast explains
    } finally {
      searching = false
    }
  }

  // Installed detection by recorded source (npm:name) — the manifest id and
  // the npm package name are separate, so match on the source.
  const installed = $derived(
    new Map(plugins.list.filter((p) => p.source).map((p) => [p.source as string, p.version]))
  )

  async function install(p: NpmPlugin) {
    installingId = p.name
    try {
      await installFromSource(p.source)
      pushToast(`Installed ${p.displayName} — enabled`, 'success')
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
    <span class="hint">npm packages tagged <code>pragma-plugin</code></span>
  </div>
  <div class="desc-lead">
    Every Pragma plugin is an <code>npm</code> package — one plugin per package,
    publishing with <code>npm publish</code>. Search below, install with one click;
    updates are checked automatically like any other source. Searches are pinned to
    Pragma plugins (packages named <code>pragma-plugin-*</code> and tagged
    <code>pragma-plugin</code>) — unrelated npm packages never show up.
  </div>

  <div class="search-row">
    <input
      class="search"
      bind:value={query}
      placeholder="Search plugins…"
      onkeydown={(e) => {
        if (e.key === 'Enter') search()
      }}
    />
    <button class="btn" onclick={search} disabled={searching}>
      {searching ? 'Searching…' : 'Search'}
    </button>
  </div>

  {#if !searched && !searching}
    <div class="empty">
      <Icon name="extension" size={30} />
      <p>Searching npm for plugins…</p>
    </div>
  {:else if results.length === 0}
    <div class="empty">
      <Icon name="extension" size={30} />
      <p>No plugins found — searches are limited to <code>pragma-plugin-*</code> packages.</p>
    </div>
  {:else}
    <ul class="grid">
      {#each results as p (p.name)}
        <li class="card">
          <div class="card-head">
            <span class="name">{p.displayName}</span>
            <span class="ver">v{p.version}</span>
          </div>
          <div class="card-id">{p.name}{p.author ? ` · by ${p.author}` : ''}</div>
          {#if p.description}<p class="desc">{p.description}</p>{/if}
          <div class="card-foot">
            {#if installed.has(p.source)}
              <span class="installed-tag"><Icon name="check" size={13} /> Installed v{installed.get(p.source)}</span>
            {:else}
              <button class="btn" onclick={() => install(p)} disabled={installingId === p.name}>
                {installingId === p.name ? 'Installing…' : 'Install'}
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
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
  }
  .hint {
    font-size: 11px;
    color: var(--text-muted);
  }
  .hint code {
    font-family: var(--font-mono);
    font-size: 10.5px;
    background: var(--bg-overlay);
    padding: 1px 5px;
    border-radius: 2px;
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
  .search-row {
    display: flex;
    gap: 8px;
    margin-bottom: 16px;
  }
  .search {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    color: var(--text-primary);
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    padding: 7px 12px;
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
