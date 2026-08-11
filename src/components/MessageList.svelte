<script lang="ts">
  import { tick } from 'svelte'
  import ChatMessage from './ChatMessage.svelte'
  import ToolCard from './ToolCard.svelte'
  import QuestionBlock from './QuestionBlock.svelte'
  import ThoughtCard from './ThoughtCard.svelte'
  import ReturnModal from './ReturnModal.svelte'
  import ContextMenu, { type MenuItem } from './ContextMenu.svelte'
  import { chat, answerQuestion, stopTurn, STATUS_META, type AgentStatus } from '../stores/chat.svelte'
  import { pushToast } from '../stores/toast.svelte'
  import { buildAgentTranscript } from '../lib/transcript'
  import { messageText, isToolResultTurn, parseContent } from '../lib/types'
  import type { Message, ToolResultBlock } from '../lib/types'

  let { sessionId }: { sessionId: string } = $props()

  let listEl = $state<HTMLElement | null>(null)
  // "Stick to bottom": while the user hasn't scrolled up, keep following
  // streamed text / tool updates so the newest content is always visible.
  let stickToBottom = $state(true)
  // Question-block answers (skeleton) keyed by live block index.
  let qAnswers = $state<Record<number, string>>({})

  const messages      = $derived(chat.messages[sessionId] ?? [])
  const streamingToken = $derived(chat.streamingToken[sessionId])
  const running       = $derived(chat.running[sessionId] ?? false)
  const liveBlocks    = $derived(chat.liveBlocks[sessionId] ?? [])
  const toolItems     = $derived(chat.toolItems[sessionId] ?? [])
  const status        = $derived((chat.status[sessionId] ?? 'idle') as AgentStatus)
  const statusMeta    = $derived(STATUS_META[status])
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

  const thinking = $derived(running && liveBlocks.length === 0)

  // Design streaming meta for the thought card: "<effort> effort" while the
  // model is thinking (off/auto/low/med/high), empty once sealed.
  const effort = $derived((chat.thinking[sessionId] ?? 'auto') as string)
  const thoughtMeta = $derived(thinking ? `${effort} effort` : '')

  // Live indicator (design `Pragma UI Kit`): per-status icon + mono label +
  // bouncing dots (except the waiting-for-you state) + elapsed timer + Stop.
  const INDICATOR: Record<AgentStatus, { icon: string; spin: boolean; still: boolean }> = {
    thinking:   { icon: 'neurology',          spin: false, still: false },
    working:    { icon: 'progress_activity',  spin: true,  still: false },
    generating: { icon: 'edit',               spin: false, still: false },
    waiting:    { icon: 'front_hand',         spin: false, still: true },
    idle:       { icon: 'bolt',               spin: false, still: false },
  }
  const indMeta = $derived(INDICATOR[status] ?? INDICATOR.idle)
  // Design indicator colors: slot while thinking, warning while working or
  // waiting on the user, accent while generating (differs from STATUS_META,
  // which keeps waiting → danger for the tab dots / session rows).
  const IND_COLORS: Record<AgentStatus, string> = {
    thinking: 'var(--slot)',
    working: 'var(--warning)',
    generating: 'var(--accent)',
    waiting: 'var(--warning)',
    idle: 'var(--accent)',
  }
  const indColor = $derived(IND_COLORS[status] ?? 'var(--accent)')
  let elapsed = $state(0)
  $effect(() => {
    if (!running) {
      elapsed = 0
      return
    }
    const started = chat.startedAt[sessionId] ?? Date.now()
    elapsed = Math.max(0, Math.round((Date.now() - started) / 100) / 10)
    const t = setInterval(() => {
      elapsed = Math.max(0, Math.round((Date.now() - (chat.startedAt[sessionId] ?? Date.now())) / 100) / 10)
    }, 200)
    return () => clearInterval(t)
  })

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
  // list height: messages, live blocks (incl. growing thought + text text),
  // tool outputs (which arrive after the tool card itself, growing it later),
  // errors.
  const lastMsgId = $derived(
    visibleMessages.length > 0 ? visibleMessages[visibleMessages.length - 1].id : ''
  )
  const liveChars = $derived.by(() =>
    (chat.liveBlocks[sessionId] ?? []).reduce((n, b) => {
      if (b.type === 'thinking' || b.type === 'text') return n + b.text.length
      return n
    }, 0)
  )
  const followKey = $derived(
    messages.length
    + liveBlocks.length
    + liveChars
    + (streamingToken ?? '').length
    + (error ? 1 : 0)
    + (chat.toolItems[sessionId] ?? []).reduce((n, t) => n + (t.output?.length ?? 0), 0)
  )

  $effect(() => {
    const _ = followKey
    if (!stickToBottom) return
    tick().then(() => {
      if (!listEl) return
      // Coalesce to one scroll per frame: token events can arrive faster than
      // frames, and the pre-rAF scrollTop set forced a layout on top of the
      // rAF one. A single rAF-aligned scroll avoids the double layout per
      // token while keeping the list pinned at the bottom.
      if (scrollQueued) return
      scrollQueued = true
      requestAnimationFrame(() => {
        scrollQueued = false
        if (!listEl || !stickToBottom) return
        listEl.scrollTop = listEl.scrollHeight
      })
    })
  })

  function onScroll() {
    if (!listEl) return
    stickToBottom = listEl.scrollHeight - listEl.scrollTop - listEl.clientHeight < 60
  }

  // Per-frame scroll coalescing flag (see stick-to-bottom effect above).
  let scrollQueued = false

  function onAnswer(idx: number, option: string) {
    qAnswers[idx] = option
    const block = liveBlocks[idx]
    if (block?.type === 'question') answerQuestion(sessionId, block.questionId, option)
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
      animate={msg.id === lastMsgId}
      onContextMenu={(e) => {
        e.preventDefault()
        ctx = { msg, x: e.clientX, y: e.clientY }
      }}
    />
  {/each}

  {#if liveBlocks.length > 0 || thinking}
    <div class="live-turn">
      <div class="avatar" aria-hidden="true">P</div>
      <div class="live-body">
        {#each liveBlocks as block, bi (block.type === 'tool' ? block.toolUseId : block.type === 'thinking' ? `th-${bi}` : `${block.type}-${bi}`)}
          {#if block.type === 'thinking'}
            <ThoughtCard
              title={block.done ? 'Thought process' : 'Thinking'}
              meta={block.done ? '' : thoughtMeta}
              streaming={!block.done}
            >{block.text}</ThoughtCard>
          {:else if block.type === 'text'}
            <div class="live-bubble">{block.text}{#if running && bi === liveBlocks.length - 1}<span class="caret" aria-hidden="true">▌</span>{/if}</div>
          {:else if block.type === 'tool'}
            {@const tool = toolItems.find(t => t.toolUseId === block.toolUseId)}
            {#if tool}
              <ToolCard {sessionId} {tool} />
            {/if}
          {:else if block.type === 'question'}
            <QuestionBlock
              text={block.text}
              options={block.options}
              answer={block.picked ?? qAnswers[bi] ?? null}
              onSelect={(opt) => onAnswer(bi, opt)}
            />
          {/if}
        {/each}

        {#if running}
          <div
            class="indicator"
            role="status"
            aria-label={statusMeta.label}
            style={`border-color:${indColor}`}
          >
            <span class="ind-icon" class:spin={indMeta.spin} style={`color:${indColor}`} aria-hidden="true">
              <span class="msr">{indMeta.icon}</span>
            </span>
            <span class="ind-label">{statusMeta.label}</span>
            {#if !indMeta.still}
              <span class="ind-dots" aria-hidden="true">
                {#each [0, 1, 2] as i (i)}
                  <span class="ind-dot" style={`background:${indColor}; animation-delay:${i * 0.16}s`}></span>
                {/each}
              </span>
            {/if}
            {#if elapsed > 0}<span class="ind-elapsed">{elapsed.toFixed(1)}s</span>{/if}
            <button
              class="ind-stop"
              onclick={() => stopTurn(sessionId)}
              title="Stop the response"
              aria-label="Stop generating"
            >
              <span class="msr" aria-hidden="true">stop</span>Stop
            </button>
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
      <span class="msr" aria-hidden="true">stop</span>
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
  /* Design log style: 20px 24px padding, 16px gaps (tight at small widths). */
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .live-turn {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }
  .avatar {
    width: 24px;
    height: 24px;
    flex: none;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-on);
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    margin-top: 2px;
  }
  .live-body {
    flex: 1;
    min-width: 0;
    max-width: 78%;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  /* Live text — design text bubble */
  .live-bubble {
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
    animation: blockIn 0.32s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  .caret {
    color: var(--accent);
    animation: cursorblink 1s step-start infinite;
  }

  /* Live indicator — design: bubble, icon + mono label + bounce
     dots (except while waiting for the user) + elapsed + Stop. */
  .indicator {
    display: flex;
    align-items: center;
    gap: 9px;
    background: var(--bg-elevated);
    border: 1px solid var(--accent);
    border-radius: 5px 16px 16px 16px;
    padding: 7px 12px 7px 11px;
    align-self: flex-start;
    animation: bubbleIn 0.3s cubic-bezier(0.22, 0.8, 0.3, 1) both;
    transition: border-color 0.3s ease;
    max-width: 100%;
  }
  .ind-icon {
    flex: none;
    display: flex;
    animation: dotPulse 1.5s ease-in-out infinite;
  }
  .ind-icon .msr {
    font-size: 16px;
    line-height: 1;
  }
  .ind-icon.spin {
    animation: spin 1.1s linear infinite;
  }
  .ind-label {
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-secondary);
    white-space: nowrap;
  }
  .ind-dots {
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .ind-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    flex: none;
    animation: bounceDot 1.1s ease-in-out infinite;
  }
  .ind-elapsed {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    flex: none;
    font-variant-numeric: tabular-nums;
  }
  .ind-stop {
    display: flex;
    align-items: center;
    gap: 3px;
    margin-left: 4px;
    background: none;
    border: 1px solid var(--border-default);
    border-radius: 11px;
    color: var(--text-muted);
    font-family: var(--font-body);
    font-size: 11px;
    padding: 2px 9px 2px 6px;
    cursor: pointer;
    transition: color 0.2s ease, border-color 0.2s ease;
  }
  .ind-stop:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
  }
  .ind-stop .msr {
    font-size: 12px;
    line-height: 1;
  }

  .error-row {
    display: flex;
    align-items: flex-start;
    gap: var(--spacing-2);
    padding: 8px 12px;
    background: color-mix(in srgb, var(--danger) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--danger) 30%, transparent);
    border-radius: var(--radius-md);
    color: var(--danger);
    font-size: var(--font-size-sm);
  }
  .error-row svg { flex-shrink: 0; margin-top: 1px; }
  .error-row span { white-space: pre-wrap; word-break: break-word; }

  .notice-row {
    display: flex;
    align-items: flex-start;
    gap: var(--spacing-2);
    padding: 8px 12px;
    background: color-mix(in srgb, var(--text-muted) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--text-muted) 25%, transparent);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font-size: var(--font-size-sm);
  }
  .notice-row .msr { font-size: 14px; flex-shrink: 0; }

  /* Tight windows */
  @media (max-width: 640px) {
    .list {
      padding: 14px 14px;
      gap: 12px;
    }
  }
</style>