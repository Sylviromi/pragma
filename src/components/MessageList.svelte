<script lang="ts">
  import { tick } from 'svelte'
  import ChatMessage from './ChatMessage.svelte'
  import ToolCard from './ToolCard.svelte'
  import ReturnModal from './ReturnModal.svelte'
  import ContextMenu, { type MenuItem } from './ContextMenu.svelte'
  import Icon from './Icon.svelte'
  import { chat } from '../stores/chat.svelte'
  import { pushToast } from '../stores/toast.svelte'
  import { buildAgentTranscript } from '../lib/transcript'
  import { messageText, isToolResultTurn, parseContent } from '../lib/types'
  import type { Message, ToolResultBlock } from '../lib/types'

  let { sessionId }: { sessionId: string } = $props()

  let listEl = $state<HTMLElement | null>(null)
  // "Stick to bottom": while the user hasn't scrolled up, keep following
  // streamed text / tool updates so the newest content is always visible.
  let stickToBottom = $state(true)

  const messages      = $derived(chat.messages[sessionId] ?? [])
  const streamingToken = $derived(chat.streamingToken[sessionId])
  const running       = $derived(chat.running[sessionId] ?? false)
  const liveBlocks    = $derived(chat.liveBlocks[sessionId] ?? [])
  const toolItems     = $derived(chat.toolItems[sessionId] ?? [])
  const error         = $derived(chat.error[sessionId] ?? null)
  const notice        = $derived(chat.notice[sessionId] ?? null)

  // "Return to this point" target for the modal
  let revertTarget = $state<Message | null>(null)

  // Right-click context menu: message + cursor position
  let ctx = $state<{ msg: Message; x: number; y: number } | null>(null)

  function msgText(m: Message): string {
    return messageText(m.content)
  }

  const ctxItems = $derived.by<MenuItem[]>(() => {
    const c = ctx
    if (!c) return []
    return [
      {
        label: 'Copy message text',
        icon: 'copy' as const,
        onSelect: () => {
          navigator.clipboard?.writeText(msgText(c.msg)).catch(() => {})
          ctx = null
        },
      },
      {
        label: 'Copy as agent transcript',
        icon: 'transcript' as const,
        onSelect: () => {
          const t = buildAgentTranscript(chat.messages[sessionId] ?? [], c.msg.id)
          navigator.clipboard?.writeText(t).then(
            () => pushToast('Agent transcript copied to clipboard'),
            () => pushToast('Could not copy transcript', 'error')
          )
          ctx = null
        },
      },
      {
        label: 'Return to this point…',
        icon: 'return' as const,
        hint: running ? 'Stop the current run first (Esc)' : undefined,
        disabled: running,
        onSelect: () => {
          revertTarget = c.msg
          ctx = null
        },
      },
    ]
  })

  const liveReasoningText = $derived(chat.liveReasoning[sessionId] ?? '')
  const thinking = $derived(running && liveBlocks.length === 0 && !liveReasoningText)

  // Cycling status label shown while the model is thinking (no content yet)
  const statusWords = ['Thinking', 'Working', 'Generating']
  let statusIndex = $state(0)
  $effect(() => {
    if (!thinking) return
    statusIndex = 0
    const t = setInterval(() => {
      statusIndex = (statusIndex + 1) % statusWords.length
    }, 1400)
    return () => clearInterval(t)
  })
  const statusWord = $derived(statusWords[statusIndex])

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
        if (!isToolResultTurn(c)) {
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
      for (const b of c) {
        if (b.type !== 'tool_result') continue
        const r = b as ToolResultBlock
        map[r.tool_use_id] = { content: r.content, isError: r.is_error ?? false }
      }
    }
    return map
  })

  // A key that changes whenever ANY streamed content could have changed the
  // list height: messages, live blocks, reasoning, live text, tool outputs
  // (which arrive after the tool card itself, growing it later), errors.
  const followKey = $derived(
    messages.length
    + liveBlocks.length
    + liveReasoningText.length
    + (streamingToken ?? '').length
    + (error ? 1 : 0)
    + (chat.toolItems[sessionId] ?? []).reduce((n, t) => n + (t.output?.length ?? 0), 0)
  )

  $effect(() => {
    const _ = followKey
    if (!stickToBottom) return
    tick().then(() => {
      if (!listEl) return
      listEl.scrollTop = listEl.scrollHeight
      // Second pass after layout settles — tool cards/expanded outputs can
      // grow the list right after the first scroll.
      requestAnimationFrame(() => {
        if (listEl && stickToBottom) listEl.scrollTop = listEl.scrollHeight
      })
    })
  })

  function onScroll() {
    if (!listEl) return
    stickToBottom = listEl.scrollHeight - listEl.scrollTop - listEl.clientHeight < 60
  }
</script>

<div
  class="list"
  bind:this={listEl}
  onscroll={onScroll}
  role="log"
  aria-live="polite"
  aria-label="Conversation"
>
  {#each visibleMessages as msg (msg.id)}
    <ChatMessage
      message={msg}
      {toolOutputs}
      highlighted={ctx?.msg.id === msg.id}
      onContextMenu={(e) => {
        e.preventDefault()
        ctx = { msg, x: e.clientX, y: e.clientY }
      }}
    />
  {/each}

  {#if liveReasoningText}
    <div class="live-turn">
      <div class="avatar" aria-hidden="true"><Icon name="sparkle" size={13} /></div>
      <div class="live-body">
        <details class="thought-card" open>
          <summary>
            <span class="thought-label">Thinking…</span>
            <span class="thought-status">{(liveBlocks.length > 0 ? 'answered — click to expand' : 'streaming')}</span>
          </summary>
          <div class="thought-text">{liveReasoningText}</div>
        </details>
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
          <div class="stream-indicator" aria-label="Generating">
            <span class="stream-caret" aria-hidden="true"></span>
          </div>
        {/if}
      </div>
    </div>
  {:else if thinking}
    <div class="message assistant">
      <div class="avatar" aria-hidden="true"><Icon name="sparkle" size={13} /></div>
      <div class="status-pill" role="status" aria-label="Assistant is thinking">
        <span class="status-dot" aria-hidden="true"></span>
        <span class="status-text">
          {#key statusWord}<span class="status-word">{statusWord}</span>{/key}
        </span>
      </div>
    </div>
  {:else if liveBlocks.length > 0}
    <div class="live-turn">
      <div class="avatar" aria-hidden="true"><Icon name="sparkle" size={13} /></div>
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
          <div class="stream-indicator" aria-label="Generating">
            <span class="stream-caret" aria-hidden="true"></span>
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

  {#if notice}
    <div class="notice-row" role="status">
      <span><Icon name="stop" size={14} /></span>
      <span>{notice}</span>
    </div>
  {/if}
</div>

{#if revertTarget}
  <ReturnModal
    {sessionId}
    message={revertTarget}
    onclose={() => (revertTarget = null)}
  />
{/if}

{#if ctx}
  <ContextMenu x={ctx.x} y={ctx.y} items={ctxItems} onclose={() => (ctx = null)} />
{/if}

<style>
  .list {
    flex: 1;
    min-height: 0; /* never push the composer off-window on short screens */
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
  /* Status pill — shown while the model is thinking (no content yet) */
  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--color-accent-brand) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-accent-brand) 22%, transparent);
    min-height: 36px;
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--color-accent-brand);
    animation: status-pulse 1.4s ease-in-out infinite;
  }
  .status-text {
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
  }
  .status-word {
    display: inline-block;
    animation: status-fade 0.4s ease;
  }
  @keyframes status-pulse {
    0%, 100% { transform: scale(1); box-shadow: 0 0 0 0 color-mix(in srgb, var(--color-accent-brand) 35%, transparent); }
    50% { transform: scale(0.7); box-shadow: 0 0 0 5px color-mix(in srgb, var(--color-accent-brand) 0%, transparent); }
  }
  @keyframes status-fade {
    from { opacity: 0; transform: translateY(3px); }
    to { opacity: 1; transform: translateY(0); }
  }

  /* Live/stopped thinking card — shows the model's reasoning as it arrives,
     collapses to a chip once the visible answer starts streaming. */
  .thought-card {
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--color-accent-brand) 5%, var(--color-bg-elevated));
    border: 1px solid color-mix(in srgb, var(--color-accent-brand) 18%, transparent);
    margin-bottom: var(--spacing-2);
    max-width: 100%;
  }
  .thought-card summary {
    cursor: pointer;
    list-style: none;
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    user-select: none;
    -webkit-user-select: none;
  }
  .thought-card summary::-webkit-details-marker { display: none; }
  .thought-label {
    font-size: var(--font-size-sm);
    color: color-mix(in srgb, var(--color-accent-brand) 75%, var(--color-text-secondary));
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-2);
  }
  .thought-label::before {
    content: '';
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-accent-brand);
    animation: status-pulse 1.4s ease-in-out infinite;
  }
  .thought-status {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    margin-left: auto;
    white-space: nowrap;
  }
  .thought-text {
    padding: 0 var(--spacing-3) var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-sm);
    line-height: 1.55;
    color: var(--color-text-secondary);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 42vh;
    overflow-y: auto;
  }

  /* Stream caret — blinks while tokens are still flowing after the live text */
  .stream-indicator {
    display: flex;
    align-items: center;
    padding: 0 var(--spacing-1);
  }
  .stream-caret {
    width: 2px;
    height: 15px;
    border-radius: 1px;
    background: var(--color-accent-brand);
    animation: caret-blink 1s steps(2, start) infinite;
  }
  @keyframes caret-blink {
    0%, 55% { opacity: 1; }
    56%, 100% { opacity: 0; }
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
    width: fit-content;
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

  .notice-row {
    display: flex;
    align-items: flex-start;
    gap: var(--spacing-2);
    margin: var(--spacing-2) var(--spacing-4);
    padding: var(--spacing-2) var(--spacing-3);
    background: color-mix(in srgb, var(--color-text-muted) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-text-muted) 25%, transparent);
    border-radius: var(--radius-md);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
  .notice-row span:first-child { flex-shrink: 0; }
</style>
