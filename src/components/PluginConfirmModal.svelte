<script lang="ts">
  import { listen } from '@tauri-apps/api/event'
  import { respondPluginConfirmation } from '../lib/tauri'
  import Modal from './Modal.svelte'
  import Button from './Button.svelte'
  import { onMount } from 'svelte'

  interface ConfirmRequest {
    requestId: string
    pluginId: string
    title: string
    message: string
  }

  let request = $state<ConfirmRequest | null>(null)

  onMount(() => {
    const unlisten = listen<ConfirmRequest>('plugin:confirm-request', (e) => {
      request = e.payload
    })
    return () => { unlisten.then((f) => f()) }
  })

  function respond(accepted: boolean) {
    if (!request) return
    respondPluginConfirmation(request.requestId, accepted)
    request = null
  }

  // Enter accepts (the shared Modal already handles ESC → dismiss).
  $effect(() => {
    if (!request) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Enter') respond(true)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
</script>

{#if request}
  {@const req = request}
  <Modal onclose={() => respond(false)} label={req.title}>
    {#snippet header()}
      <div class="head">
        <span class="badge" aria-hidden="true">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" y1="8" x2="12" y2="12"/>
            <line x1="12" y1="16" x2="12.01" y2="16"/>
          </svg>
        </span>
        <div>
          <h2>{req.title}</h2>
          <p class="sub">Plugin: {req.pluginId}</p>
        </div>
      </div>
    {/snippet}

    <p class="message">{req.message}</p>

    {#snippet footer()}
      <Button variant="ghost" class="modal-btn" onclick={() => respond(false)}>Dismiss</Button>
      <Button variant="primary" class="modal-btn" onclick={() => respond(true)}>Accept</Button>
    {/snippet}
  </Modal>
{/if}

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
    background: color-mix(in srgb, var(--warning) 14%, transparent);
    color: var(--warning);
    flex-shrink: 0;
  }
  h2 { margin: 0; font-size: var(--font-size-lg); font-weight: 600; }
  .sub { margin: 2px 0 0; font-size: var(--font-size-sm); color: var(--text-muted); }

  .message {
    font-size: var(--font-size-sm);
    color: var(--text-primary);
    line-height: 1.5;
    margin: 0 0 var(--spacing-3);
  }
  /* Modal buttons keep the dialog's 8px-radius skin. */
  :global(.btn.modal-btn) {
    border-radius: 8px;
    padding: 8px 16px;
    font-weight: 600;
  }
</style>
