<script lang="ts">
  import Icon from './Icon.svelte';
  import Collapsible from './Collapsible.svelte';

  // Design thought card (`Pragma UI Kit.dc.html` → buildBlock kind 'thought'):
  // bordered slot-tinted card with a left rail, mono uppercase title, collapse
  // chevron + grid-rows body, shimmer streaming title.
  // Rendered from the live `thinking` agent event (streaming until sealed) and
  // from historical thinking/reasoning blocks (never streaming).
  // The card shell (rail + chevron + collapse) lives in Collapsible; this file
  // supplies the thought-specific header styling and body typography.
  let {
    title = 'Thought process',
    meta = '',
    streaming = false,
    children,
  }: {
    title?: string;
    meta?: string;
    streaming?: boolean;
    children?: import('svelte').Snippet;
  } = $props();

  // Design shows thought blocks open by default; the chevron collapses them.
  let open = $state(true);
</script>

<Collapsible
  bind:open
  rail
  railColor="var(--slot)"
  {streaming}
  label={open ? 'Collapse thought process' : 'Expand thought process'}
  class="thought-card"
>
  {#snippet header()}
    <span class="t-icon" class:streaming aria-hidden="true"
      ><Icon name="neurology" size={16} /></span
    >
    <span class="t-title" class:streaming>{title}</span>
    {#if meta}<span class="t-meta">{meta}</span>{/if}
  {/snippet}

  <div class="tc-body">
    {@render children?.()}
    {#if streaming}<span class="t-caret" aria-hidden="true">▌</span>{/if}
  </div>
</Collapsible>

<style>
  /* Root card theme — class lands on Collapsible's root, so the border/bg
     must be global (the class name is unique to this card). */
  :global(.thought-card) {
    border: 1px solid color-mix(in oklch, var(--slot) 26%, transparent);
    background: color-mix(in oklch, var(--slot) 7%, transparent);
    transition:
      border-color 0.3s ease,
      background 0.3s ease;
  }

  .t-icon {
    flex: none;
    display: flex;
    color: var(--slot);
  }
  .t-icon.streaming {
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

  .tc-body {
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
