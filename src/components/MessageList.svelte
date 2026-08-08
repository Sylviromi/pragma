<script lang="ts">
  import { tick } from 'svelte'
  import ChatMessage from './ChatMessage.svelte'
  import ToolCard from './ToolCard.svelte'
  import { chat } from '../stores/chat.svelte'

  let { sessionId, autoApprove }: { sessionId: string; autoApprove: boolean } = $props()

  let listEl = $state<HTMLElement | null>(null)
  let userScrolled = $state(false)

  const messages = $derived(chat.messages[sessionId] ?? [])
  const streaming = $derived(chat.streamingToken[sessionId])
  const pendingTools = $derived(chat.pendingTools[sessionId] ?? [])
  const toolResults = $derived(chat.toolResults[sessionId] ?? [])

  $effect(() => {
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

  {#each pendingTools as tool (tool.toolUseId)}
    <div style="padding: 0 var(--spacing-4)">
      <ToolCard
        {sessionId}
        {tool}
        result={toolResults.find((r) => r.toolUseId === tool.toolUseId)}
        {autoApprove}
      />
    </div>
  {/each}

  {#if streaming}
    <div class="message assistant" style="padding: var(--spacing-2) var(--spacing-4)">
      <div class="avatar" aria-hidden="true">✦</div>
      <div class="bubble">
        <p>{streaming}<span class="cursor" aria-hidden="true">▋</span></p>
      </div>
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
  p {
    margin: 0;
    color: var(--color-text-primary);
    white-space: pre-wrap;
  }
  .cursor {
    animation: blink 1s step-end infinite;
  }
  @keyframes blink {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0;
    }
  }
</style>