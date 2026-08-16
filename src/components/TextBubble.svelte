<script lang="ts">
  import { renderMarkdown, segmentsOf, tidy, withCaret } from '../lib/markdown';
  import Icon from './Icon.svelte';

  let {
    text,
    showCaret = false,
  }: {
    text: string;
    /** Blinking cursor at the end of the last prose segment (streaming only). */
    showCaret?: boolean;
  } = $props();

  // ONE render path for assistant text — used by both the persisted turn
  // (ChatMessage) and the streaming live bubble (MessageList), so the two can
  // never drift apart again (trailing-newline gaps, style mismatches, …).
  // tidy() strips trailing whitespace BEFORE markdown so pre-wrap never adds
  // a phantom empty line at the bubble's end.
  //
  // Streaming path: tokens arrive faster than frames, and every single one
  // would re-run segmentsOf + tidy + marked.parse + the caret regex + an
  // innerHTML replacement over the WHOLE accumulated text (the markdown cache
  // can't help — the text changes every token, so it always misses). Throttle
  // live updates to ~12/sec so that work is bounded; the persisted path
  // (showCaret=false) renders once, unchanged, and flips instantly when a
  // stream ends.
  let liveText = $state('');
  let lastRender = 0;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let pending = '';

  $effect(() => {
    if (!showCaret) {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
      liveText = text;
      pending = text;
      return;
    }
    pending = text;
    const now = Date.now();
    const wait = 80 - (now - lastRender);
    if (wait <= 0) {
      liveText = pending;
      lastRender = now;
    } else if (!timer) {
      timer = setTimeout(() => {
        timer = null;
        lastRender = Date.now();
        liveText = pending;
      }, wait);
    }
    return () => {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
    };
  });

  const segs = $derived(segmentsOf(tidy(showCaret ? liveText : text)));
  const last = $derived(segs.length - 1);

  async function copyCode(code: string) {
    try {
      await navigator.clipboard.writeText(code);
    } catch {
      /* clipboard unavailable */
    }
  }
</script>

{#if segs.length > 0}
  <div class="text-bubble">
    {#each segs as seg, si (seg.kind + seg.text.slice(0, 40))}
      {#if seg.kind === 'prose' && seg.text.trim()}
        <div class="md-prose">
          {@html showCaret && si === last
            ? withCaret(renderMarkdown(seg.text))
            : renderMarkdown(seg.text)}
        </div>
      {:else if seg.kind === 'code' && seg.text.trim()}
        <div class="code-block">
          <button
            class="copy"
            aria-label="Copy code"
            title="Copy code"
            onclick={() => copyCode(seg.text)}><Icon name="content_copy" size={14} /></button
          >
          <pre>{seg.text.trimEnd()}</pre>
        </div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  /* Shared assistant text bubble — design: elevated, asymmetric radius,
     blockIn entrance (matches ThoughtCard/ToolCard so every chat block
     animates consistently). */
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
    animation: blockIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  :global(.caret) {
    color: var(--accent);
    animation: cursorblink 1s step-start infinite;
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
