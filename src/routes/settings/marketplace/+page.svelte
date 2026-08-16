<script lang="ts">
  import { plugins, loadPlugins, installFromSource } from '../../../stores/plugins.svelte'
  import { pushToast } from '../../../stores/toast.svelte'
  import { onMount } from 'svelte'
  import SettingsHeader from '../../../components/SettingsHeader.svelte'
  import SectionHeader from '../../../components/SectionHeader.svelte'
  import SearchInput from '../../../components/SearchInput.svelte'
  import EmptyState from '../../../components/EmptyState.svelte'
  import Icon from '../../../components/Icon.svelte'

  // Design registry (`Pragma UI Kit v2.dc.html` → "Install a plugin"): a
  // bordered search panel over row-based npm results — mono initial, name +
  // description, meta line and an Install button per row.
  // Search UX: the input starts EMPTY and the fetched pool narrows live as you
  // type (npm's search API is not a strict AND, so narrowing happens locally).

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

  let query = $state('')
  /** The fetched npm pool (all pragma-plugin-* matches). */
  let all = $state<NpmPlugin[]>([])
  /** What the user sees: the pool filtered live by the search term. */
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
    <SectionHeader icon="deployed_code_update" title="Install a plugin" />

    <div class="reg-search">
      <SearchInput
        mono
        bind:value={query}
        placeholder="Search plugins"
        label="Search plugins"
        onkeydown={(e) => {
          if (e.key === 'Enter') search()
        }}
      >
        {#snippet trailing()}
          <span class="reg-count">{searching ? 'searching…' : `${visible.length} found`}</span>
        {/snippet}
      </SearchInput>
    </div>
  </section>

  <!-- results -->
  {#if !searched && !searching}
    <EmptyState icon="search">
      <p>Searching npm for plugins…</p>
    </EmptyState>
  {:else if visible.length === 0}
    <EmptyState icon="search_off">
      <p>
        Nothing on npm matches that. Plugins are published as
        <code>pragma-plugin-*</code> or under <code>@pragma/</code>.
      </p>
    </EmptyState>
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
            <span class="btn installed"><Icon name="check" size={15} />Installed</span>
          {:else}
            <button class="btn" class:busy={isInstalling} onclick={() => install(p)} disabled={isInstalling}>
              <Icon name={isInstalling ? 'progress_activity' : 'download'} size={16} />
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
  .reg-search {
    margin: 0 14px 13px;
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
  /* install button states */
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
  .btn.busy {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--text-primary);
    cursor: default;
  }
  .btn.busy :global(.msr) {
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
  .btn.installed :global(.msr) {
    color: var(--success);
  }
</style>
