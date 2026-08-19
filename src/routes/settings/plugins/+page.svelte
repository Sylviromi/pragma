<script lang="ts">
  import {
    plugins,
    loadPlugins,
    setEnabled,
    installFromSource,
    checkUpdates,
  } from '../../../stores/plugins.svelte';
  import { openPluginFolder } from '../../../lib/tauri';
  import { pushToast } from '../../../stores/toast.svelte';
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import type { PluginUpdateInfo } from '../../../lib/tauri';
  import SettingsHeader from '../../../components/SettingsHeader.svelte';
  import CornerMarks from '../../../components/CornerMarks.svelte';
  import Button from '../../../components/Button.svelte';
  import SearchInput from '../../../components/SearchInput.svelte';
  import Segmented from '../../../components/Segmented.svelte';
  import SectionHeader from '../../../components/SectionHeader.svelte';
  import EmptyState from '../../../components/EmptyState.svelte';
  import Icon from '../../../components/Icon.svelte';
  import Pill from '../../../components/Pill.svelte';
  import Toggle from '../../../components/Toggle.svelte';
  import { goto } from '$app/navigation';

  // Design v2 plugin page (`Pragma UI Kit v2.dc.html`): search + filter pills
  // over a grid of click-through cards (→ plugin detail), and an "Install a
  // plugin" panel at the bottom. Per-plugin actions (update/uninstall) live
  // on the detail page.

  let query = $state('');
  let filter = $state<'all' | 'on' | 'off' | 'updates'>('all');
  let checking = $state(false);
  let installing = $state(false);
  let showInstall = $state(false);
  let sourceInput = $state('');

  onMount(async () => {
    await loadPlugins();
    // If the startup check hasn't run yet (or failed), check now so the
    // badges render — the store dedupes a concurrent in-flight check.
    if (plugins.checkedAt === null) checkUpdates().catch(() => {});
  });

  const updateFor = (id: string): PluginUpdateInfo | undefined =>
    plugins.updates?.find((u) => u.id === id && u.updateAvailable);

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return plugins.list.filter((p) => {
      if (filter === 'on' && !p.enabled) return false;
      if (filter === 'off' && p.enabled) return false;
      if (filter === 'updates' && !updateFor(p.id)) return false;
      if (!q) return true;
      return (p.name + ' ' + p.id + ' ' + p.description + ' ' + (p.author ?? ''))
        .toLowerCase()
        .includes(q);
    });
  });

  const filters = $derived([
    { value: 'all' as const, label: `All ${plugins.list.length}` },
    { value: 'on' as const, label: 'Enabled' },
    { value: 'off' as const, label: 'Disabled' },
    { value: 'updates' as const, label: 'Updates' },
  ]);

  async function checkForUpdates() {
    checking = true;
    try {
      const updates = await checkUpdates();
      if (!updates.some((u) => u.updateAvailable))
        pushToast('All plugins are up to date', 'success');
    } catch (err) {
      pushToast(String(err), 'error');
    } finally {
      checking = false;
    }
  }

  async function browseFolder() {
    const path = await open({ multiple: false, directory: true });
    if (path) sourceInput = path;
  }

  async function install() {
    const spec = sourceInput.trim();
    if (!spec) return;
    installing = true;
    try {
      await installFromSource(spec);
      pushToast('Plugin installed — enabled', 'success');
      sourceInput = '';
      showInstall = false;
    } catch (err) {
      pushToast(`Install failed: ${String(err)}`, 'error');
    } finally {
      installing = false;
    }
  }

  /** Toggle a plugin's enabled state without navigating into its settings page. */
  async function toggleEnabled(id: string, next: boolean) {
    try {
      await setEnabled(id, next);
    } catch (err) {
      pushToast(`Could not toggle plugin: ${String(err)}`, 'error');
    }
  }
</script>

<div class="page">
  <div class="title-row">
    <SettingsHeader
      kicker="Extensions"
      title="Plugins"
      blurb="Everything Pragma can do beyond the core loop. Each plugin brings its own settings and its own skills."
    />
    <div class="header-actions">
      <Button variant="ghost" mono icon="sync" loading={checking} onclick={checkForUpdates}>
        {checking ? 'Checking…' : 'Check for updates'}
      </Button>
      <Button
        variant="ghost"
        mono
        icon="add"
        onclick={() => (showInstall = !showInstall)}
        aria-expanded={showInstall}
      >
        Install
      </Button>
    </div>
  </div>

  <!-- toolbar: search + filter pills -->
  <div class="toolbar">
    <SearchInput bind:value={query} placeholder="Search plugins" label="Search plugins" />
    <Segmented
      mono
      options={filters}
      value={filter}
      onchange={(v) => (filter = v as typeof filter)}
      ariaLabel="Filter plugins"
    />
  </div>

  {#if showInstall}
    <div class="install-panel">
      <SectionHeader
        icon="deployed_code_update"
        title="Install from a source"
        note="npm package or local folder — hot-reloads instantly"
      />
      <div class="install-row">
        <Icon name="search" size={16} />
        <input
          class="source-input"
          bind:value={sourceInput}
          placeholder="npm:pragma-plugin-…  ·  /path/to/folder"
          aria-label="Plugin source"
          onkeydown={(e) => {
            if (e.key === 'Enter') install();
            if (e.key === 'Escape') {
              showInstall = false;
              sourceInput = '';
            }
          }}
        />
        <Button
          variant="ghost"
          icon="folder_open"
          onclick={browseFolder}
          title="Pick a local plugin folder"
        >
          Browse…
        </Button>
        <Button
          variant="primary"
          class="go"
          onclick={install}
          disabled={installing || !sourceInput.trim()}
        >
          {installing ? 'Installing…' : 'Install'}
        </Button>
      </div>
      <p class="install-hint">
        Every Pragma plugin is an npm package — one plugin per package, publishing with
        <code>npm publish</code>. Dropping a folder into
        <code>~/.config/dev.pragma.app/plugins/</code> works too. Or{' '}
        <a href="/settings/marketplace">browse the marketplace</a>.
      </p>
    </div>
  {/if}

  {#if plugins.loading && plugins.list.length === 0}
    <div class="page-empty">Loading…</div>
  {:else if plugins.list.length === 0}
    <EmptyState icon="extension">
      <p>No plugins installed yet.</p>
      <p>Restart and the demo plugin will be bootstrapped automatically.</p>
    </EmptyState>
  {:else if filtered.length === 0}
    <EmptyState icon="search_off">
      <p>Nothing matches “{query}”.</p>
    </EmptyState>
  {:else}
    <div class="grid">
      {#each filtered as p, i (p.id)}
        {@const up = updateFor(p.id)}
        <div
          class="card stagger"
          style={`--i:${Math.min(i, 10)}`}
          class:off={!p.enabled}
          role="link"
          tabindex="0"
          aria-label={`${p.name} — open plugin settings`}
          onclick={(e) => {
            // Toggling a plugin must not open its settings page.
            if ((e.target as HTMLElement).closest('.foot-actions')) return;
            goto(`/settings/plugin/${p.id}`);
          }}
          onkeydown={(e) => {
            if ((e.target as HTMLElement).closest('.foot-actions')) return;
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              goto(`/settings/plugin/${p.id}`);
            }
          }}
        >
          <CornerMarks color={p.enabled ? 'var(--accent)' : 'var(--border-subtle)'} />

          <div class="card-top">
            <span class="initial" aria-hidden="true">{p.name.slice(0, 1).toUpperCase()}</span>
            <div class="name-col">
              <div class="name">{p.name}</div>
              <div class="ver">v{p.version} · {p.publisher || p.author || 'community'}</div>
            </div>
          </div>
          <p class="desc">{p.description || 'No description'}</p>
          <div class="card-foot">
            <Pill
              >{p.skills.length
                ? `${p.skills.length} skill${p.skills.length > 1 ? 's' : ''}`
                : 'no skills'}</Pill
            >
            <div class="foot-actions">
              {#if up}
                <span class="update-tag" title={`Update available — v${up.latest}`}>
                  update → v{up.latest}
                </span>
              {/if}
              <Toggle
                size="sm"
                checked={p.enabled}
                label={p.enabled ? 'Disable this plugin' : 'Enable this plugin'}
                onChange={(v) => void toggleEnabled(p.id, v)}
              />
            </div>
          </div>
        </div>
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

  /* toolbar: search + filter pills */
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 18px;
    flex-wrap: wrap;
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
    /* on = solid, clearly accent-bordered (accent-soft was so faint it read
       as off); off = barely-there border below */
    border: 1px solid var(--accent);
    padding: 0;
    cursor: pointer;
    text-decoration: none;
    transition: border-color 0.2s ease;
  }
  .card:hover,
  .card:focus-visible {
    border-color: var(--accent-hover);
    outline: none;
    /* light the corner marks up with the border */
    --cm-color: var(--accent);
  }
  /* off = faint border; the border itself is the on/off showcase */
  .card.off {
    border-color: var(--border-subtle);
  }
  .card.off:hover,
  .card.off:focus-visible {
    border-color: var(--accent);
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
  .desc {
    margin: 8px 13px 0;
    font-family: var(--font-body);
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-muted);
    flex: 1;
    /* clamp descriptions so long ones don't stretch the whole grid row */
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .card-foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 13px 12px;
    margin-top: auto;
  }
  .foot-actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .update-tag {
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.05em;
    color: var(--warning);
    white-space: nowrap;
  }

  /* install panel (design "Install a plugin" registry section) */
  .install-panel {
    margin: 26px 0 18px;
    border: 1px solid var(--border-default);
    background: var(--bg-elevated);
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
  .install-row > :global(.msr) {
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
  /* Restore the page's original Install button (heading face, square). */
  :global(.btn.go) {
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 12.5px;
    padding: 8px 14px;
    border-radius: 0;
  }

  .page-empty {
    display: flex;
    justify-content: center;
    padding: 48px 0;
    color: var(--text-muted);
    font-size: 12.5px;
  }
</style>
