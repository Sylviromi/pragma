<script lang="ts">
  import { plugins, loadPlugins, setEnabled } from '../../../stores/plugins.svelte'
  import { openPluginFolder } from '../../../lib/tauri'
  import { pushToast } from '../../../stores/toast.svelte'

  async function toggle(p: { id: string; name: string; enabled: boolean }) {
    try {
      await setEnabled(p.id, !p.enabled)
      pushToast(p.enabled ? `Enabled ${p.name}` : `Disabled ${p.name}`, 'success')
    } catch (err) {
      pushToast(String(err), 'error')
    }
  }

  function reveal(p: { id: string }) {
    openPluginFolder(p.id).catch((e) => pushToast(String(e), 'error'))
  }
</script>

<h2 style="margin-top:0;color:var(--color-text-primary)">Plugins</h2>
<p class="desc">
  Drop plugin folders into <code>~/.config/pragma/plugins/</code> — changes hot-reload instantly.
</p>

{#if plugins.loading}
  <p class="desc">Loading…</p>
{:else if plugins.list.length === 0}
  <div class="empty">
    <svg width="32" height="32" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.2" aria-hidden="true">
      <rect x="2" y="2" width="6.5" height="6.5"/>
      <rect x="7.5" y="7.5" width="6.5" height="6.5"/>
    </svg>
    <p>No plugins installed yet.</p>
    <p>Restart and the demo plugin will be bootstrapped automatically.</p>
  </div>
{:else}
  <ul class="cards">
    {#each plugins.list as p (p.id)}
      <li class="card">
        <div class="row">
          <span class="name">{p.name}</span>
          <span class="ver">v{p.version}</span>
        </div>
        <p class="meta">{p.description || 'No description'}</p>
        <div class="tags">
          <span class="chip">{p.id}</span>
          {#if p.hasComponent}<span class="chip">component</span>{/if}
          {#if p.hasSkill}<span class="chip">skill</span>{/if}
          {#each p.contributes.providers as prv (prv)}<span class="chip">provider: {prv}</span>{/each}
          {#each p.contributes.slots as s (s)}<span class="chip">slot: {s}</span>{/each}
          {#each p.contributes.commands as c (c)}<span class="chip">cmd: {c}</span>{/each}
        </div>
        <div class="row actions">
          <button class="btn" class:off={p.enabled} onclick={() => toggle(p)} aria-pressed={p.enabled}>
            {p.enabled ? 'Enabled' : 'Disabled'}
          </button>
          <button class="btn ghost" onclick={() => reveal(p)}>Open folder</button>
          <code class="folder">{p.folder}</code>
        </div>
      </li>
    {/each}
  </ul>
{/if}

<style>
  h2 { font-size: var(--font-size-lg); font-weight: 600; margin-bottom: var(--spacing-2); }
  .desc { font-size: var(--font-size-sm); color: var(--color-text-muted); margin: 0 0 var(--spacing-6); }
  code {
    background: var(--color-bg-overlay); padding: 1px 5px;
    border-radius: var(--radius-sm); font-family: var(--font-mono); font-size: var(--font-size-xs);
  }
  .empty {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: var(--spacing-2); padding: var(--spacing-6) 0;
    color: var(--color-text-muted); font-size: var(--font-size-sm);
  }
  .empty p { margin: 0; }
  .empty svg { opacity: 0.4; margin-bottom: var(--spacing-2); }
  .cards { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--spacing-3); }
  .card {
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-lg);
    padding: var(--spacing-4);
    display: flex; flex-direction: column; gap: var(--spacing-3);
  }
  .row { display: flex; align-items: baseline; gap: var(--spacing-2); }
  .name { font-size: var(--font-size-md); font-weight: 600; color: var(--color-text-primary); }
  .ver { font-size: var(--font-size-xs); color: var(--color-text-muted); font-family: var(--font-mono); }
  .meta { margin: 0; font-size: var(--font-size-sm); color: var(--color-text-muted); }
  .tags { display: flex; flex-wrap: wrap; gap: var(--spacing-1); }
  .chip {
    font-family: var(--font-mono); font-size: var(--font-size-xs);
    background: var(--color-bg-overlay); color: var(--color-text-secondary);
    border: 1px solid var(--color-border-subtle); border-radius: var(--radius-sm);
    padding: 1px 6px;
  }
  .actions { display: flex; align-items: center; gap: var(--spacing-2); }
  .btn {
    background: var(--color-accent-brand); color: #fff; border: none;
    border-radius: var(--radius-md); padding: 4px 14px;
    font-size: var(--font-size-sm); cursor: pointer;
  }
  .btn:hover { filter: brightness(1.1); }
  .btn.off { background: var(--color-bg-overlay); color: var(--color-text-secondary); border: 1px solid var(--color-border-default); }
  .btn.ghost { background: transparent; color: var(--color-text-primary); border: 1px solid var(--color-border-default); }
  .folder { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 280px; }
</style>