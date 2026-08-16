<script lang="ts">
  import { tick } from 'svelte';
  import ChatMessage from './ChatMessage.svelte';
  import ToolCard from './ToolCard.svelte';
  import QuestionBlock from './QuestionBlock.svelte';
  import ThoughtCard from './ThoughtCard.svelte';
  import ReturnModal from './ReturnModal.svelte';
  import VerificationBubble from './VerificationBubble.svelte';
  import ContextMenu, { type MenuItem } from './ContextMenu.svelte';
  import {
    chat,
    answerQuestion,
    stopTurn,
    STATUS_META,
    type AgentStatus,
  } from '../stores/chat.svelte';
  import { pushToast } from '../stores/toast.svelte';
  import { buildAgentTranscript } from '../lib/transcript';
  import { messageText, isToolResultTurn, parseContent, parseVerification } from '../lib/types';
  import TextBubble from './TextBubble.svelte';
  import SubagentCard from './SubagentCard.svelte';
  import ProcessCard from './ProcessCard.svelte';
  import Icon from './Icon.svelte';
  import InlineAlert from './InlineAlert.svelte';
  import TurnRow from './TurnRow.svelte';
  import { sessionTint } from '../lib/tint';
  import type { Message, ToolResultBlock } from '../lib/types';

  let { sessionId }: { sessionId: string } = $props();

  let listEl = $state<HTMLElement | null>(null);
  // "Stick to bottom": while the user hasn't scrolled up, keep following
  // streamed text / tool updates so the newest content is always visible.
  let stickToBottom = $state(true);
  // Question-block answers (skeleton) keyed by live block index.
  let qAnswers = $state<Record<number, string>>({});

  const messages = $derived(chat.messages[sessionId] ?? []);
  const streamingToken = $derived(chat.streamingToken[sessionId]);
  const running = $derived(chat.running[sessionId] ?? false);
  const liveBlocks = $derived(chat.liveBlocks[sessionId] ?? []);
  const toolItems = $derived(chat.toolItems[sessionId] ?? []);
  const status = $derived((chat.status[sessionId] ?? 'idle') as AgentStatus);
  const statusMeta = $derived(STATUS_META[status]);
  const error = $derived(chat.error[sessionId] ?? null);
  const notice = $derived(chat.notice[sessionId] ?? null);
  const verifying = $derived(chat.verifying[sessionId] ?? null);
  const verification = $derived(chat.verification[sessionId] ?? null);

  // "Return to this point" target for the modal
  let revertTarget = $state<Message | null>(null);

  // Right-click context menu: message + cursor position
  let ctx = $state<{ msg: Message; x: number; y: number } | null>(null);

  function msgText(m: Message): string {
    return messageText(m.content);
  }

  const ctxItems = $derived.by<MenuItem[]>(() => {
    const c = ctx;
    if (!c) return [];
    return [
      {
        label: 'Copy message text',
        icon: 'copy' as const,
        onSelect: () => {
          navigator.clipboard?.writeText(msgText(c.msg)).catch(() => {});
          ctx = null;
        },
      },
      {
        label: 'Copy as agent transcript',
        icon: 'transcript' as const,
        onSelect: () => {
          const t = buildAgentTranscript(chat.messages[sessionId] ?? [], c.msg.id);
          navigator.clipboard?.writeText(t).then(
            () => pushToast('Agent transcript copied to clipboard'),
            () => pushToast('Could not copy transcript', 'error'),
          );
          ctx = null;
        },
      },
      {
        label: 'Return to this point…',
        icon: 'return' as const,
        hint: running ? 'Stop the current run first (Esc)' : undefined,
        disabled: running,
        onSelect: () => {
          revertTarget = c.msg;
          ctx = null;
        },
      },
    ];
  });

  const thinking = $derived(running && liveBlocks.length === 0);

  // Design streaming meta for the thought card: "<effort> effort" while the
  // model is thinking (off/auto/low/med/high), empty once sealed.
  const effort = $derived((chat.thinking[sessionId] ?? 'auto') as string);
  const thoughtMeta = $derived(thinking ? `${effort} effort` : '');

  // Live indicator (design `Pragma UI Kit`): per-status icon + mono label +
  // bouncing dots (except the waiting-for-you state) + elapsed timer + Stop.
  const INDICATOR: Record<AgentStatus, { icon: string; spin: boolean; still: boolean }> = {
    thinking: { icon: 'neurology', spin: false, still: false },
    working: { icon: 'progress_activity', spin: true, still: false },
    generating: { icon: 'edit', spin: false, still: false },
    waiting: { icon: 'front_hand', spin: false, still: true },
    verifying: { icon: 'fact_check', spin: false, still: false },
    idle: { icon: 'bolt', spin: false, still: false },
  };
  const indMeta = $derived(INDICATOR[status] ?? INDICATOR.idle);
  // Design indicator colors: slot while thinking, warning while working or
  // waiting on the user, accent while generating (differs from STATUS_META,
  // which keeps waiting → danger for the tab dots / session rows).
  const IND_COLORS: Record<AgentStatus, string> = {
    thinking: 'var(--slot)',
    working: 'var(--warning)',
    generating: 'var(--accent)',
    waiting: 'var(--warning)',
    verifying: 'var(--accent)',
    idle: 'var(--accent)',
  };
  const indColor = $derived(IND_COLORS[status] ?? 'var(--accent)');
  let elapsed = $state(0);
  $effect(() => {
    if (!running) {
      elapsed = 0;
      return;
    }
    const started = chat.startedAt[sessionId] ?? Date.now();
    elapsed = Math.max(0, Math.round((Date.now() - started) / 100) / 10);
    const t = setInterval(() => {
      elapsed = Math.max(
        0,
        Math.round((Date.now() - (chat.startedAt[sessionId] ?? Date.now())) / 100) / 10,
      );
    }, 200);
    return () => clearInterval(t);
  });

  // When a live turn is in progress (liveBlocks non-empty), hide the DB messages for
  // that turn so they don't duplicate what liveBlocks already shows. Only show messages
  // up to and including the last "real" user message (not tool_result).
  const visibleMessages = $derived.by(() => {
    if (liveBlocks.length === 0) return messages;
    // Walk backwards to find the last real user message
    for (let i = messages.length - 1; i >= 0; i--) {
      const msg = messages[i];
      if (msg.role === 'user') {
        const c = parseContent(msg.content);
        if (!isToolResultTurn(c)) {
          return messages.slice(0, i + 1);
        }
      }
    }
    return [];
  });

  // Build a map of toolUseId → output from all user tool_result messages.
  // Used by ChatMessage to show outputs in historical (completed) tool cards.
  const toolOutputs = $derived.by(() => {
    const map: Record<string, { content: string; isError: boolean }> = {};
    for (const msg of messages) {
      if (msg.role !== 'user') continue;
      const c = parseContent(msg.content);
      if (!Array.isArray(c)) continue;
      for (const b of c) {
        if (b.type !== 'tool_result') continue;
        const r = b as ToolResultBlock;
        map[r.tool_use_id] = { content: r.content, isError: r.is_error ?? false };
      }
    }
    return map;
  });

  // A key that changes whenever ANY streamed content could have changed the
  // list height: messages, live blocks (incl. growing thought + text text),
  // tool outputs (which arrive after the tool card itself, growing it later),
  // errors.
  const lastMsgId = $derived(
    visibleMessages.length > 0 ? visibleMessages[visibleMessages.length - 1].id : '',
  );
  const liveChars = $derived.by(() =>
    (chat.liveBlocks[sessionId] ?? []).reduce((n, b) => {
      if (b.type === 'thinking' || b.type === 'text') return n + b.text.length;
      return n;
    }, 0),
  );
  const followKey = $derived(
    messages.length +
      liveBlocks.length +
      liveChars +
      (streamingToken ?? '').length +
      (error ? 1 : 0) +
      (verification || verifying ? 1 : 0) +
      (chat.toolItems[sessionId] ?? []).reduce((n, t) => n + (t.output?.length ?? 0), 0),
  );

  $effect(() => {
    const _ = followKey;
    if (!stickToBottom) return;
    tick().then(() => {
      if (!listEl) return;
      // Coalesce to one scroll per frame: token events can arrive faster than
      // frames, and the pre-rAF scrollTop set forced a layout on top of the
      // rAF one. A single rAF-aligned scroll avoids the double layout per
      // token while keeping the list pinned at the bottom.
      if (scrollQueued) return;
      scrollQueued = true;
      requestAnimationFrame(() => {
        scrollQueued = false;
        if (!listEl || !stickToBottom) return;
        listEl.scrollTop = listEl.scrollHeight;
      });
    });
  });

  function onScroll() {
    if (!listEl) return;
    stickToBottom = listEl.scrollHeight - listEl.scrollTop - listEl.clientHeight < 60;
  }

  // Per-frame scroll coalescing flag (see stick-to-bottom effect above).
  let scrollQueued = false;

  function onAnswer(idx: number, option: string) {
    qAnswers[idx] = option;
    const block = liveBlocks[idx];
    if (block?.type === 'question') answerQuestion(sessionId, block.questionId, option);
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
    {#if msg.role === 'verification'}
      {@const vrun = parseVerification(msg)}
      {#if vrun}
        <VerificationBubble run={vrun} />
      {/if}
    {:else}
      <ChatMessage
        message={msg}
        {toolOutputs}
        highlighted={ctx?.msg.id === msg.id}
        animate={msg.id === lastMsgId}
        onContextMenu={(e) => {
          e.preventDefault();
          ctx = { msg, x: e.clientX, y: e.clientY };
        }}
      />
    {/if}
  {/each}

  {#if liveBlocks.length > 0 || thinking}
    <TurnRow {sessionId}>
      <div class="live-body">
        {#each liveBlocks as block, bi (block.type === 'tool' ? block.toolUseId : block.type === 'thinking' ? `th-${bi}` : `${block.type}-${bi}`)}
          {#if block.type === 'thinking'}
            <ThoughtCard
              title={block.done ? 'Thought process' : 'Thinking'}
              meta={block.done ? '' : thoughtMeta}
              streaming={!block.done}>{block.text}</ThoughtCard
            >
          {:else if block.type === 'text'}
            <TextBubble text={block.text} showCaret={running && bi === liveBlocks.length - 1} />
          {:else if block.type === 'tool'}
            {@const tool = toolItems.find((t) => t.toolUseId === block.toolUseId)}
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
          {:else if block.type === 'subagent'}
            <!-- TODO(agents): inert until sub-agent events land (see TODO.md). -->
            <SubagentCard
              name={block.name}
              meta={block.meta}
              runState={block.state}
              task={block.task}
              steps={block.steps}
              tokens={block.tokens}
              result={block.result}
            />
          {:else if block.type === 'process'}
            <!-- TODO(agents): inert until detached-process events land (see TODO.md). -->
            <ProcessCard
              cmd={block.cmd}
              runState={block.state}
              exitCode={block.exitCode}
              meta={block.meta}
              log={block.log}
            />
          {/if}
        {/each}

        {#if running}
          <div
            class="indicator"
            role="status"
            aria-label={statusMeta.label}
            style={`--ind-color:${indColor}`}
          >
            <span class="ind-icon" class:spin={indMeta.spin} aria-hidden="true">
              <Icon name={indMeta.icon as never} size={13} />
            </span>
            <span class="ind-label">{statusMeta.label.toLowerCase()}</span>
            {#if !indMeta.still}
              <span class="ind-dots" aria-hidden="true">
                {#each [0, 1, 2] as i (i)}
                  <span class="ind-dot" style={`animation-delay:${i * 0.16}s`}></span>
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
              <Icon name="stop_circle" size={14} />
            </button>
          </div>
        {/if}
      </div>
    </TurnRow>
  {/if}

  {#if error}
    <InlineAlert kind="error">{error}</InlineAlert>
  {/if}

  {#if notice}
    <InlineAlert kind="info" icon="stop">{notice}</InlineAlert>
  {/if}

  {#if running && (verifying || verification)}
    <!-- Live verification bubble: shows the active phase with a Stop button
         while the harness checks/reviews, then the result until 'done' hands
         over to the persisted role='verification' message. -->
    <VerificationBubble run={verification} phase={verifying} onStop={() => stopTurn(sessionId)} />
  {/if}
</div>

{#if revertTarget}
  <ReturnModal {sessionId} message={revertTarget} onclose={() => (revertTarget = null)} />
{/if}

{#if ctx}
  <ContextMenu x={ctx.x} y={ctx.y} items={ctxItems} onclose={() => (ctx = null)} />
{/if}

<style>
  /* Design log style (v2): measured, centered column — margins grow with
     the window so full-screen adds margin instead of line length. */
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    /* Bound layout/paint effects to the scroll surface so style changes inside
       (hover, streaming) never reflow/paint the rest of the app. */
    contain: content;
    padding: 20px max(24px, calc((100% - 900px) / 2));
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .live-body {
    flex: 1;
    min-width: 0;
    max-width: 78%;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  /* Live indicator — design v2: subtle text row (no bubble): colored icon,
     lowercase mono label, bounce dots (except while waiting), elapsed, stop. */
  .indicator {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 0;
    align-self: flex-start;
    animation: fadeIn 0.3s ease both;
    max-width: 100%;
    margin-top: 2px;
  }
  .ind-icon {
    flex: none;
    display: flex;
    color: var(--ind-color);
    animation: dotPulse 1.5s ease-in-out infinite;
  }
  .ind-icon :global(.msr) {
    font-size: 13px;
    line-height: 1;
  }
  .ind-icon.spin {
    animation: spin 1.1s linear infinite;
  }
  .ind-label {
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .ind-dots {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .ind-dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--ind-color);
    animation: bounceDot 1.1s ease-in-out infinite;
  }
  .ind-elapsed {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    flex: none;
    font-variant-numeric: tabular-nums;
  }
  .ind-stop {
    display: flex;
    align-items: center;
    margin-left: 2px;
    background: none;
    border: none;
    padding: 1px;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0.7;
    transition:
      opacity 0.2s ease,
      color 0.2s ease;
  }
  .ind-stop:hover {
    opacity: 1;
    color: var(--text-primary);
  }
  .ind-stop :global(.msr) {
    font-size: 14px;
    line-height: 1;
  }

  /* Tight windows */
  @media (max-width: 640px) {
    .list {
      padding: 14px 14px;
      gap: 12px;
    }
  }
</style>
