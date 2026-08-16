<script lang="ts">
  import Modal from './Modal.svelte'
  import Button from './Button.svelte'
  import { getRevertInfo } from '../lib/tauri'
  import { revertToMessage } from '../stores/chat.svelte'
  import { pushToast } from '../stores/toast.svelte'
  import type { Message, RevertInfo } from '../lib/types'

  let { sessionId, message, onclose }: {
    sessionId: string
    message: Message
    onclose: () => void
  } = $props()

  let info = $state<RevertInfo | null>(null)
  let revertFiles = $state(true)
  let busy = $state(false)
  let err = $state<string | null>(null)

  $effect(() => {
    let alive = true
    getRevertInfo(sessionId, message.id)
      .then((r) => { if (alive) info = r })
      .catch((e) => { if (alive) err = String(e) })
    return () => { alive = false }
  })

  const when = $derived.by(() => {
    try {
      return new Date(message.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    } catch {
      return ''
    }
  })

  async function confirm() {
    if (busy) return
    busy = true
    err = null
    try {
      const summary = await revertToMessage(sessionId, message.id, revertFiles)
      pushToast(
        `Returned — removed ${summary.messagesRemoved} message${summary.messagesRemoved === 1 ? '' : 's'}` +
        (summary.filesReverted.length ? `, restored ${summary.filesReverted.length} file${summary.filesReverted.length === 1 ? '' : 's'}` : ''),
        'success'
      )
      onclose()
    } catch (e) {
      err = String(e)
    } finally {
      busy = false
    }
  }
</script>

<Modal onclose={onclose} label="Return to this point">
  {#snippet header()}
    <div class="head">
      <span class="badge" aria-hidden="true">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M3 7v6h6"/>
          <path d="M3 13a9 9 0 1 0 3-7.7L2.99 5"/>
        </svg>
      </span>
      <div>
        <h2>Return to this point</h2>
        <p class="sub">Roll the conversation back and start fresh from here.</p>
      </div>
    </div>
  {/snippet}

  <div class="preview-card">
    <div class="preview-meta">
      <span class="role-tag" class:user={message.role === 'user'}>{message.role === 'user' ? 'You' : 'Assistant'}</span>
      {#if when}<span class="time">{when}</span>{/if}
    </div>
    <p class="preview">{info?.preview ?? '…'}</p>
  </div>

  <ul class="effects">
    <li class="effect">
      <span class="e-ic" aria-hidden="true">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="3" width="18" height="18" rx="2"/>
          <path d="M9 9l6 6M15 9l-6 6"/>
        </svg>
      </span>
      <span>
        <strong>{info ? `${info.messagesRemoved} message${info.messagesRemoved === 1 ? '' : 's'}` : '…'}</strong> after this point will be removed.
      </span>
    </li>
    <li class="effect">
      <span class="e-ic" aria-hidden="true">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M3 3v18h18"/>
          <path d="M18 17V9M13 17V5M8 17v-3"/>
        </svg>
      </span>
      <span>
        {#if info && info.files.length > 0}
          <strong>{info.files.length} file{info.files.length === 1 ? '' : 's'}</strong> changed in that span — restore them too?
        {:else}
          No file changes in that span.
        {/if}
      </span>
    </li>
  </ul>

  <label class="option" class:off={!!info && info.files.length === 0}>
    <input type="checkbox" bind:checked={revertFiles} disabled={!!info && info.files.length === 0} />
    <span>Also restore files to how they were</span>
  </label>

  {#if info && info.files.length > 0}
    <div class="chips">
      {#each info.files.slice(0, 6) as f}
        <code class="chip">{f}</code>
      {/each}
      {#if info.files.length > 6}
        <code class="chip more">+{info.files.length - 6}</code>
      {/if}
    </div>
  {/if}

  {#if err}
    <p class="err" role="alert">{err}</p>
  {/if}

  {#snippet footer()}
    <Button variant="ghost" class="modal-btn" onclick={onclose} disabled={busy}>Cancel</Button>
    <Button variant="danger-solid" class="modal-btn" onclick={confirm} disabled={busy || !info}>
      {busy ? 'Rolling back…' : 'Roll back'}
    </Button>
  {/snippet}
</Modal>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    margin-bottom: var(--spacing-3);
  }
  .badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
    flex-shrink: 0;
  }
  h2 { margin: 0; font-size: var(--font-size-lg); font-weight: 600; }
  .sub { margin: 2px 0 0; font-size: var(--font-size-sm); color: var(--text-muted); }

  .preview-card {
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 10px;
    padding: var(--spacing-3);
    margin-bottom: var(--spacing-3);
  }
  .preview-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    margin-bottom: var(--spacing-2);
  }
  .role-tag {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-muted);
    background: color-mix(in srgb, var(--text-muted) 12%, transparent);
    border-radius: 5px;
    padding: 2px 7px;
  }
  .role-tag.user {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .time { font-size: var(--font-size-xs); color: var(--text-muted); }
  .preview {
    margin: 0;
    font-size: var(--font-size-sm);
    line-height: 1.5;
    color: var(--text-primary);
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    line-clamp: 3;
    overflow: hidden;
  }

  .effects {
    list-style: none;
    margin: 0 0 var(--spacing-3);
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }
  .effect {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    font-size: var(--font-size-sm);
    color: var(--text-primary);
    line-height: 1.4;
  }
  .e-ic {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--text-muted) 10%, transparent);
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .option {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    font-size: var(--font-size-sm);
    cursor: pointer;
    padding: var(--spacing-2) var(--spacing-3);
    background: color-mix(in srgb, var(--accent) 6%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 22%, transparent);
    border-radius: 9px;
    margin-bottom: var(--spacing-2);
  }
  .option.off { background: transparent; border-color: var(--border-subtle); opacity: 0.6; }
  .option input { accent-color: var(--accent); }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: var(--spacing-3);
  }
  .chip {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    padding: 2px 7px;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chip.more { color: var(--text-muted); }

  .err { color: var(--danger); font-size: var(--font-size-sm); margin: 0 0 var(--spacing-2); }
  /* Modal buttons keep the dialog's 8px-radius skin (spacing-5 was never
     defined in tokens, so the old padding was broken — standardize on 8×16). */
  :global(.btn.modal-btn) {
    border-radius: 8px;
    padding: 8px 16px;
    font-weight: 600;
  }
</style>
