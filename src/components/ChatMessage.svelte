<script lang="ts">
  import { isToolResultTurn, parseContent, textBlocks } from '../lib/types'
  import type { ContentBlock, Message, ToolUseBlock, TextBlock } from '../lib/types'
  import { renderMarkdown } from '../lib/markdown'
  import ToolCard from './ToolCard.svelte'
  import QuestionBlock from './QuestionBlock.svelte'
  import ThoughtCard from './ThoughtCard.svelte'
  import Icon from './Icon.svelte'

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
  function tidy(s: string | null | undefined): string {
    return (s ?? '').replace(/\s+$/, '')
  }

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

  // Split a text block into prose + fenced code segments so code renders as
  // the design's dark inline code box (with copy), keeping prose in the bubble.
  interface Segment { kind: 'prose' | 'code'; text: string }
  function segmentsOf(text: string): Segment[] {
    const re = /```(\w*)\n?([\s\S]*?)```/g
    const out: Segment[] = []
    let last = 0
    let m: RegExpExecArray | null
    while ((m = re.exec(text)) !== null) {
      if (m.index > last) out.push({ kind: 'prose', text: text.slice(last, m.index) })
      out.push({ kind: 'code', text: m[2] })
      last = m.index + m[0].length
    }
    if (last < text.length) out.push({ kind: 'prose', text: text.slice(last) })
    if (out.length === 0 && text.trim()) out.push({ kind: 'prose', text })
    return out
  }

  function onMenu(e: MouseEvent) {
    e.preventDefault()
    onContextMenu?.(e)
  }

  async function copyCode(code: string) {
    try {
      await navigator.clipboard.writeText(code)
    } catch { /* clipboard unavailable */ }
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
    <!-- Assistant turn: avatar + ordered blocks (thought → tool → text) -->
    <div class="assistant-row">
      <div class="avatar" aria-hidden="true">P</div>
      <div class="turn-body" oncontextmenu={onMenu} role="group" aria-label="Assistant response">
        {#if blocks}
          {#each blocks as block}
            {#if block.type === 'text' && (block as TextBlock).text.trim()}
              {@const b = block as TextBlock}
              {@const segs = segmentsOf(tidy(b.text))}
              {#if segs.length > 0}
                <div class="text-bubble">
                  {#each segs as seg (seg.kind + seg.text.slice(0, 40))}
                    {#if seg.kind === 'prose' && seg.text.trim()}
                      <div class="md-prose">{@html renderMarkdown(seg.text)}</div>
                    {:else if seg.kind === 'code' && seg.text.trim()}
                      <div class="code-block">
                        <button
                          class="copy"
                          aria-label="Copy code"
                          title="Copy code"
                          onclick={() => copyCode(seg.text)}
                        ><Icon name="content_copy" size={14} /></button>
                        <pre>{seg.text.trimEnd()}</pre>
                      </div>
                    {/if}
                  {/each}
                </div>
              {/if}
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
          <div class="text-bubble"><p>{tidy(content)}</p></div>
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

  /* Assistant turn: avatar left, body right */
  .assistant-row {
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
  .turn-body {
    flex: 1;
    min-width: 0;
    max-width: 78%;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  /* Text block: elevated bubble, asymmetric radius, prose + inline code. */
  .text-bubble {
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 5px 18px 18px 18px;
    padding: 11px 16px;
    font-family: var(--font-body);
    font-size: 14px;
    color: var(--text-primary);
    line-height: 1.55;
  }
  .text-bubble p {
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  /* Markdown prose inside the bubble (rendered model output). */
  .md-prose :global(p) {
    margin: 0 0 0.6em;
    overflow-wrap: anywhere;
  }
  .md-prose :global(p:last-child) {
    margin-bottom: 0;
  }
  .md-prose :global(pre) {
    margin: 0.7em 0;
  }
  .md-prose :global(code) {
    font-family: var(--font-mono);
    font-size: 0.88em;
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    padding: 0.15em 0.4em;
  }
  .md-prose :global(pre code) {
    display: block;
    padding: 10px 12px;
    overflow-x: auto;
    border: none;
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    font-size: 0.85em;
    color: var(--text-secondary);
    line-height: 1.5;
  }
  .md-prose :global(h1), .md-prose :global(h2), .md-prose :global(h3),
  .md-prose :global(h4) {
    font-family: var(--font-heading);
    color: var(--text-primary);
    line-height: 1.25;
    margin: 0.9em 0 0.4em;
    font-weight: 600;
  }
  .md-prose :global(h1) { font-size: 1.25em; }
  .md-prose :global(h2) { font-size: 1.15em; }
  .md-prose :global(h3) { font-size: 1.05em; }
  .md-prose :global(h4) { font-size: 1em; }
  .md-prose :global(h1:first-child), .md-prose :global(h2:first-child),
  .md-prose :global(h3:first-child), .md-prose :global(h4:first-child) {
    margin-top: 0;
  }
  .md-prose :global(ul), .md-prose :global(ol) {
    margin: 0.5em 0;
    padding-left: 1.4em;
  }
  .md-prose :global(li) {
    margin: 0.22em 0;
  }
  .md-prose :global(ol) {
    list-style: decimal;
  }
  .md-prose :global(ul) {
    list-style: disc;
  }
  .md-prose :global(li > ul), .md-prose :global(li > ol) {
    margin: 0.15em 0;
  }
  .md-prose :global(blockquote) {
    margin: 0.6em 0;
    padding: 0.1em 0.9em;
    border-left: 3px solid var(--border-focus);
    color: var(--text-secondary);
    background: var(--bg-overlay);
    border-radius: var(--radius-sm);
  }
  .md-prose :global(blockquote p) {
    margin: 0.35em 0;
  }
  .md-prose :global(a) {
    color: var(--accent);
    text-decoration: none;
    border-bottom: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
  }
  .md-prose :global(a:hover) {
    border-bottom-color: var(--accent);
  }
  .md-prose :global(hr) {
    border: none;
    border-top: 1px solid var(--border-default);
    margin: 0.9em 0;
  }
  .md-prose :global(strong) { font-weight: 600; }
  .md-prose :global(em) { font-style: italic; }
  .md-prose :global(table) {
    border-collapse: collapse;
    margin: 0.6em 0;
    font-size: 0.93em;
  }
  .md-prose :global(th), .md-prose :global(td) {
    border: 1px solid var(--border-default);
    padding: 4px 9px;
    text-align: left;
  }
  .md-prose :global(th) {
    background: var(--bg-overlay);
    font-weight: 600;
  }
  .code-block {
    position: relative;
    margin-top: 10px;
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    padding: 12px 14px;
    font-family: var(--font-mono);
    font-size: 12.5px;
    color: var(--text-secondary);
  }
  .code-block pre {
    margin: 0;
    white-space: pre;
    overflow-x: auto;
  }
  .copy {
    position: absolute;
    top: 6px;
    right: 6px;
    background: var(--bg-overlay);
    border: 1px solid var(--border-default);
    border-radius: 2px;
    color: var(--text-muted);
    cursor: pointer;
    padding: 3px;
    display: flex;
  }
  .copy:hover {
    color: var(--text-primary);
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