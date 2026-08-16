<script lang="ts">
  // Status pill / mono tag (audit D1) — previously ~9 near-identical
  // implementations (.q-pill, .think-badge, .c-state, .p-state, .skill-tag,
  // .status-tag, .status-pill, .verified, .pane-badge …). Tones map to the
  // design's status colors; `color` accepts a raw CSS color for dynamic
  // states; `outline` swaps the tinted fill for a border.
  import Icon from './Icon.svelte'
  import type { Snippet } from 'svelte'

  let {
    tone = 'muted',
    color,
    outline = false,
    radius = 'square',
    icon,
    title,
    children,
    class: klass = '',
  }: {
    tone?: 'muted' | 'accent' | 'slot' | 'success' | 'warning' | 'danger'
    /** Raw CSS color override (dynamic states, e.g. --pcolor). */
    color?: string
    outline?: boolean
    radius?: 'square' | 'pill'
    icon?: string
    title?: string
    children?: Snippet
    class?: string
  } = $props()
</script>

<span
  class={`pill ${tone}${outline ? ' outline' : ''}${radius === 'pill' ? ' pill-radius' : ''}${klass ? ' ' + klass : ''}`}
  style={color ? `--pill-color:${color}` : undefined}
  {title}
>
  {#if icon}
    <span class="p-ico" aria-hidden="true"><Icon name={icon as never} size={12} /></span>
  {/if}
  {@render children?.()}
</span>

<style>
  .pill {
    --pill-color: var(--text-muted);
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--pill-color);
    background: color-mix(in oklch, var(--pill-color) 13%, transparent);
    border: 1px solid transparent;
    border-radius: 2px;
    padding: 2px 7px;
    white-space: nowrap;
  }
  .p-ico {
    display: inline-flex;
    flex: none;
  }
  .pill.muted {
    color: var(--text-muted);
    background: var(--bg-overlay);
  }
  .pill.accent { --pill-color: var(--accent); }
  .pill.slot { --pill-color: var(--slot); }
  .pill.success { --pill-color: var(--success); }
  .pill.warning { --pill-color: var(--warning); }
  .pill.danger { --pill-color: var(--danger); }
  .pill.outline {
    background: transparent;
    border-color: color-mix(in oklch, var(--pill-color) 45%, var(--border-default));
  }
  .pill.pill-radius {
    border-radius: 999px;
    padding: 2px 8px;
  }
</style>
