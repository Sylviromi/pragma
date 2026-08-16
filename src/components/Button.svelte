<script lang="ts">
  // Single button primitive for the whole app — previously every file
  // re-defined its own .ghost/.primary/.danger/.go/.hdr-btn/.act (componentization
  // audit C6). Variants: primary (accent fill) / ghost (bordered) / danger
  // (outlined red) / danger-solid (filled red). Sizes: xs (tool-card header),
  // sm (default), md (big accent CTA). `mono` switches to the design's mono
  // uppercase label style (settings header actions, plugin actions, follow…).
  // `icon` renders a Material glyph; `loading` swaps it for a spinner.
  // `href` renders an <a> with the same skin (back links, plugin cards…).
  import Icon from './Icon.svelte'
  import type { Snippet } from 'svelte'

  let {
    variant = 'ghost',
    size = 'sm',
    mono = false,
    flat = false,
    icon,
    iconRight = false,
    loading = false,
    disabled = false,
    href,
    title,
    ariaLabel,
    onclick,
    children,
    class: klass = '',
    ...restProps
  }: {
    variant?: 'primary' | 'ghost' | 'danger' | 'danger-solid' | 'warning' | 'quiet'
    size?: 'xs' | 'sm' | 'md'
    mono?: boolean
    /** Borderless ghost (the old settings-page `.ghost` — hover = bg-overlay). */
    flat?: boolean
    icon?: string
    iconRight?: boolean
    loading?: boolean
    disabled?: boolean
    href?: string
    title?: string
    ariaLabel?: string
    onclick?: (e: MouseEvent) => void
    children?: Snippet
    class?: string
    /** Remaining attributes forwarded to the <button>/<a> (aria-*, etc.). */
    [key: string]: unknown
  } = $props()

  const iconSize = $derived(size === 'xs' ? 14 : 15)
  const iconName = $derived((loading ? 'progress_activity' : icon) as never)
  const showIcon = $derived(!!icon || loading)
  const cls = $derived(
    `btn v-${variant} s-${size}${mono ? ' mono' : ''}${flat ? ' flat' : ''}${klass ? ` ${klass}` : ''}`
  )
</script>

{#if href}
  <a {href} class={cls} {title} aria-label={ariaLabel} {onclick} {...restProps}>
    {#if showIcon && !iconRight}
      <span class="ico" class:spin={loading} aria-hidden="true"><Icon name={iconName} size={iconSize} /></span>
    {/if}
    {@render children?.()}
    {#if showIcon && iconRight}
      <span class="ico" class:spin={loading} aria-hidden="true"><Icon name={iconName} size={iconSize} /></span>
    {/if}
  </a>
{:else}
  <button class={cls} {disabled} {title} aria-label={ariaLabel} aria-busy={loading} {onclick} {...restProps}>
    {#if showIcon && !iconRight}
      <span class="ico" class:spin={loading} aria-hidden="true"><Icon name={iconName} size={iconSize} /></span>
    {/if}
    {@render children?.()}
    {#if showIcon && iconRight}
      <span class="ico" class:spin={loading} aria-hidden="true"><Icon name={iconName} size={iconSize} /></span>
    {/if}
  </button>
{/if}

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border-radius: var(--radius-md);
    font-family: var(--font-body);
    cursor: pointer;
    white-space: nowrap;
    flex: none;
    text-decoration: none;
    transition: background 0.18s ease, border-color 0.18s ease, color 0.18s ease;
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .ico {
    display: inline-flex;
    flex: none;
  }
  .ico.spin {
    animation: spin 1s linear infinite;
  }

  /* sizes — xs: tool-card header (11.5px, 2px radius); sm: default;
     md: big accent CTA (heading face, like the design's send button). */
  .btn.s-xs {
    font-size: 11.5px;
    padding: 2px 10px;
    border-radius: var(--radius-sm);
  }
  .btn.s-sm {
    font-size: 12.5px;
    padding: 7px 12px;
  }
  .btn.s-md {
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 13px;
    padding: 8px 15px;
  }

  /* variants */
  .btn.v-ghost {
    background: transparent;
    color: var(--text-secondary);
    border: 1px solid var(--border-default);
  }
  .btn.v-ghost:hover:not(:disabled) {
    color: var(--text-primary);
    border-color: var(--accent);
  }
  /* Borderless ghost — general page's original `.ghost` buttons. */
  .btn.v-ghost.flat {
    border-color: transparent;
  }
  .btn.v-ghost.flat:hover:not(:disabled) {
    background: var(--bg-overlay);
    border-color: transparent;
  }
  .btn.v-primary {
    background: var(--accent);
    color: var(--accent-on);
    border: 1px solid var(--accent);
  }
  .btn.v-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .btn.v-danger {
    background: transparent;
    color: var(--danger);
    border: 1px solid currentColor;
  }
  .btn.v-danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }
  .btn.v-danger-solid {
    background: color-mix(in srgb, var(--danger) 85%, black);
    color: white;
    border: 1px solid transparent;
    font-weight: 600;
  }
  .btn.v-danger-solid:hover:not(:disabled) {
    filter: brightness(1.1);
  }
  .btn.v-warning {
    background: transparent;
    color: var(--warning);
    border: 1px solid color-mix(in srgb, var(--warning) 45%, transparent);
  }
  .btn.v-warning:hover:not(:disabled) {
    border-color: var(--warning);
  }
  /* Quiet text link (back crumbs) — no border or padding emphasis. */
  .btn.v-quiet {
    background: transparent;
    color: var(--text-muted);
    border: 1px solid transparent;
  }
  .btn.v-quiet:hover:not(:disabled) {
    color: var(--accent);
  }

  /* design mono uppercase label style (settings header actions, plugin
     actions, follow / back links) */
  .btn.mono {
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
</style>
