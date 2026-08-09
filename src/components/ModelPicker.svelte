<script lang="ts">
  import { onMount } from 'svelte'
  import { loadProviders, providerState } from '../stores/providers.svelte'
  import SlotStack from './SlotStack.svelte'
  import Icon from './Icon.svelte'

  let {
    model = $bindable(''),
    provider = $bindable('')
  }: { model: string; provider: string } = $props()

  let open = $state(false)
  let search = $state('')
  let el = $state<HTMLElement | null>(null)
  let searchInput = $state<HTMLInputElement | null>(null)

  $effect(() => {
    if (open) searchInput?.focus()
  })

  onMount(async () => {
    await loadProviders()
    if (!model && providerState.providers[0]) {
      provider = providerState.providers[0].id
      model = providerState.providers[0].models[0]?.id ?? ''
    }
  })

  const filtered = $derived(
    providerState.providers
      .map((p) => ({
        ...p,
        models: p.models.filter(
          (m) =>
            !search ||
            m.name.toLowerCase().includes(search.toLowerCase()) ||
            m.id.toLowerCase().includes(search.toLowerCase())
        )
      }))
      .filter((p) => p.models.length > 0)
  )

  const current = $derived(
    providerState.providers
      .flatMap((p) => p.models.map((m) => ({ ...m, pid: p.id, pname: p.name })))
      .find((m) => m.id === model && m.pid === provider)
  )

  function select(pid: string, mid: string) {
    provider = pid
    model = mid
    open = false
    search = ''
  }
</script>

<svelte:window
  onclick={(e) => {
    if (el && !el.contains(e.target as Node)) open = false
  }}
/>

<div class="picker" bind:this={el}>
  <button
    class="chip"
    onclick={() => (open = !open)}
    aria-haspopup="listbox"
    aria-expanded={open}
  >
    <span class="dot" aria-hidden="true"></span>
    <span class="mname">{current?.name ?? 'Choose model'}</span>
    {#if current}<span class="pname">· {current.pname}</span>{/if}
    <span aria-hidden="true"><Icon name="expand_more" size={14} /></span>
  </button>

  {#if open}
    <div class="dropdown" role="listbox">
      <div class="search">
        <input
          type="search"
          placeholder="Search models…"
          bind:value={search}
          bind:this={searchInput}
          aria-label="Search models"
        />
      </div>
      <div class="list">
        {#each filtered as p (p.id)}
          <div class="group-label"
            >{p.name}{#if p.source !== 'builtin'} <span class="plugin-tag">· plugin</span>{/if}</div
          >
          {#each p.models as m (m.id)}
            <button
              class="row"
              class:sel={m.id === model && p.id === provider}
              onclick={() => select(p.id, m.id)}
              role="option"
              aria-selected={m.id === model && p.id === provider}
            >
              <span class="dot" aria-hidden="true"></span>
              <span class="rname">{m.name}</span>
              {#if m.contextWindow}<span class="rmeta">{(m.contextWindow / 1000).toFixed(0)}k</span>{/if}
              {#if m.isFree}<span class="rmeta free">free</span>{/if}
              {#if m.id === model && p.id === provider}<span class="check" aria-hidden="true"><Icon name="check" size={13} /></span>{/if}
            </button>
          {/each}
        {/each}
      </div>
      <SlotStack slot="model-picker-extra" />
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
  }
  .chip {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-md);
    padding: 3px var(--spacing-2);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    white-space: nowrap;
    min-width: 0;
    max-width: 100%;
  }
  /* Model text ellipsizes when the window is narrower than the name. */
  .chip .mname {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chip:hover {
    border-color: var(--color-border-focus);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-accent-brand);
    flex-shrink: 0;
  }
  .pname {
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dropdown {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    width: 280px;
    /* never wider than the window (narrow screens / plugin chips near the edge) */
    max-width: min(280px, calc(100vw - 24px));
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-lg);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    overflow: hidden;
    z-index: 100;
  }
  .search {
    padding: var(--spacing-2);
    border-bottom: 1px solid var(--color-border-subtle);
  }
  .search input {
    width: 100%;
    background: var(--color-bg-base);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-sm);
    padding: 4px var(--spacing-2);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
  }
  .list {
    max-height: min(300px, 45vh); /* short windows: cap the list so the
                                     dropdown always fits above the composer */
    overflow-y: auto;
    padding: var(--spacing-1) 0;
  }
  .group-label {
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-xs);
    font-weight: 700;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .plugin-tag {
    font-weight: normal;
    text-transform: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    width: 100%;
    padding: 5px var(--spacing-4);
    background: transparent;
    border: none;
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    text-align: left;
  }
  .row:hover {
    background: var(--color-bg-overlay);
    color: var(--color-text-primary);
  }
  .row.sel {
    background: color-mix(in srgb, var(--color-accent-brand) 10%, transparent);
  }
  .rname {
    flex: 1;
  }
  .rmeta {
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
  }
  .free {
    color: var(--color-accent-success);
  }
  .check {
    color: var(--color-accent-success);
    margin-left: auto;
  }

  /* Narrow: the provider name suffix (· Anthropic) costs ~80px — drop it so
     the chip stays short enough for one-row footers. */
  @media (max-width: 940px) {
    .chip {
      padding: 3px var(--spacing-1);
    }
    .pname {
      display: none;
    }
  }
</style>