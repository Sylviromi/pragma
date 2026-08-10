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
  /* Dropdown is position:fixed so it escapes the footer's overflow clipping
     (the footer must scroll on narrow windows, which makes it a clipping
     scroll container — an upward-opening absolute dropdown inside it gets
     cut off). Anchor it to the chip's on-screen position instead. */
  let anchor = $state<{ top: number; left: number } | null>(null)

  $effect(() => {
    if (open) {
      const r = el?.querySelector('.chip')?.getBoundingClientRect()
      anchor = r ? { top: r.top - 6, left: r.left } : null
      searchInput?.focus()
    }
  })

  onMount(async () => {
    await loadProviders()
  })

  // Default to the first available model once providers are known — also fires
  // when discovery lands *after* mount (new API key saved, /v1/models response
  // received), so the chip never sits on "Choose model" when choices exist.
  $effect(() => {
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
    <div
      class="dropdown"
      style:top={anchor ? `${anchor.top}px` : 'auto'}
      style:left={anchor ? `${anchor.left}px` : '0'}
      role="listbox"
    >
      <div class="search">
        <span class="msr" aria-hidden="true">search</span>
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
            >{p.name.toUpperCase()}{#if p.source !== 'builtin'} <span class="plugin-tag">· plugin</span>{/if}</div
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
  /* Design chip: accent dot + model name + provider tag + chevron. */
  .chip {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    padding: 6px 10px;
    font-family: var(--font-body);
    font-size: 12.5px;
    color: var(--text-primary);
    cursor: pointer;
    white-space: nowrap;
    flex: none;
    min-width: 0;
    max-width: 100%;
  }
  .chip:hover {
    border-color: var(--text-muted);
  }
  .chip .mname {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    flex-shrink: 0;
  }
  .pname {
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Design dropdown: opens upward from the chip, 280px, grouped list. */
  .dropdown {
    position: fixed;
    transform: translateY(-100%);
    left: 0;
    top: 0;
    width: 280px;
    max-width: min(280px, calc(100vw - 24px));
    max-height: min(400px, calc(100vh - 76px));
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
    z-index: 100;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border-default);
    margin: 6px 6px 0;
  }
  .search .msr {
    font-size: 15px;
    line-height: 1;
    color: var(--text-muted);
    flex: none;
  }
  .search input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-family: var(--font-body);
    font-size: 12.5px;
  }
  .search input::placeholder {
    color: var(--text-muted);
  }
  .list {
    max-height: min(300px, 45vh);
    overflow-y: auto;
    padding: 2px 6px 6px;
  }
  .group-label {
    padding: 4px 8px;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .plugin-tag {
    color: var(--slot);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    background: transparent;
    border: none;
    border-radius: 2px;
    color: var(--text-secondary);
    font-size: 12.5px;
    font-family: var(--font-body);
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: var(--surface-hover);
    color: var(--text-primary);
  }
  .row.sel {
    background: var(--accent-soft);
  }
  .rname {
    flex: 1;
  }
  .rmeta {
    color: var(--text-muted);
    font-size: 11px;
  }
  .free {
    color: var(--success);
  }
  .check {
    color: var(--success);
    margin-left: auto;
    display: flex;
  }

  /* Narrow: the provider name suffix (· Anthropic) costs ~80px — drop it so
     the chip stays short enough for one-row footers. */
  @media (max-width: 940px) {
    .chip {
      padding: 6px 8px;
    }
    .pname {
      display: none;
    }
  }
</style>