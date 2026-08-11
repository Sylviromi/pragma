<script lang="ts">
  import { renderMarkdown, segmentsOf, tidy, withCaret } from '../lib/markdown'
  import Icon from './Icon.svelte'

  let {
    text,
    showCaret = false,
  }: {
    text: string
    /** Blinking cursor at the end of the last prose segment (streaming only). */
    showCaret?: boolean
  } = $props()

  // ONE render path for assistant text — used by both the persisted turn
  // (ChatMessage) and the streaming live bubble (MessageList), so the two can
  // never drift apart again (trailing-newline gaps, style mismatches, …).
  // tidy() strips trailing whitespace BEFORE markdown so pre-wrap never adds
  // a phantom empty line at the bubble's end.
  const segs = $derived(segmentsOf(tidy(text)))
  const last = $derived(segs.length - 1)

  async function copyCode(code: string) {
    try {
      await navigator.clipboard.writeText(code)
    } catch { /* clipboard unavailable */ }
  }
</script>

{#if segs.length > 0}
  <div class="text-bubble">
    {#each segs as seg, si (seg.kind + seg.text.slice(0, 40))}
      {#if seg.kind === 'prose' && seg.text.trim()}
        <div class="md-prose">
          {@html showCaret && si === last ? withCaret(renderMarkdown(seg.text)) : renderMarkdown(seg.text)}
        </div>
      {:else if seg.kind === 'code' && seg.text.trim()}
        <div class="code-block">
          <button
            class="copy"
            aria-label="Copy code"
            title="Copy code"
            onclick={() => copyCode(seg.text)}
          ><Icon name="content_copy" size={14} /></button>
          <pre>{seg.text.trimEnd()}</pre>
        </div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  /* Shared assistant text bubble — design: elevated, asymmetric radius. */
  .text-bubble {
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 5px 18px 18px 18px;
    padding: 11px 16px;
    font-family: var(--font-body);
    font-size: 14px;
    color: var(--text-primary);
    line-height: 1.55;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  :global(.caret) {
    color: var(--accent);
    animation: cursorblink 1s step-start infinite;
  }
  @keyframes cursorblink {
    50% { opacity: 0; }
  }
  .code-block {
    position: relative;
    margin-top: 10px;
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    padding: 12px 14px;
    font-family: var(--font-mono);
    font-size: 12.5px;
    color: var(--text-secondary);
  }
  .code-block pre {
    margin: 0;
    white-space: pre;
    overflow-x: auto;
  }
  .copy {
    position: absolute;
    top: 6px;
    right: 6px;
    background: var(--bg-overlay);
    border: 1px solid var(--border-default);
    border-radius: 2px;
    color: var(--text-muted);
    cursor: pointer;
    padding: 3px;
    display: flex;
  }
  .copy:hover {
    color: var(--text-primary);
  }
</style>