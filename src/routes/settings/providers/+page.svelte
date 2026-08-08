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
</script>

<h2 style="margin-top:0;color:var(--color-text-primary)">Providers</h2>
<p style="color:var(--color-text-muted);font-size:var(--font-size-sm)">
  Keys stored in ~/.config/pragma/settings.json
</p>

{#each providerState.providers.filter((p) => p.source === 'builtin') as p}
  <div style="margin-top:var(--spacing-6)">
    <label
      style="display:block;font-weight:500;margin-bottom:6px;color:var(--color-text-primary)"
      for="k-{p.id}"
    >{p.name}</label>
    <div style="display:flex;gap:8px">
      <input
        id="k-{p.id}"
        type="password"
        bind:value={keys[p.id]}
        placeholder="API key"
        style="flex:1;background:var(--color-bg-elevated);border:1px solid var(--color-border-default);border-radius:var(--radius-md);padding:6px 10px;color:var(--color-text-primary);font-family:var(--font-mono);font-size:var(--font-size-sm)"
      />
      <button
        onclick={() => save(p.id)}
        style="background:var(--color-accent-brand);color:white;border:none;border-radius:var(--radius-md);padding:6px 16px;cursor:pointer"
      >
        {saved[p.id] ? 'Saved ✓' : 'Save'}
      </button>
    </div>
  </div>
{/each}