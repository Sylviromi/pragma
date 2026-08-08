<script lang="ts">
  export type MenuIcon = 'copy' | 'transcript' | 'return'

  export interface MenuItem {
    label: string
    hint?: string
    icon?: MenuIcon
    danger?: boolean
    disabled?: boolean
    onSelect: () => void
  }

  let {
    x,
    y,
    items,
    onclose,
  }: {
    x: number
    y: number
    items: MenuItem[]
    onclose: () => void
  } = $props()

  let menuEl = $state<HTMLElement | null>(null)
  let pos = $state({ x, y })

  // Clamp to the viewport once we know our own size.
  $effect(() => {
    const el = menuEl
    if (!el) return
    const r = el.getBoundingClientRect()
    const nx = Math.min(x, window.innerWidth - r.width - 8)
    const ny = Math.min(y, window.innerHeight - r.height - 8)
    pos = { x: Math.max(8, nx), y: Math.max(8, ny) }
  })

  // Close on outside click, Escape, window blur, or any scroll.
  $effect(() => {
    const onPointer = (e: PointerEvent) => {
      if (menuEl && !menuEl.contains(e.target as Node)) onclose()
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onclose()
    }
    window.addEventListener('pointerdown', onPointer)
    window.addEventListener('keydown', onKey)
    window.addEventListener('blur', onclose)
    window.addEventListener('scroll', onclose, true)
    return () => {
      window.removeEventListener('pointerdown', onPointer)
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('blur', onclose)
      window.removeEventListener('scroll', onclose, true)
    }
  })
</script>

<div
  class="ctx-menu"
  style="left: {pos.x}px; top: {pos.y}px;"
  bind:this={menuEl}
  role="menu"
  aria-label="Message actions"
>
  <div class="ctx-header">Message actions</div>
  {#each items as item}
    <button
      class="ctx-item"
      class:danger={item.danger}
      role="menuitem"
      disabled={item.disabled}
      onclick={(e) => {
        e.stopPropagation()
        item.onSelect()
      }}
    >
      <span class="ic" class:ic-danger={item.danger} aria-hidden="true">
        {#if item.icon === 'copy'}
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="9" y="9" width="13" height="13" rx="2"/>
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
          </svg>
        {:else if item.icon === 'transcript'}
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="4" width="7" height="16" rx="1.5"/>
            <rect x="14" y="4" width="7" height="16" rx="1.5"/>
          </svg>
        {:else}
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 7v6h6"/>
            <path d="M3 13a9 9 0 1 0 3-7.7L2.99 5"/>
          </svg>
        {/if}
      </span>
      <span class="body">
        <span class="label">{item.label}</span>
        {#if item.hint}
          <span class="hint">{item.hint}</span>
        {/if}
      </span>
    </button>
  {/each}
</div>

<style>
  .ctx-menu {
    position: fixed;
    z-index: 1100;
    min-width: 240px;
    max-width: 340px;
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border-default);
    border-radius: 10px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45), 0 2px 8px rgba(0, 0, 0, 0.2);
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: menu-in 120ms ease-out;
    transform-origin: top left;
    /* tiny backdrop triangle-like accent under the cursor feels native */
  }
  @keyframes menu-in {
    from { opacity: 0; transform: translateY(-4px) scale(0.97); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }

  .ctx-header {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--color-text-muted);
    padding: 4px 10px 6px;
  }

  .ctx-item {
    display: flex;
    align-items: center;
    gap: 10px;
    background: transparent;
    border: none;
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    padding: 7px 10px;
    border-radius: 7px;
    text-align: left;
    cursor: pointer;
    width: 100%;
  }
  .ctx-item:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-accent-brand) 12%, transparent);
  }
  .ctx-item:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .ic {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--color-text-muted) 12%, transparent);
    color: var(--color-text-muted);
    flex-shrink: 0;
  }
  .ctx-item:hover:not(:disabled) .ic {
    background: color-mix(in srgb, var(--color-accent-brand) 18%, transparent);
    color: var(--color-accent-brand);
  }
  .ic-danger {
    background: color-mix(in srgb, var(--color-accent-error, #f85149) 14%, transparent);
    color: var(--color-accent-error, #f85149);
  }
  .ctx-item:hover:not(:disabled) .ic-danger {
    background: color-mix(in srgb, var(--color-accent-error, #f85149) 20%, transparent);
    color: var(--color-accent-error, #f85149);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .label {
    font-size: var(--font-size-sm);
    line-height: 1.3;
  }
  .hint {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }
</style>