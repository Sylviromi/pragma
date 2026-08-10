<script lang="ts">
  import { onMount } from 'svelte'
  import { loadProviders, providerState } from '../stores/providers.svelte'
  import SlotStack from './SlotStack.svelte'
  import Icon from './Icon.svelte'
  import type { ThinkingPref } from '../lib/types'

  let {
    model = $bindable(''),
    provider = $bindable(''),
    thinking = $bindable<ThinkingPref>('auto')
  }: {
    model: string
    provider: string
    thinking: ThinkingPref
  } = $props()

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
  // Prefers a provider with a free model (zen-proxy style free tiers) so a
  // fresh app without API keys lands on a model that actually works, instead
  // of Anthropic's fallback list which needs an API key.
  $effect(() => {
    if (model || !providerState.providers.length) return
    const prov =
      providerState.providers.find((p) => p.models.some((m) => m.isFree)) ??
      providerState.providers[0]
    provider = prov.id
    model = prov.models[0]?.id ?? ''
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

  // Thinking controls. The available levels adapt to the selected model:
  // models with graduated thinking (Sonnet 4.5+/Opus 4.x, gpt-5, o-series,
  // qwen3, …) get off/auto/low/medium/high; thinking-but-fixed models
  // (Claude 3.7) get just off/auto; non-thinking models get a muted disabled
  // row. `thinking` lives in the chat store per session via the parent.
  const canThink = $derived(current?.supportsThinking ?? false)
  const canLevels = $derived(current?.supportsThinkingLevels ?? false)
  const levels: ThinkingPref[] = $derived(
    canLevels ? ['off', 'auto', 'low', 'medium', 'high'] : ['off', 'auto']
  )
  const levelLabel = (l: ThinkingPref) =>
    l === 'auto' ? 'Auto' : l === 'off' ? 'Off' : l[0].toUpperCase() + l.slice(1)
  const thinkHint = $derived(
    !current
      ? 'Enable when supported'
      : !canThink
        ? 'Model does not support'
        : canLevels
          ? 'Off · Auto · Low · Med · High'
          : 'Fixed level — Off or Auto'
  )
  let levelMenuOpen = $state(false)
  $effect(() => {
    // Close the level menu when the model changes (its options/meaning shift).
    if (model || provider) levelMenuOpen = false
  })
</script>

<svelte:window
  onclick={(e) => {
    // composedPath() is snapshotted when the event dispatches, so it still
    // contains the picker even for clicks that unmount part of the DOM before
    // bubbling reaches window (e.g. picking a thinking level removes the menu,
    // detaching the clicked button — el.contains would then return false and
    // wrongly close the whole picker).
    if (el && !e.composedPath().includes(el)) open = false
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
              {#if m.supportsThinking}<span class="rmeta think">think</span>{/if}
              {#if m.id === model && p.id === provider}<span class="check" aria-hidden="true"><Icon name="check" size={13} /></span>{/if}
            </button>
          {/each}
        {/each}
      </div>
      <!-- Thinking preference: how much reasoning the model may do. Off = no
           thinking (cost control); Auto = model default level; Low/Med/High =
           graduated thinking where the model supports levels. Muted + disabled
           when the selected model can't think at all. -->
      <div class="think-row" class:muted={!canThink}>
        <div class="think-copy">
          <span class="think-label">Thinking</span>
          <span class="think-hint">{thinkHint}</span>
        </div>
        <button
          class="think-value"
          onclick={() => (levelMenuOpen = !levelMenuOpen)}
          disabled={!canThink}
          aria-haspopup="listbox"
          aria-expanded={levelMenuOpen}
          title={canLevels ? 'Thinking level' : 'Thinking'}
        >
          {levelLabel(thinking)}<Icon name="expand_more" size={13} />
        </button>
      </div>
      {#if levelMenuOpen && canThink}
        <div class="think-menu" role="listbox" aria-label="Thinking level">
          {#each levels as l (l)}
            <button
              class="tm-opt"
              class:sel={thinking === l}
              onclick={() => {
                thinking = l
                levelMenuOpen = false
              }}
              role="option"
              aria-selected={thinking === l}
            >
              <span class="tm-name">{levelLabel(l)}</span>
              {#if l === 'auto'}
                <span class="tm-sub">model default</span>
              {/if}
              {#if thinking === l}<span class="check" aria-hidden="true"><Icon name="check" size={13} /></span>{/if}
            </button>
          {/each}
        </div>
      {/if}
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
    /* Flex column: the model list shrinks to make room when the thinking-level
       menu opens, instead of the dropdown overflowing its viewport cap. */
    display: flex;
    flex-direction: column;
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
    flex: 1;
    min-height: 0;
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
  .think {
    color: var(--accent);
    text-transform: uppercase;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
  }
  /* Thinking preference row: label + hint left, toggle right. */
  .think-row {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-content: space-between;
    padding: 8px 12px;
    border-top: 1px solid var(--border-subtle);
    margin: 4px 6px 0;
  }
  .think-row.muted {
    opacity: 0.5;
  }
  .think-copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .think-label {
    font-size: 12.5px;
    color: var(--text-primary);
  }
  .think-hint {
    font-size: 11px;
    color: var(--text-muted);
  }
  /* Select-like value button (Off / Auto / Low / …). */
  .think-value {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 3px 8px;
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-size: 12px;
    font-family: var(--font-body);
    cursor: pointer;
    flex: none;
  }
  .think-value:hover:not(:disabled) {
    border-color: var(--text-muted);
  }
  .think-value:disabled {
    cursor: default;
  }
  /* Level options list — opens inside the dropdown, scrolls if tight. */
  .think-menu {
    margin: 1px 6px 4px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    max-height: 152px;
    overflow-y: auto;
    background: var(--bg-overlay);
  }
  .tm-opt {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    background: transparent;
    border: none;
    color: var(--text-secondary);
    font-size: 12.5px;
    font-family: var(--font-body);
    text-align: left;
    cursor: pointer;
  }
  .tm-opt:hover {
    background: var(--surface-hover);
    color: var(--text-primary);
  }
  .tm-opt.sel {
    background: var(--accent-soft);
    color: var(--text-primary);
  }
  .tm-name {
    flex: 1;
  }
  .tm-sub {
    color: var(--text-muted);
    font-size: 10.5px;
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