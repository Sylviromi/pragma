<script lang="ts">
  import Icon from './Icon.svelte'
  import { theme } from '../stores/theme.svelte'

  // Design thought card (`Pragma UI Kit.dc.html` → buildBlock kind 'thought'):
  // bordered slot-tinted card with a left rail, mono uppercase title, collapse
  // chevron + grid-rows body, shimmer streaming title, thinkGlow while live.
  // Rendered from the live `thinking` agent event (streaming until sealed) and
  // from historical thinking/reasoning blocks (never streaming).
  let {
    title = 'Thought process',
    meta = '',
    streaming = false,
    children,
  }: {
    title?: string
    meta?: string
    streaming?: boolean
    children?: import('svelte').Snippet
  } = $props()

  // Design shows thought blocks open by default; the chevron collapses them.
  let open = $state(true)
</script>

<div class="thought-card" class:streaming>
  <span class="rail" aria-hidden="true"></span>
  <button
    class="head"
    onclick={() => (open = !open)}
    aria-expanded={open}
    aria-label={open ? 'Collapse thought process' : 'Expand thought process'}
  >
    <span class="t-icon" aria-hidden="true"><Icon name="neurology" size={16} /></span>
    <span class="t-title" class:streaming>{title}</span>
    {#if meta}<span class="t-meta">{meta}</span>{/if}
    <span class="t-chev" class:open aria-hidden="true"><Icon name="expand_more" size={16} /></span>
  </button>
  <div class="body-grid" class:closed={!open}>
    <div class="body">
      {@render children?.()}
      {#if streaming}<span class="t-caret" aria-hidden="true">▌</span>{/if}
    </div>
  </div>
</div>

<style>
  /* Design thought block — slot-tinted elevated card, 2px left rail. */
  .thought-card {
    position: relative;
    display: flex;
    flex-direction: column;
    border: 1px solid color-mix(in oklch, var(--slot) 26%, transparent);
    border-radius: var(--radius-sm);
    background: color-mix(in oklch, var(--slot) 7%, transparent);
    overflow: hidden;
    transition: border-color 0.3s ease, background 0.3s ease;
    animation: blockIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  /* Streaming: breathing glow around the card. */
  .thought-card.streaming {
    animation: blockIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both,
      thinkGlow 2.2s ease-in-out infinite;
  }
  .rail {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--slot);
    opacity: 0.55;
  }
  .thought-card.streaming .rail {
    opacity: 1;
    animation: railPulse 1.4s ease-in-out infinite;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 11px 7px 13px;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    transition: background 0.2s ease;
  }
  .head:hover {
    background: color-mix(in oklch, var(--slot) 6%, transparent);
  }
  .t-icon {
    flex: none;
    display: flex;
    color: var(--slot);
  }
  .thought-card.streaming .t-icon {
    animation: dotPulse 1.6s ease-in-out infinite;
  }

  .t-title {
    flex: none;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--slot);
  }
  /* Streaming title: shimmer sweep over the gradient text. */
  .t-title.streaming {
    background: linear-gradient(
      90deg,
      var(--text-muted) 0%,
      var(--slot) 30%,
      var(--text-primary) 50%,
      var(--slot) 70%,
      var(--text-muted) 100%
    );
    background-size: 200% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    animation: shimmer 2.2s linear infinite;
  }
  .t-meta {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .t-chev {
    flex: none;
    display: flex;
    color: var(--text-muted);
    transition: transform 0.25s cubic-bezier(0.22, 0.8, 0.3, 1);
  }
  .t-chev.open {
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
  .body {
    min-height: 0;
    overflow: hidden;
    padding: 0 14px 12px 13px;
    font-family: var(--font-body);
    font-size: 13px;
    line-height: 1.7;
    color: var(--text-secondary);
    text-wrap: pretty;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .t-caret {
    color: var(--slot);
    margin-left: 1px;
    animation: cursorblink 1s step-start infinite;
  }
</style>