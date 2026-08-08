<script lang="ts">
  import { parseContent } from '../lib/types'
  import type { Message, ContentBlock } from '../lib/types'

  let { message }: { message: Message } = $props()

  const isUser = $derived(message.role === 'user')
  const content = $derived(parseContent(message.content))

  const textBlocks = $derived(
    Array.isArray(content)
      ? (content as ContentBlock[]).filter((b) => b.type === 'text').map((b) => (b as any).text).join('')
      : typeof content === 'string' ? content : ''
  )

  const toolUseBlocks = $derived(
    Array.isArray(content)
      ? (content as ContentBlock[]).filter((b) => b.type === 'tool_use')
      : []
  )

  // tool_result rows come in a separate user-role message — skip rendering them as chat bubbles
  const isToolResultMsg = $derived(
    Array.isArray(content) &&
    (content as ContentBlock[]).every((b) => b.type === 'tool_result')
  )
</script>

{#if !isToolResultMsg && (!isUser || textBlocks || toolUseBlocks.length > 0)}
  <div class="message" class:user={isUser} class:assistant={!isUser}>
    {#if !isUser}
      <div class="avatar" aria-hidden="true">✦</div>
    {/if}
    <div class="bubble">
      {#if toolUseBlocks.length > 0}
        <div class="tool-list">
          {#each toolUseBlocks as block}
            {@const b = block as any}
            <div class="tool-row" title={JSON.stringify(b.input, null, 2)}>
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" aria-hidden="true"><rect x="1.5" y="2" width="13" height="12" rx="1"/><polyline points="4,6 6.5,8 4,10"/><line x1="7.5" y1="10" x2="10.5" y2="10"/></svg>
              <span class="tool-name">{b.name}</span>
              <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="var(--color-accent-success)" stroke-width="1.6" stroke-linecap="round" aria-hidden="true"><polyline points="3,8 6,11 13,4"/></svg>
            </div>
          {/each}
        </div>
      {/if}
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
  .tool-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-bottom: var(--spacing-2);
  }
  .tool-row {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    cursor: default;
  }
  .tool-name {
    flex: 1;
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