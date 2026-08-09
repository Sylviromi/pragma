<script lang="ts">
  import { isToolResultTurn, parseContent, textBlocks } from '../lib/types'
  import type { ContentBlock, Message, ToolUseBlock } from '../lib/types'
  import ToolCard from './ToolCard.svelte'
  import Icon from './Icon.svelte'

  let {
    message,
    toolOutputs = {},
    onContextMenu,
    highlighted = false,
  }: {
    message: Message
    toolOutputs: Record<string, { content: string; isError: boolean }>
    onContextMenu?: (e: MouseEvent) => void
    highlighted?: boolean
  } = $props()

  // Trim trailing whitespace/newlines from text before display — the stored
  // text often ends with a stray newline, which pre-wrap would render as an
  // extra empty line inside the bubble.
  function tidy(s: string | null | undefined): string {
    return (s ?? '').replace(/\s+$/, '')
  }

  const isUser = $derived(message.role === 'user')
  const content = $derived(parseContent(message.content))
  const blocks = $derived(Array.isArray(content) ? content as ContentBlock[] : null)

  const userText = $derived(
    typeof content === 'string'
      ? tidy(content)
      : tidy(textBlocks(content).map((b) => b.text).join(''))
  )

  const isToolResultMsg = $derived(isToolResultTurn(content))

  function onMenu(e: MouseEvent) {
    e.preventDefault()
    onContextMenu?.(e)
  }
</script>

{#if !isToolResultMsg && (isUser ? !!userText : true)}
  <div class="msg-row" class:highlighted data-role={isUser ? 'user' : 'assistant'}>
  {#if isUser}
    <div class="message user">
      <div class="bubble user-bubble" oncontextmenu={onMenu}>
        <p>{userText}</p>
      </div>
    </div>
  {:else}
    <!-- Assistant message: text and tool_use in order, tool_use as full ToolCards -->
    <div class="assistant-turn">
      <div class="avatar" aria-hidden="true"><Icon name="sparkle" size={13} /></div>
      <div class="turn-body" oncontextmenu={onMenu}>
        {#if blocks}
          {#each blocks as block}
            {#if block.type === 'text' && tidy(block.text)}
              <p class="text-block">{tidy(block.text)}</p>
            {:else if block.type === 'reasoning' && tidy(block.content)}
              <div class="reasoning-bubble">{tidy(block.content)}</div>
            {:else if block.type === 'thinking' && tidy(block.thinking)}
              <div class="reasoning-bubble">{tidy(block.thinking)}</div>
            {:else if block.type === 'tool_use'}
              {@const b = block as ToolUseBlock}
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
  </div>
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
    width: fit-content;
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

  /* Stored model reasoning / thinking block — collapsed, subtle */
  /* Stored model reasoning — a normal bubble but tinted with the brand accent
     so the internal monologue stands apart from the final answer. */
  .reasoning-bubble {
    margin: 2px 0 6px;
    width: fit-content;
    max-width: 70%;
    align-self: flex-start;
    border-radius: var(--radius-lg);
    padding: var(--spacing-2) var(--spacing-3);
    background: color-mix(in srgb, var(--color-accent-brand) 10%, var(--color-bg-overlay));
    border: 1px solid color-mix(in srgb, var(--color-accent-brand) 24%, transparent);
    color: var(--color-text-primary);
    font-style: italic;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-size: var(--font-size-sm);
    line-height: 1.6;
    max-height: 40vh;
    overflow-y: auto;
  }

  p { margin: 0; color: var(--color-text-primary); white-space: pre-wrap; overflow-wrap: anywhere; }

  .meta {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .msg-row {
    padding: var(--spacing-1) 0;
    cursor: context-menu;
    border-radius: var(--radius-md);
    transition: background-color 120ms ease;
  }
  .msg-row.highlighted {
    background: color-mix(in srgb, var(--color-accent-brand) 8%, transparent);
    box-shadow: inset 2px 0 0 0 var(--color-accent-brand);
  }
</style>
