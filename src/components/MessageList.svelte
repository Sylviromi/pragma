<script lang="ts">
  import { tick } from 'svelte'
  import ChatMessage from './ChatMessage.svelte'
  import ToolCard from './ToolCard.svelte'
  import { chat } from '../stores/chat.svelte'
  import { parseContent } from '../lib/types'
  import type { ContentBlock } from '../lib/types'

  let { sessionId }: { sessionId: string } = $props()

  let listEl = $state<HTMLElement | null>(null)
  let userScrolled = $state(false)

  const messages      = $derived(chat.messages[sessionId] ?? [])
  const streamingToken = $derived(chat.streamingToken[sessionId])
  const running       = $derived(chat.running[sessionId] ?? false)
  const liveBlocks    = $derived(chat.liveBlocks[sessionId] ?? [])
  const toolItems     = $derived(chat.toolItems[sessionId] ?? [])
  const error         = $derived(chat.error[sessionId] ?? null)

  const thinking = $derived(running && liveBlocks.length === 0)

  // When a live turn is in progress (liveBlocks non-empty), hide the DB messages for
  // that turn so they don't duplicate what liveBlocks already shows. Only show messages
  // up to and including the last "real" user message (not tool_result).
  const visibleMessages = $derived.by(() => {
    if (liveBlocks.length === 0) return messages
    // Walk backwards to find the last real user message
    for (let i = messages.length - 1; i >= 0; i--) {
      const msg = messages[i]
      if (msg.role === 'user') {
        const c = parseContent(msg.content)
        const isToolResult = Array.isArray(c) &&
          (c as ContentBlock[]).every(b => b.type === 'tool_result')
        if (!isToolResult) {
          return messages.slice(0, i + 1)
        }
      }
    }
    return []
  })

  // Build a map of toolUseId → output from all user tool_result messages.
  // Used by ChatMessage to show outputs in historical (completed) tool cards.
  const toolOutputs = $derived.by(() => {
    const map: Record<string, { content: string; isError: boolean }> = {}
    for (const msg of messages) {
      if (msg.role !== 'user') continue
      const c = parseContent(msg.content)
      if (!Array.isArray(c)) continue
      for (const b of c as ContentBlock[]) {
        if (b.type === 'tool_result') {
          map[(b as any).tool_use_id] = {
            content: (b as any).content ?? '',
            isError: (b as any).is_error ?? false,
          }
        }
      }
    }
    return map
  })

  $effect(() => {
    const _ = messages.length + liveBlocks.length + (streamingToken ?? '').length
              + (thinking ? 1 : 0) + (error ? 1 : 0)
    if (!userScrolled) tick().then(() => { if (listEl) listEl.scrollTop = listEl.scrollHeight })
  })
</script>

<div
  class="list"
  bind:this={listEl}
  onscroll={() => {
    if (listEl) userScrolled = listEl.scrollHeight - listEl.scrollTop - listEl.clientHeight > 50
  }}
  role="log"
  aria-live="polite"
  aria-label="Conversation"
>
  {#each visibleMessages as msg (msg.id)}
    <ChatMessage message={msg} {toolOutputs} />
  {/each}

  {#if thinking}
    <div class="message assistant">
      <div class="avatar" aria-hidden="true">✦</div>
      <div class="bubble thinking-bubble" aria-label="Thinking">
        <span class="dot"></span>
        <span class="dot"></span>
        <span class="dot"></span>
      </div>
    </div>
  {:else if liveBlocks.length > 0}
    <div class="live-turn">
      <div class="avatar" aria-hidden="true">✦</div>
      <div class="live-body">
        {#each liveBlocks as block}
          {#if block.type === 'text'}
            <p class="live-text">{block.text}</p>
          {:else if block.type === 'tool'}
            {@const tool = toolItems.find(t => t.toolUseId === block.toolUseId)}
            {#if tool}
              <ToolCard {sessionId} {tool} />
            {/if}
          {/if}
        {/each}
        {#if running}
          <div class="thinking-tail" aria-label="Generating">
            <span class="dot"></span>
            <span class="dot"></span>
            <span class="dot"></span>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  {#if error}
    <div class="error-row" role="alert">
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true">
        <circle cx="8" cy="8" r="7"/>
        <line x1="8" y1="5" x2="8" y2="8.5"/>
        <circle cx="8" cy="11" r="0.5" fill="currentColor" stroke="none"/>
      </svg>
      <span>{error}</span>
    </div>
  {/if}
</div>

<style>
  .list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-4) 0;
  }

  .message, .live-turn {
    display: flex;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-4);
    align-items: flex-start;
  }

  .avatar {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--color-accent-brand);
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-size: 12px;
    flex-shrink: 0;
    margin-top: 2px;
  }

  .bubble {
    background: var(--color-bg-elevated);
    border-radius: var(--radius-sm) var(--radius-lg) var(--radius-lg) var(--radius-lg);
    padding: var(--spacing-2) var(--spacing-3);
  }
  .thinking-bubble {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: var(--spacing-2) var(--spacing-3);
    min-height: 36px;
    max-width: 70%;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-text-muted);
    animation: bounce 1.2s ease-in-out infinite;
  }
  .dot:nth-child(2) { animation-delay: 0.2s; }
  .dot:nth-child(3) { animation-delay: 0.4s; }
  @keyframes bounce {
    0%, 60%, 100% { transform: translateY(0); opacity: 0.4; }
    30% { transform: translateY(-5px); opacity: 1; }
  }

  /* Compact dots shown inside the live turn while the model is still generating */
  .thinking-tail {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: var(--spacing-1) 0;
  }
  .thinking-tail .dot {
    width: 5px;
    height: 5px;
  }

  .live-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .live-text {
    margin: 0;
    max-width: 70%;
    align-self: flex-start;
    overflow-wrap: anywhere;
    background: var(--color-bg-elevated);
    border-radius: var(--radius-sm) var(--radius-lg) var(--radius-lg) var(--radius-lg);
    padding: var(--spacing-2) var(--spacing-3);
    color: var(--color-text-primary);
    white-space: pre-wrap;
    font-size: var(--font-size-md);
    line-height: 1.6;
  }

  .cursor { animation: blink 1s step-end infinite; }
  @keyframes blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0; }
  }

  .error-row {
    display: flex;
    align-items: flex-start;
    gap: var(--spacing-2);
    margin: var(--spacing-2) var(--spacing-4);
    padding: var(--spacing-2) var(--spacing-3);
    background: color-mix(in srgb, var(--color-accent-error, #f85149) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-accent-error, #f85149) 30%, transparent);
    border-radius: var(--radius-md);
    color: var(--color-accent-error, #f85149);
    font-size: var(--font-size-sm);
  }
  .error-row svg { flex-shrink: 0; margin-top: 1px; }
  .error-row span { white-space: pre-wrap; word-break: break-word; }
</style>
