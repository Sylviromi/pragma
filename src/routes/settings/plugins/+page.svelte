<script lang="ts">
  import { plugins, loadPlugins, setEnabled } from '../../../stores/plugins.svelte'
  import { openPluginFolder } from '../../../lib/tauri'
  import { pushToast } from '../../../stores/toast.svelte'
  import { onMount } from 'svelte'
  import Icon from '../../../components/Icon.svelte'

  let expanded = $state<Record<string, boolean>>({})

  onMount(async () => {
    await loadPlugins()
  })

  async function toggle(p: { id: string; name: string; enabled: boolean }) {
    const next = !p.enabled
    try {
      await setEnabled(p.id, next)
      pushToast(`${next ? 'Enabled' : 'Disabled'} ${p.name}`, 'success')
    } catch (err) {
      pushToast(String(err), 'error')
    }
  }

  function reveal(p: { id: string }) {
    openPluginFolder(p.id).catch((e) => pushToast(String(e), 'error'))
  }

  const badge = (text: string, kind: 'provider' | 'slot' | 'command' | 'skill') => ({
    text,
    kind
  })
</script>

<div class="page">
  <div class="title">Plugins</div>
  <div class="desc-lead">
    Drop plugin folders into <code>~/.config/dev.pragma.app/plugins/</code> — changes hot-reload instantly.
  </div>

  {#if plugins.loading}
    <div class="empty">Loading…</div>
  {:else if plugins.list.length === 0}
    <div class="empty">
      <Icon name="extension" size={30} />
      <p>No plugins installed yet.</p>
      <p>Restart and the demo plugin will be bootstrapped automatically.</p>
    </div>
  {:else}
    <ul class="cards">
      {#each plugins.list as p (p.id)}
        <li class="card">
          <div class="head">
            <span class="initial" aria-hidden="true">{p.name.slice(0, 1).toUpperCase()}</span>
            <div class="name-col">
              <div class="name-row">
                <span class="name">{p.name}</span>
                <span class="ver">v{p.version}</span>
              </div>
              <div class="id">{p.id}</div>
            </div>
            <button
              class="expand"
              class:open={expanded[p.id]}
              onclick={() => (expanded[p.id] = !expanded[p.id])}
              aria-expanded={!!expanded[p.id]}
              aria-label="Expand {p.name}"
            >
              <Icon name="chevron_right" size={16} />
            </button>
          </div>

          <p class="meta">{p.description || 'No description'}</p>

          <div class="tags">
            {#each p.contributes.providers as prv (prv)}<span class="b provider">{badge(prv, 'provider').text}</span>{/each}
            {#each p.contributes.slots as s (s)}<span class="b slot">{s}</span>{/each}
            {#each p.contributes.commands as c (c)}<span class="b command">{c}</span>{/each}
            {#if p.hasSkill}<span class="b skill">skill</span>{/if}
          </div>

          {#if expanded[p.id]}
            <div class="detail">
              <div class="status-line">
                <span class="status-dot" class:off={!p.enabled} aria-hidden="true"></span>
                <span>{p.enabled ? 'Active' : 'Disabled'}</span>
                <code class="folder">{p.folder}</code>
              </div>
              <div class="actions">
                <a class="link" href={`/settings/plugin/${p.id}`}>Open settings</a>
                <button class="link" onclick={() => reveal(p)}>Open folder</button>
              </div>
            </div>
          {/if}

          <div class="foot">
            <button class="btn" class:off={p.enabled} onclick={() => toggle(p)} aria-pressed={p.enabled}>
              {p.enabled ? 'Enabled' : 'Disabled'}
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .title {
    font-family: var(--font-heading);
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 2px;
  }
  .desc-lead {
    color: var(--text-muted);
    font-size: 12.5px;
    margin-bottom: 20px;
  }
  .desc-lead code {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-overlay);
    padding: 1px 5px;
    border-radius: 2px;
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
  .empty p {
    margin: 0;
  }
  .cards {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 720px;
  }
  .card {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .initial {
    width: 32px;
    height: 32px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 15px;
    color: var(--accent-on);
    background: var(--accent);
    flex: none;
  }
  .name-col {
    flex: 1;
    min-width: 0;
  }
  .name-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .name {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .ver {
    font-size: 11px;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
  .id {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
  }
  .expand {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    background: transparent;
    color: var(--text-muted);
    border-radius: 3px;
    cursor: pointer;
    flex: none;
    transition: transform 0.15s ease;
  }
  .expand.open {
    transform: rotate(90deg);
  }
  .expand:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }
  .meta {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .b {
    font-family: var(--font-mono);
    font-size: 10px;
    border: 1px solid var(--border-default);
    border-radius: 2px;
    padding: 1px 7px;
  }
  .b.provider {
    color: var(--success);
    border-color: color-mix(in srgb, var(--success) 40%, transparent);
    background: color-mix(in srgb, var(--success) 12%, transparent);
  }
  .b.slot {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .b.command {
    color: var(--slot);
    border-color: color-mix(in srgb, var(--slot) 40%, transparent);
    background: color-mix(in srgb, var(--slot) 14%, transparent);
  }
  .b.skill {
    color: var(--warning);
    border-color: color-mix(in srgb, var(--warning) 40%, transparent);
    background: color-mix(in srgb, var(--warning) 12%, transparent);
  }
  .detail {
    border-top: 1px solid var(--border-subtle);
    padding-top: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .status-line {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--success);
  }
  .status-dot.off {
    background: var(--text-muted);
  }
  .folder {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 320px;
    margin-left: auto;
  }
  .actions {
    display: flex;
    gap: 16px;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    font-size: 12px;
    color: var(--accent);
    cursor: pointer;
    text-decoration: none;
  }
  .link:hover {
    text-decoration: underline;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
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
  .btn.off {
    background: var(--bg-overlay);
    color: var(--text-secondary);
    border: 1px solid var(--border-default);
  }
</style>