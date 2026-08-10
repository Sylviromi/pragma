<script lang="ts">
  import { sendMessage, stopTurn, chat } from '../stores/chat.svelte'
  import ModelPicker from './ModelPicker.svelte'
  import Icon from './Icon.svelte'
  import DirChip from './DirChip.svelte'
  import SlotStack from './SlotStack.svelte'
  import { getSession } from '../stores/sessions.svelte'
  import { updateSession, captureAppScreenshot } from '../lib/tauri'
  import { pushToast } from '../stores/toast.svelte'
  import type { ThinkingPref } from '../lib/types'

  let { sessionId }: { sessionId: string } = $props()

  let text = $state('')
  let model = $state('')
  let provider = $state('')
  // User preference: how much thinking the model may do ('auto' = model default
  // level, 'off' = no thinking at all, explicit low/medium/high where the model
  // supports graduated levels). Lives in the chat store per session so it
  // survives HMR and is picked up by sendMessage.
  let thinking = $state<ThinkingPref>('auto') // hydrated from chat.thinking on first effect run
  let thinkingHydrated = $state(false)
  $effect(() => {
    if (!thinkingHydrated) {
      thinkingHydrated = true
      thinking = chat.thinking[sessionId] ?? 'auto'
    }
    chat.thinking[sessionId] = thinking
  })
  // Becomes true once the session's stored model/provider have been loaded,
  // so we never persist transient defaults (mount / HMR resets) over the real
  // selection.
  let hydrated = $state(false)
  // Which session id the local model/provider were hydrated from. Prevents the
  // hydration effect (below) from re-applying the stored values over the
  // user's fresh pick from the model picker.
  let hydratedFor: string | null = null
  // Auto-grow the textarea from 1 to ~6 lines; taller content scrolls.
  let ta = $state<HTMLTextAreaElement | null>(null)

  const session = $derived(getSession(sessionId))
  const running = $derived(chat.running[sessionId] ?? false)

  // Initialise model/provider from persisted session — but only once per
  // session (or after a reset such as HMR, when model is ''). Never clobber a
  // selection the user just made: the effect re-runs whenever model/provider
  // change, and without the hydratedFor guard it would immediately reset the
  // freshly-picked model back to the stored one.
  $effect(() => {
    const s = getSession(sessionId)
    if (!s) return
    if (hydratedFor === s.id && model !== '') return
    if (s.model && model !== s.model) model = s.model
    if (s.provider && provider !== s.provider) provider = s.provider
    hydratedFor = s.id
    hydrated = true
  })

  // Persist model/provider when they change — but only after hydration and
  // only when they actually differ from what's stored.
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
    if (ta) ta.style.height = 'auto'
    await sendMessage(
      sessionId,
      content,
      model,
      provider,
      session?.workingDir ?? '',
      chat.autoApprove[sessionId] ?? false,
      thinking
    )
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      submit()
    }
  }

  // Auto-grow the textarea (1 → ~6 lines ≈ 144px), scrolling beyond.
  function autoresize(el: HTMLTextAreaElement) {
    const grow = () => {
      el.style.height = 'auto'
      el.style.height = Math.min(el.scrollHeight, 144) + 'px'
    }
    grow()
    el.addEventListener('input', grow)
    return { destroy: () => el.removeEventListener('input', grow) }
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

  const autoApprove = $derived(chat.autoApprove[sessionId] ?? false)
  function toggleAuto() {
    chat.autoApprove[sessionId] = !autoApprove
  }
</script>

<div class="input-area">
  <textarea
    bind:this={ta}
    bind:value={text}
    onkeydown={onKeydown}
    placeholder="Message Pragma…"
    disabled={running}
    aria-label="Message input"
    aria-multiline="true"
    use:autoresize
  ></textarea>

  <div class="footer">
    <ModelPicker bind:model bind:provider bind:thinking />
    <DirChip {sessionId} />

    <!-- Plugins can contribute their own controls here (design's composer-center). -->
    <SlotStack slot="composer-center" context={sessionId} />

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
      <!-- Auto-approve (compact toggle; moved into the composer so the tab
           bar matches the design's icon-only actions). -->
      <button class="auto-dot" class:on={autoApprove} onclick={toggleAuto} title="Auto-approve all tool calls (active session)" aria-pressed={autoApprove}>
        <span class="track" aria-hidden="true"><span class="knob"></span></span>
      </button>
      <button
        class="send-btn"
        onclick={submit}
        disabled={running || !text.trim()}
        aria-label="Send message"
      >
        Send <Icon name="arrow_upward" size={15} />
      </button>
    </div>
  </div>
</div>

<style>
  /* Design composer: elevated strip, 14px 20px 16px padding, rounded textarea. */
  .input-area {
    border-top: 1px solid var(--border-default);
    background: var(--bg-elevated);
    padding: 14px 20px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex-shrink: 0;
    min-width: 0;
  }
  textarea {
    width: 100%;
    resize: none;
    overflow-y: auto;
    max-height: 144px;
    background: var(--bg-base);
    color: var(--text-primary);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    font-family: var(--font-body);
    font-size: 14px;
    line-height: 1.5;
    outline: none;
    min-height: 40px;
  }
  textarea:focus {
    border-color: var(--border-focus);
  }
  textarea::placeholder {
    color: var(--text-muted);
  }
  .footer {
    position: relative;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    row-gap: 8px;
    gap: 8px;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .footer::-webkit-scrollbar {
    display: none;
  }
  .divider {
    width: 1px;
    align-self: stretch;
    background: var(--border-default);
    margin: 0 2px;
    flex: none;
  }
  .footer-right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex-shrink: 0;
  }
  .hint-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    color: var(--text-secondary);
    font-size: 12.5px;
    padding: 4px 6px;
    border-radius: var(--radius-sm);
    white-space: nowrap;
    flex: none;
  }
  .hint-btn:hover {
    color: var(--text-primary);
    background: var(--bg-overlay);
  }
  .send-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--accent);
    color: var(--accent-on);
    border: none;
    border-radius: var(--radius-md);
    padding: 7px 14px;
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 13px;
    cursor: pointer;
    white-space: nowrap;
    flex: none;
  }
  .send-btn:hover {
    background: var(--accent-hover);
  }
  .send-btn:active {
    background: var(--accent-active);
  }
  .send-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .stop-btn {
    color: var(--danger);
    border: 1px solid color-mix(in srgb, var(--danger) 40%, transparent);
  }
  .stop-btn:hover {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
    color: var(--danger);
  }
  /* Compact auto-approve switch */
  .auto-dot {
    display: flex;
    align-items: center;
    background: transparent;
    border: none;
    padding: 2px;
    cursor: pointer;
    flex: none;
  }
  .track {
    position: relative;
    width: 26px;
    height: 15px;
    border-radius: 2px;
    background: var(--border-default);
    transition: background 0.12s;
    flex-shrink: 0;
  }
  .auto-dot.on .track {
    background: var(--accent);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 11px;
    height: 11px;
    border-radius: 1px;
    background: var(--bg-elevated);
    transition: left 0.12s;
  }
  .auto-dot.on .knob {
    left: calc(100% - 13px);
    background: var(--accent-on);
  }

  @media (max-width: 940px) {
    .input-area {
      padding: 12px 16px 14px;
    }
    .hide-narrow {
      display: none;
    }
  }
</style>