<script lang="ts">
  import { isToolResultTurn, parseContent, textBlocks } from '../lib/types'
  import type { ContentBlock, Message, ToolUseBlock, TextBlock } from '../lib/types'
  import { tidy } from '../lib/markdown'
  import TextBubble from './TextBubble.svelte'
  import ToolCard from './ToolCard.svelte'
  import QuestionBlock from './QuestionBlock.svelte'
  import ThoughtCard from './ThoughtCard.svelte'
  import AgentMark from './AgentMark.svelte'
  import { sessionTint } from '../lib/tint'

  let {
    message,
    toolOutputs = {},
    onContextMenu,
    highlighted = false,
    onAnswerQuestion,
    animate = false,
  }: {
    message: Message
    toolOutputs: Record<string, { content: string; isError: boolean }>
    onContextMenu?: (e: MouseEvent) => void
    highlighted?: boolean
    /** Answer callback for the question-block skeleton (option chips). */
    onAnswerQuestion?: (option: string) => void
    /** Entrance animation — design animates bubbles as they appear; history
     *  only animates the newest message so tab switches stay quiet. */
    animate?: boolean
  } = $props()

  // Trim trailing whitespace/newlines from text before display — the stored
  // text often ends with a stray newline, which pre-wrap would render as an
  // extra empty line inside the bubble.
  // Recover the picked option from an ask_question tool result (the loop
  // writes "User answered: <option>"; timeouts get a different message).
  function questionAnswer(out: string | undefined): string | null {
    if (!out) return null
    const m = /^User answered: (.+)$/s.exec(out.trim())
    return m ? m[1] : null
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
  <div class="msg-row" class:highlighted class:animate data-role={isUser ? 'user' : 'assistant'}>
  {#if isUser}
    <div class="user-row">
      <div class="user-bubble" oncontextmenu={onMenu} role="group" aria-label="User message">
        <p>{userText}</p>
      </div>
    </div>
  {:else}
    <!-- Assistant turn: agent mark + ordered blocks (thought → tool → text) -->
    <div class="assistant-row">
      <div class="avatar" aria-hidden="true"><AgentMark tint={sessionTint(message.sessionId)} /></div>
      <div class="turn-body" oncontextmenu={onMenu} role="group" aria-label="Assistant response">
        {#if blocks}
          {#each blocks as block}
            {#if block.type === 'text' && (block as TextBlock).text.trim()}
              {@const b = block as TextBlock}
              <TextBubble text={b.text} />
            {:else if block.type === 'reasoning' && tidy(block.content)}
              <ThoughtCard title="Thought process">{tidy(block.content)}</ThoughtCard>
            {:else if block.type === 'thinking' && tidy(block.thinking)}
              <ThoughtCard title="Thought process">{tidy(block.thinking)}</ThoughtCard>
            {:else if block.type === 'tool_use'}
              {@const b = block as ToolUseBlock}
              {@const out = toolOutputs[b.id]}
              {#if b.name === 'ask_question'}
                {@const qText = String((b.input as any)?.text ?? '')}
                {@const qOptions = (Array.isArray((b.input as any)?.options) ? (b.input as any).options : []) as string[]}
                {@const qAnswer = questionAnswer(out?.content)}
                <QuestionBlock text={qText} options={qOptions} answer={qAnswer} />
              {:else}
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
            {/if}
          {/each}
        {:else if typeof content === 'string' && content.trim()}
          <TextBubble text={content} />
        {/if}

        {#if message.model}
          <div class="meta">
            <span>{message.model}</span>
            {#if message.tokens > 0}<span>·</span><span>{message.tokens} tokens</span>{/if}
          </div>
        {/if}
      </div>
    </div>
  {/if}
  </div>
{/if}

<style>
  .msg-row {
    padding: var(--spacing-1) 0;
    cursor: context-menu;
    border-radius: var(--radius-md);
    transition: background-color 120ms ease;
  }
  .msg-row.highlighted {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    box-shadow: inset 2px 0 0 0 var(--accent);
  }
  /* Design bubble entrance — applied to the newest message only. */
  .msg-row.animate {
    animation: bubbleIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }

  /* User bubble — right aligned, accent-soft fill, asymmetric radius. */
  .user-row {
    display: flex;
    justify-content: flex-end;
  }
  .user-bubble {
    max-width: 64%;
    overflow-wrap: anywhere;
    background: var(--accent-soft);
    border: 1px solid var(--border-subtle);
    border-radius: 18px 18px 5px 18px;
    padding: 10px 16px;
    font-family: var(--font-body);
    font-size: 14px;
    color: var(--text-primary);
    line-height: 1.5;
  }
  .user-bubble p { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }

  /* Assistant turn: agent mark left, body right */
  .assistant-row {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }
  .avatar {
    width: 30px;
    height: 30px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-top: 2px;
  }
  .turn-body {
    flex: 1;
    min-width: 0;
    max-width: 78%;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  /* Meta: model · tokens · duration (design mono caption) */
  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-muted);
  }
</style>