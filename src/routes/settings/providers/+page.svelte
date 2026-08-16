<script lang="ts">
  import { onMount } from 'svelte'
  import SettingsHeader from '../../../components/SettingsHeader.svelte'
  import { saveApiKeys, getApiKeys } from '../../../lib/tauri'
  import { providerState, loadProviders } from '../../../stores/providers.svelte'

  let keys = $state<Record<string, string[]>>({})
  // Working draft per provider — saved on Save (rotation order, first = primary).
  let drafts = $state<Record<string, string[]>>({})
  let saved = $state<Record<string, boolean>>({})

  onMount(async () => {
    await loadProviders()
    keys = await getApiKeys()
    for (const p of providerState.providers) {
      drafts[p.id] = [...(keys[p.id] ?? [])]
    }
  })

  function addKey(id: string) {
    if (!drafts[id]) drafts[id] = []
    drafts[id].push('')
  }

  function removeKey(id: string, idx: number) {
    drafts[id].splice(idx, 1)
  }

  async function save(id: string) {
    const cleaned = (drafts[id] ?? []).map((k) => k.trim()).filter((k) => k.length > 0)
    await saveApiKeys(id, cleaned)
    keys[id] = cleaned
    saved[id] = true
    setTimeout(() => {
      saved[id] = false
    }, 2000)
  }

  function maskKey(k: string): string {
    if (!k) return ''
    if (k.length <= 8) return '•'.repeat(8)
    return `${k.slice(0, 5)}${'•'.repeat(12)}${k.slice(-3)}`
  }
</script>

<div class="page">
  <SettingsHeader
    kicker="Credentials"
    title="Providers"
    blurb="Model endpoints and their API tokens. A provider can hold several tokens — Pragma rotates them automatically on rate limits."
  />

  <div class="card stack">
    {#each providerState.providers.filter((p) => p.source === 'builtin') as p}
      {@const list = drafts[p.id] ?? []}
      <div class="block">
        <div class="row-head">
          <div class="pname">
            <span class="dot" aria-hidden="true"></span>
            {p.name}
          </div>
          <span class="status-tag">
            {list.length > 0 ? `${list.length} key${list.length !== 1 ? 's' : ''} · rotates on 429` : 'no key'}
          </span>
        </div>

        {#each list as _, idx (idx)}
          <div class="key-row">
            <input
              id="k-{p.id}-{idx}"
              type="password"
              bind:value={list[idx]}
              placeholder={maskKey((keys[p.id] ?? [])[idx]) || `Key ${idx + 1}`}
              aria-label="{p.name} API key {idx + 1}"
              autocomplete="off"
            />
            {#if list.length > 1}
              <button
                class="icon-btn"
                onclick={() => removeKey(p.id, idx)}
                title="Remove key"
                aria-label="Remove {p.name} key {idx + 1}"
              >
                ✕
              </button>
            {/if}
          </div>
        {/each}

        <div class="btn-row">
          <button class="add-btn" onclick={() => addKey(p.id)}>+ Add key</button>
          <button
            class="save-btn"
            onclick={() => save(p.id)}
            aria-label="Save {p.name} API keys"
          >
            {saved[p.id] ? 'Saved ✓' : 'Save'}
          </button>
        </div>
        {#if list.length > 1}
          <div class="hint">First key is primary (used for model discovery); the others are tried in order after a 429.</div>
        {/if}
      </div>
    {/each}

    {#if providerState.providers.filter((p) => p.source !== 'builtin').length > 0}
      <div class="hint">Plugin-contributed providers are managed on each plugin's settings page.</div>
    {/if}
  </div>
</div>

<style>
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
    margin-bottom: 6px;
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
  .icon-btn {
    background: none;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    cursor: pointer;
    padding: 0 10px;
    font-size: 12px;
  }
  .icon-btn:hover {
    color: var(--danger);
    border-color: var(--danger);
  }
  .btn-row {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }
  .add-btn {
    background: var(--bg-base);
    border: 1px dashed var(--border-default);
    border-radius: var(--radius-sm);
    padding: 5px 12px;
    font-size: 12px;
    color: var(--text-secondary);
    cursor: pointer;
  }
  .add-btn:hover {
    color: var(--accent);
    border-color: var(--accent);
  }
  .save-btn {
    background: var(--accent);
    color: var(--accent-on);
    border: none;
    border-radius: var(--radius-sm);
    padding: 5px 16px;
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }
  .save-btn:hover {
    background: var(--accent-hover);
  }
  .hint {
    margin-top: 6px;
    color: var(--text-muted);
    font-size: 11px;
    line-height: 1.4;
  }
</style>
