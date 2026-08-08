<script lang="ts">
  import { sendMessage, chat } from '../stores/chat.svelte'
  import ModelPicker from './ModelPicker.svelte'
  import { getSession } from '../stores/sessions.svelte'

  let { sessionId }: { sessionId: string } = $props()

  let text = $state('')
  let model = $state('claude-sonnet-4-6')
  let provider = $state('anthropic')
  let autoApprove = $state(false)

  const session = $derived(getSession(sessionId))
  const running = $derived(chat.running[sessionId] ?? false)

  async function submit() {
    const content = text.trim()
    if (!content || running) return
    text = ''
    await sendMessage(sessionId, content, model, provider, session?.workingDir ?? '', autoApprove)
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      submit()
    }
  }
</script>

<div class="input-area">
  <textarea
    bind:value={text}
    onkeydown={onKeydown}
    placeholder="Ask anything, or describe what to build…"
    disabled={running}
    aria-label="Message input"
    aria-multiline="true"
  ></textarea>
  <div class="footer">
    <ModelPicker bind:model bind:provider />
    <div class="divider" aria-hidden="true"></div>
    <label class="auto-approve" title="Auto-approve all tool calls without confirmation">
      <input type="checkbox" bind:checked={autoApprove} aria-label="Auto-approve tools" />
      <span>Auto</span>
    </label>
    <div class="footer-right">
      <button class="hint-btn" type="button">@ skills</button>
      <button class="hint-btn" type="button">/ cmds</button>
      <button
        class="send-btn"
        onclick={submit}
        disabled={running || !text.trim()}
        aria-label="Send message"
      >
        {running ? '…' : 'Send'}
      </button>
    </div>
  </div>
</div>

<style>
  .input-area {
    border-top: 1px solid var(--color-border-subtle);
    padding: var(--spacing-3) var(--spacing-4);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    background: var(--color-bg-base);
  }
  textarea {
    width: 100%;
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-md);
    padding: var(--spacing-2) var(--spacing-3);
    color: var(--color-text-primary);
    font-family: var(--font-sans);
    font-size: var(--font-size-md);
    resize: none;
    min-height: 44px;
    max-height: 140px;
    overflow-y: auto;
    line-height: 1.5;
  }
  textarea:focus {
    outline: 2px solid var(--color-border-focus);
    outline-offset: -1px;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }
  .divider {
    width: 1px;
    height: 18px;
    background: var(--color-border-default);
  }
  .auto-approve {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .footer-right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }
  .hint-btn {
    background: transparent;
    border: none;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    padding: var(--spacing-1) var(--spacing-2);
    border-radius: var(--radius-sm);
  }
  .hint-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-overlay);
  }
  .send-btn {
    background: var(--color-accent-brand);
    color: white;
    border: none;
    border-radius: var(--radius-md);
    padding: var(--spacing-1) var(--spacing-4);
    font-size: var(--font-size-md);
    font-weight: 500;
  }
  .send-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>