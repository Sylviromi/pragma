<script lang="ts">
  import { parseContent } from '../lib/types'
  import type { Message, ContentBlock } from '../lib/types'
  import ToolCard from './ToolCard.svelte'

  let {
    message,
    toolOutputs = {},
  }: {
    message: Message
    toolOutputs: Record<string, { content: string; isError: boolean }>
  } = $props()

  const isUser = $derived(message.role === 'user')
  const content = $derived(parseContent(message.content))
  const blocks = $derived(Array.isArray(content) ? content as ContentBlock[] : null)

  const userText = $derived(
    typeof content === 'string'
      ? content
      : Array.isArray(content)
        ? (content as ContentBlock[]).filter(b => b.type === 'text').map(b => (b as any).text).join('')
        : ''
  )

  const isToolResultMsg = $derived(
    Array.isArray(content) &&
    (content as ContentBlock[]).every(b => b.type === 'tool_result')
  )
</script>

{#if !isToolResultMsg && (isUser ? !!userText : true)}
  {#if isUser}
    <div class="message user">
      <div class="bubble user-bubble">
        <p>{userText}</p>
      </div>
    </div>
  {:else}
    <!-- Assistant message: text and tool_use in order, tool_use as full ToolCards -->
    <div class="assistant-turn">
      <div class="avatar" aria-hidden="true">✦</div>
      <div class="turn-body">
        {#if blocks}
          {#each blocks as block}
            {#if block.type === 'text' && (block as any).text}
              <p class="text-block">{(block as any).text}</p>
            {:else if block.type === 'tool_use'}
              {@const b = block as any}
              {@const out = toolOutputs[b.id]}
              <ToolCard
                sessionId={message.sessionId}
                tool={{
                  toolUseId: b.id,
                  name: b.name,
                  input: b.input ?? {},
                  output: out?.content,
                  isError: out?.isError,
                  status: out ? (out.isError ? 'error' : 'done') : 'done',
                }}
              />
            {/if}
          {/each}
        {:else}
          <p class="text-block">{typeof content === 'string' ? content : ''}</p>
        {/if}
        {#if message.model}
          <div class="meta">{message.model}</div>
        {/if}
      </div>
    </div>
  {/if}
{/if}

<style>
  .message {
    display: flex;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-4);
    align-items: flex-start;
  }
  .message.user { flex-direction: row-reverse; }

  .bubble {
    max-width: 70%;
    overflow-wrap: anywhere;
    border-radius: var(--radius-lg);
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-md);
    line-height: 1.6;
  }
  .user-bubble {
    background: color-mix(in srgb, var(--color-accent-brand) 15%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-accent-brand) 30%, transparent);
    border-radius: var(--radius-lg) var(--radius-lg) var(--radius-sm) var(--radius-lg);
  }

  /* Assistant turn: avatar on left, body on right */
  .assistant-turn {
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
  .turn-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .text-block {
    margin: 0;
    max-width: 70%;
    align-self: flex-start;
    background: var(--color-bg-elevated);
    border-radius: var(--radius-sm) var(--radius-lg) var(--radius-lg) var(--radius-lg);
    padding: var(--spacing-2) var(--spacing-3);
    color: var(--color-text-primary);
    white-space: pre-wrap;
    font-size: var(--font-size-md);
    line-height: 1.6;
  }

  p { margin: 0; color: var(--color-text-primary); white-space: pre-wrap; overflow-wrap: anywhere; }

  .meta {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }
</style>
