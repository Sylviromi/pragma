<script lang="ts">
  // Collapsible card primitive — the shared structure behind ThoughtCard,
  // SubagentCard, ToolCard and ProcessCard (audit A1). Owns the header row
  // (optional left rail + clickable head + right actions + rotating chevron)
  // and the grid-rows 0fr↔1fr body collapse that all four used to re-implement.
  import Icon from './Icon.svelte'
  import type { Snippet } from 'svelte'

  let {
    open = $bindable(true),
    rail = false,
    railColor = 'var(--slot)',
    streaming = false,
    clickable = true,
    dense = false,
    chevron = true,
    label,
    header,
    actions,
    footer,
    children,
    class: klass = '',
    style,
  }: {
    /** Open state — bindable; cards default open, ProcessCard starts closed. */
    open?: boolean
    /** 2px tinted left rail (thought / sub-agent cards). */
    rail?: boolean
    railColor?: string
    /** Streaming variant: pulsing rail. */
    streaming?: boolean
    /** Whole head clickable (thought/sub-agent/process) vs. chevron-only (tool). */
    clickable?: boolean
    /** Tight header padding (tool-card strip). */
    dense?: boolean
    /** Show the collapse chevron (hidden while a tool waits for approval). */
    chevron?: boolean
    /** Aria-label for the toggle control. */
    label?: string
    /** Left side of the header (icon + title + meta). */
    header?: Snippet
    /** Right side of the header, before the chevron (status glyphs, buttons). */
    actions?: Snippet
    /** Row rendered below the collapsible body (process action buttons). */
    footer?: Snippet
    /** Collapsible body. */
    children?: Snippet
    /** Extra classes on the card root (per-card theming). */
    class?: string
    /** Extra styles on the card root (e.g. --tint for sub-agent cards). */
    style?: string
  } = $props()

  function toggle() {
    open = !open
  }
</script>

<div
  class:streaming
  class:dense
  class:open={open}
  class={`collapsible${klass ? ' ' + klass : ''}`}
  {style}
>
  {#if rail}
    <span class="rail" style={`background:${railColor}`} class:streaming aria-hidden="true"></span>
  {/if}

  <div class="head">
    {#if clickable}
      <button class="head-main" onclick={toggle} aria-expanded={open} aria-label={label}>
        {@render header?.()}
      </button>
    {:else}
      <div class="head-main">
        {@render header?.()}
      </div>
    {/if}

    {#if actions}
      <span class="head-actions">
        {@render actions?.()}
      </span>
    {/if}

    {#if chevron}
      <button class="chev" class:open onclick={toggle} aria-label={label ?? 'Toggle'} aria-expanded={open}>
        <Icon name="expand_more" size={16} />
      </button>
    {/if}
  </div>

  <div class="body-grid" class:closed={!open}>
    <div class="body-inner">
      {@render children?.()}
    </div>
  </div>

  {#if footer}
    <div class="collapsible-footer">
      {@render footer?.()}
    </div>
  {/if}
</div>

<style>
  /* Root: elevated-ish card shell, entrance animation shared by all four
     chat blocks (thought / tool / subagent / process). */
  .collapsible {
    position: relative;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-sm);
    overflow: hidden;
    animation: blockIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }

  /* 2px left rail — thought/sub-agent cards only. */
  .rail {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 2px;
    opacity: 0.55;
  }
  .rail.streaming {
    opacity: 1;
    animation: railPulse 1.4s ease-in-out infinite;
  }

  /* Header row: clickable main area + actions + chevron. */
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 11px 7px 13px;
    min-width: 0;
  }
  .dense .head {
    padding: 6px 11px;
  }
  .head-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    text-align: left;
    font-family: inherit;
    transition: background 0.2s ease;
  }
  .head-actions {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  /* Chevron — rotates on open; its own button so ToolCard's non-clickable
     head still collapses (no nested <button>). */
  .chev {
    flex: none;
    display: flex;
    align-items: center;
    background: none;
    border: none;
    padding: 1px;
    color: var(--text-muted);
    cursor: pointer;
    transition: transform 0.25s cubic-bezier(0.22, 0.8, 0.3, 1), color 0.2s ease;
  }
  .chev:hover {
    color: var(--text-primary);
  }
  .chev.open {
    transform: rotate(180deg);
  }

  /* Grid-rows collapse: animates body height without layout jumps. */
  .body-grid {
    display: grid;
    grid-template-rows: 1fr;
    transition: grid-template-rows 0.3s cubic-bezier(0.22, 0.8, 0.3, 1);
  }
  .body-grid.closed {
    grid-template-rows: 0fr;
  }
  .body-inner {
    min-height: 0;
    overflow: hidden;
  }
  .collapsible-footer {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
  }
</style>
