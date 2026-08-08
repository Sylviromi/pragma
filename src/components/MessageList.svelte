<script lang="ts">
  import { tick } from 'svelte'
  import ChatMessage from './ChatMessage.svelte'
  import ToolCard from './ToolCard.svelte'
  import { chat } from '../stores/chat.svelte'

  let { sessionId }: { sessionId: string } = $props()

  let listEl = $state<HTMLElement | null>(null)
  let userScrolled = $state(false)

  const messages = $derived(chat.messages[sessionId] ?? [])
  const streaming = $derived(chat.streamingToken[sessionId])
  const running = $derived(chat.running[sessionId] ?? false)
  const toolItems = $derived(chat.toolItems[sessionId] ?? [])
  const error = $derived(chat.error[sessionId] ?? null)

  // thinking = running but no tokens yet and no tools yet
  const thinking = $derived(running && streaming === '' && toolItems.length === 0)

  $effect(() => {
    // track anything that changes the list height
    const _ = messages.length + (streaming ?? '').length + toolItems.length + (thinking ? 1 : 0) + (error ? 1 : 0)
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
  {#each messages as msg (msg.id)}
    <ChatMessage message={msg} />
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
  {:else if streaming}
    <div class="message assistant">
      <div class="avatar" aria-hidden="true">✦</div>
      <div class="bubble stream-bubble">
        <p>{streaming}<span class="cursor" aria-hidden="true">▋</span></p>
      </div>
    </div>
  {/if}

  {#each toolItems as tool (tool.toolUseId)}
    <div style="padding: 0 var(--spacing-4)">
      <ToolCard {sessionId} {tool} />
    </div>
  {/each}

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
  .message {
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
    font-size: 10px;
    flex-shrink: 0;
    margin-top: 2px;
  }
  .bubble {
    background: var(--color-bg-elevated);
    border-radius: var(--radius-sm) var(--radius-lg) var(--radius-lg) var(--radius-lg);
    padding: var(--spacing-2) var(--spacing-3);
  }
  .stream-bubble {
    max-width: 70%;
  }
  .thinking-bubble {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: var(--spacing-2) var(--spacing-3);
    min-height: 36px;
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
  p {
    margin: 0;
    color: var(--color-text-primary);
    white-space: pre-wrap;
  }
  .cursor {
    animation: blink 1s step-end infinite;
  }
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
