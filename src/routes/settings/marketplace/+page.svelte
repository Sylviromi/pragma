<script lang="ts">
  import { plugins, loadPlugins, installFromSource } from '../../../stores/plugins.svelte'
  import { pushToast } from '../../../stores/toast.svelte'
  import { onMount } from 'svelte'
  import SettingsHeader from '../../../components/SettingsHeader.svelte'

  // Design registry (`Pragma UI Kit v2.dc.html` → "Install a plugin"): a
  // bordered search panel over row-based npm results — mono initial, name +
  // verified tag, description, meta line and an Install button per row.

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

  // The keyword qualifier is ALWAYS applied to the npm query internally — the
  // input is plain free text and starts empty; no pragma-plugin- prefill.
  let query = $state('')
  /** The fetched npm pool (all pragma-plugin-* matches). */
  let all = $state<NpmPlugin[]>([])
  /** What the user sees: the pool filtered live by the search term. npm's
   *  search API is not a strict AND (a term query still returns the keyword
   *  matches), so the narrowing happens here — typing narrows instantly. */
  const visible = $derived.by(() => {
    const term = query.trim().toLowerCase()
    if (!term) return all
    return all.filter(
      (p) =>
        p.name.toLowerCase().includes(term) ||
        p.displayName.toLowerCase().includes(term) ||
        (p.description ?? '').toLowerCase().includes(term)
    )
  })
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
      all = (data.objects ?? [])
        .map((o: { package: any }) => o.package)
        // Only ACTUAL plugins: the pragma-plugin keyword, the pragma-plugin-*
        // naming convention, or the first-party @pragma/* scope. Everything
        // else (unrelated packages that happened to match the free text)
        // never makes it into the pool.
        .filter(
          (p: any) =>
            (p.keywords ?? []).includes('pragma-plugin') ||
            /(^|\/)pragma-plugin-/.test(p.name) ||
            /^@pragma\//.test(p.name)
        )
        .map((p: any) => ({
          name: p.name,
          displayName: p.name.replace(/^(?:@pragma\/)?(?:pragma-)?plugin-/, ''),
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
  <SettingsHeader
    kicker="Extensions"
    title="Marketplace"
    blurb="Plugins are npm packages — pragma-plugin-* or @pragma/*."
    meta="npm · registry.npmjs.org"
  />

  <!-- search panel -->
  <section class="registry">
    <div class="reg-head">
      <span class="msr" aria-hidden="true">deployed_code_update</span>
      <span class="reg-title">Install a plugin</span>
    </div>

    <div class="reg-search">
      <span class="msr" aria-hidden="true">search</span>
      <input
        type="search"
        bind:value={query}
        placeholder="Search plugins"
        aria-label="Search plugins"
        onkeydown={(e) => {
          if (e.key === 'Enter') search()
        }}
      />
      <span class="reg-count">{searching ? 'searching…' : `${visible.length} found`}</span>
    </div>
  </section>

  <!-- results -->
  {#if !searched && !searching}
    <div class="empty">
      <span class="msr" aria-hidden="true">search</span>
      <p>Searching npm for plugins…</p>
    </div>
  {:else if visible.length === 0}
    <div class="empty">
      <span class="msr" aria-hidden="true">search_off</span>
      <p>
        Nothing on npm matches that. Plugins are published as
        <code>pragma-plugin-*</code> or under <code>@pragma/</code>.
      </p>
    </div>
  {:else}
    <div class="rows">
      {#each visible as p, i (p.name)}
        {@const isInstalling = installingId === p.name}
        {@const installedVer = installed.get(p.source)}
        <div class="row stagger" style={`--i:${Math.min(i, 8)}`}>
          <span class="initial" aria-hidden="true">{p.displayName.slice(0, 1).toUpperCase()}</span>
          <div class="row-copy">
            <div class="row-top">
              <span class="name">{p.displayName}</span>
              <span class="pkg">{p.name}</span>
            </div>
            {#if p.description}<div class="desc">{p.description}</div>{/if}
            <div class="meta">v{p.version}{p.author ? ` · ${p.author}` : ''}</div>
          </div>
          {#if installedVer}
            <span class="btn installed"><span class="msr" aria-hidden="true">check</span>Installed</span>
          {:else}
            <button class="btn" class:busy={isInstalling} onclick={() => install(p)} disabled={isInstalling}>
              <span class="msr" aria-hidden="true">{isInstalling ? 'progress_activity' : 'download'}</span>
              {isInstalling ? 'Installing…' : 'Install'}
            </button>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .page {
    max-width: 860px;
  }

  /* registry panel */
  .registry {
    border: 1px solid var(--border-default);
    background: var(--bg-elevated);
    margin-bottom: 20px;
  }
  .reg-head {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 9px 14px;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--bg-overlay);
  }
  .reg-head .msr {
    font-size: 15px;
    line-height: 1;
    color: var(--accent);
  }
  .reg-title {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-primary);
  }
  .reg-search {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--border-default);
    background: var(--bg-base);
    margin: 0 14px 13px;
    padding: 8px 11px;
  }
  .reg-search > .msr {
    font-size: 16px;
    line-height: 1;
    color: var(--text-muted);
    flex: none;
  }
  .reg-search input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    outline: none;
    font-family: var(--font-mono);
    font-size: 12.5px;
    color: var(--text-primary);
  }
  .reg-search input::placeholder {
    color: var(--text-muted);
  }
  .reg-count {
    flex: none;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
  }

  /* result rows (design registryRows) */
  .rows {
    border: 1px solid var(--border-default);
    background: var(--bg-elevated);
  }
  .row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 13px 14px;
    border-top: 1px solid var(--border-subtle);
  }
  .row:first-child {
    border-top: none;
  }
  .stagger {
    animation: rowIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
    animation-delay: calc(var(--i) * 45ms);
  }
  .initial {
    flex: none;
    width: 30px;
    height: 30px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border-default);
    font-family: var(--font-heading);
    font-size: 14px;
    font-weight: 600;
    color: var(--accent);
  }
  .row-copy {
    flex: 1;
    min-width: 0;
  }
  .row-top {
    display: flex;
    align-items: baseline;
    gap: 9px;
    flex-wrap: wrap;
  }
  .name {
    font-family: var(--font-heading);
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .pkg {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
  }
  .desc {
    font-family: var(--font-body);
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-secondary);
    margin-top: 3px;
    max-width: 62ch;
    text-wrap: pretty;
  }
  .meta {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    margin-top: 5px;
  }
  /* install button states (design regBtn) */
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
    padding: 7px 13px;
    background: none;
    border: 1px solid var(--border-default);
    color: var(--text-primary);
    font-family: var(--font-body);
    font-size: 12.5px;
    cursor: pointer;
    transition: border-color 0.18s ease, color 0.18s ease;
  }
  .btn:hover:not(:disabled) {
    border-color: var(--accent);
  }
  .btn .msr {
    font-size: 16px;
    line-height: 1;
  }
  .btn.busy {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--text-primary);
    cursor: default;
  }
  .btn.busy .msr {
    animation: spin 1s linear infinite;
  }
  .btn:disabled {
    cursor: default;
  }
  .btn.installed {
    border-color: var(--border-subtle);
    color: var(--text-muted);
    cursor: default;
  }
  .btn.installed .msr {
    color: var(--success);
    font-size: 15px;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 44px 0;
    color: var(--text-muted);
    font-size: 12.5px;
    text-align: center;
  }
  .empty .msr {
    font-size: 28px;
    color: var(--text-muted);
  }
  .empty p {
    margin: 0;
  }
  .empty code {
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--text-secondary);
  }
</style>
