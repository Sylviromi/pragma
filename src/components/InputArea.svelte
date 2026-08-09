<script lang="ts">
  import { sendMessage, chat } from '../stores/chat.svelte'
  import { stopTurn } from '../stores/chat.svelte'
  import ModelPicker from './ModelPicker.svelte'
  import Icon from './Icon.svelte'
  import DirChip from './DirChip.svelte'
  import SlotStack from './SlotStack.svelte'
  import { getSession } from '../stores/sessions.svelte'
  import { updateSession, captureAppScreenshot } from '../lib/tauri'
  import { pushToast } from '../stores/toast.svelte'

  let { sessionId }: { sessionId: string } = $props()

  let text = $state('')
  let model = $state('')
  let provider = $state('')
  // Becomes true once the session's stored model/provider have been loaded,
  // so we never persist transient defaults (mount / HMR resets) over the real
  // selection.
  let hydrated = $state(false)

  const session = $derived(getSession(sessionId))
  const running = $derived(chat.running[sessionId] ?? false)

  // Initialise model/provider from persisted session once it's available
  $effect(() => {
    const s = getSession(sessionId)
    if (!s) return
    if (s.model && model !== s.model) model = s.model
    if (s.provider && provider !== s.provider) provider = s.provider
    hydrated = true
  })

  // Persist model/provider when they change — but only after hydration and only
  // when they actually differ from what's stored. This prevents the mount-time
  // default selection (or an HMR reset) from clobbering a session's plugin
  // provider, model, or working directory.
  $effect(() => {
    const s = getSession(sessionId)
    if (!hydrated || !s) return
    if (model === s.model && provider === s.provider) return
    updateSession(sessionId, model, provider, s.workingDir ?? '').catch(() => {})
  })

  async function submit() {
    const content = text.trim()
    if (!content || running) return
    text = ''
    await sendMessage(
      sessionId,
      content,
      model,
      provider,
      session?.workingDir ?? '',
      chat.autoApprove[sessionId] ?? false
    )
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      submit()
    }
  }

  // Esc interrupts the running turn wherever the focus is.
  $effect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && (chat.running[sessionId] ?? false)) {
        e.preventDefault()
        stopTurn(sessionId)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  async function shoot() {
    try {
      const path = await captureAppScreenshot()
      pushToast(`Screenshot → ${path}`, 'success')
    } catch (err) {
      pushToast(String(err), 'error')
    }
  }

  $effect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 's') {
        e.preventDefault()
        shoot()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
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
    <DirChip {sessionId} />
    <div class="divider" aria-hidden="true"></div>
    <SlotStack slot="input-toolbar" />
    <SlotStack slot="chat-toolbar-extra" context={sessionId} />
    <div class="footer-right">
      {#if running}
        <button
          class="hint-btn stop-btn"
          type="button"
          title="Stop the response (Esc)"
          onclick={() => stopTurn(sessionId)}
          aria-label="Stop generating"
        ><Icon name="stop" size={15} /> Stop</button>
      {/if}
      <button class="hint-btn" type="button" title="Screenshot this app window (Ctrl+Shift+S)" onclick={shoot}><Icon name="camera" size={16} /></button>
      <button class="hint-btn hide-narrow" type="button">@ skills</button>
      <button class="hint-btn hide-narrow" type="button">/ cmds</button>
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
    /* Never collapse below its content: the composer (textarea + Send) must
       stay visible even when the window is very short. */
    flex-shrink: 0;
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
    flex-wrap: wrap;
    row-gap: var(--spacing-1);
    gap: var(--spacing-2);
  }
  .divider {
    width: 1px;
    height: 18px;
    background: var(--color-border-default);
  }
  .footer-right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }
  .hint-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
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
  .stop-btn {
    color: var(--color-accent-error, #f85149);
    border: 1px solid color-mix(in srgb, var(--color-accent-error, #f85149) 40%, transparent);
  }
  .stop-btn:hover {
    background: color-mix(in srgb, var(--color-accent-error, #f85149) 10%, transparent);
  }

  /* ── Narrow windows ─────────────────────────────────────────────────── *
   * Keep the composer functional when width is tight: drop dead placeholder
   * buttons and compress spacing so the footer fits without wrapping off-
   * screen. */
  @media (max-width: 640px) {
    .input-area {
      padding: var(--spacing-2) var(--spacing-3);
      gap: var(--spacing-1);
    }
    .footer {
      gap: var(--spacing-1);
      row-gap: var(--spacing-1);
    }
    .hide-narrow {
      display: none;
    }
  }
  @media (max-width: 480px) {
    textarea {
      min-height: 36px;
    }
    .divider {
      display: none;
    }
    .hint-btn {
      padding: var(--spacing-1);
    }
    .footer-right {
      gap: var(--spacing-1);
    }
  }
</style>