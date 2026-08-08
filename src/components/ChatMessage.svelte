<script lang="ts">
  import { parseContent } from '../lib/types'
  import type { Message } from '../lib/types'

  let { message }: { message: Message } = $props()

  const isUser = $derived(message.role === 'user')
  const content = $derived(parseContent(message.content))
  const textBlocks = $derived(
    Array.isArray(content)
      ? content.filter((b: any) => b.type === 'text').map((b: any) => b.text).join('')
      : typeof content === 'string'
        ? content
        : ''
  )
  const hasToolBlocks = $derived(
    Array.isArray(content) &&
      content.some((b: any) => b.type === 'tool_use' || b.type === 'tool_result')
  )
</script>

{#if !isUser || textBlocks}
  <div class="message" class:user={isUser} class:assistant={!isUser}>
    {#if !isUser}
      <div class="avatar" aria-hidden="true">✦</div>
    {/if}
    <div class="bubble">
      {#if textBlocks}
        <p>{textBlocks}</p>
      {/if}
      {#if !isUser && message.model}
        <div class="meta">{message.model}</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .message {
    display: flex;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-4);
    align-items: flex-start;
  }
  .message.user {
    flex-direction: row-reverse;
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
    max-width: 70%;
    background: var(--color-bg-elevated);
    border-radius: var(--radius-lg);
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-md);
    line-height: 1.6;
  }
  .user .bubble {
    background: color-mix(in srgb, var(--color-accent-brand) 15%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-accent-brand) 30%, transparent);
    border-radius: var(--radius-lg) var(--radius-lg) var(--radius-sm) var(--radius-lg);
  }
  .assistant .bubble {
    border-radius: var(--radius-sm) var(--radius-lg) var(--radius-lg) var(--radius-lg);
  }
  p {
    margin: 0;
    color: var(--color-text-primary);
    white-space: pre-wrap;
  }
  .meta {
    margin-top: var(--spacing-1);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }
</style>