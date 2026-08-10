<script lang="ts">
  import { onMount } from 'svelte'
  import { saveApiKey, getApiKeys } from '../../../lib/tauri'
  import { providerState, loadProviders } from '../../../stores/providers.svelte'

  let keys = $state<Record<string, string>>({})
  let saved = $state<Record<string, boolean>>({})

  onMount(async () => {
    await loadProviders()
    keys = await getApiKeys()
  })

  async function save(id: string) {
    await saveApiKey(id, keys[id] ?? '')
    saved[id] = true
    setTimeout(() => {
      saved[id] = false
    }, 2000)
  }

  function maskKey(k: string): string {
    if (!k) return 'Not set'
    if (k.length <= 8) return '•'.repeat(8)
    return `${k.slice(0, 5)}${'•'.repeat(12)}${k.slice(-3)}`
  }
</script>

<div class="page">
  <div class="title">Providers</div>
  <div class="desc-lead">
    API keys are stored in <code>~/.config/dev.pragma.app/settings.json</code>.
  </div>

  <div class="card stack">
    {#each providerState.providers.filter((p) => p.source === 'builtin') as p}
      <div class="block">
        <div class="row-head">
          <div class="pname">
            <span class="dot" aria-hidden="true"></span>
            {p.name}
          </div>
          <span class="status-tag">{keys[p.id] ? 'configured' : 'no key'}</span>
        </div>
        <div class="key-row">
          <input
            id="k-{p.id}"
            type="password"
            bind:value={keys[p.id]}
            placeholder={maskKey(keys[p.id])}
            aria-label="{p.name} API key"
            autocomplete="off"
          />
          <button
            class="save-btn"
            onclick={() => save(p.id)}
            aria-label="Save {p.name} API key"
          >
            {saved[p.id] ? 'Saved ✓' : 'Save'}
          </button>
        </div>
      </div>
    {/each}

    {#if providerState.providers.filter((p) => p.source !== 'builtin').length > 0}
      <div class="hint">Plugin-contributed providers are managed on each plugin's settings page.</div>
    {/if}
  </div>
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
  .card {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    overflow: hidden;
    max-width: 640px;
  }
  .block {
    padding: 14px 16px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .block:last-child {
    border-bottom: none;
  }
  .row-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 10px;
  }
  .pname {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-primary);
    font-size: 13.5px;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .status-tag {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    border: 1px solid var(--border-default);
    border-radius: 2px;
    padding: 2px 6px;
  }
  .key-row {
    display: flex;
    gap: 8px;
  }
  input {
    flex: 1;
    min-width: 0;
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    padding: 7px 10px;
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    outline: none;
  }
  input:focus {
    border-color: var(--border-focus);
  }
  .save-btn {
    background: var(--accent);
    color: var(--accent-on);
    border: none;
    border-radius: var(--radius-sm);
    padding: 7px 16px;
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 12.5px;
    cursor: pointer;
    white-space: nowrap;
  }
  .save-btn:hover {
    background: var(--accent-hover);
  }
  .hint {
    padding: 12px 16px;
    color: var(--text-muted);
    font-size: 12px;
  }
</style>