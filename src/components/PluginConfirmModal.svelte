<script lang="ts">
  import { listen } from '@tauri-apps/api/event'
  import { respondPluginConfirmation } from '../lib/tauri'
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

  $effect(() => {
    if (!request) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') respond(false)
      if (e.key === 'Enter') respond(true)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })
</script>

{#if request}
  <div class="overlay" role="presentation" onclick={() => respond(false)}>
    <div class="modal" role="dialog" aria-modal="true" tabindex="-1" aria-label={request.title} onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <header class="head">
        <span class="badge" aria-hidden="true">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" y1="8" x2="12" y2="12"/>
            <line x1="12" y1="16" x2="12.01" y2="16"/>
          </svg>
        </span>
        <div>
          <h2>{request.title}</h2>
          <p class="sub">Plugin: {request.pluginId}</p>
        </div>
      </header>

      <p class="message">{request.message}</p>

      <footer class="actions">
        <button class="ghost" type="button" onclick={() => respond(false)}>Dismiss</button>
        <button class="primary" type="button" onclick={() => respond(true)}>Accept</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(3px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    animation: fade-in 140ms ease-out;
  }
  @keyframes fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .modal {
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border-default);
    border-radius: 14px;
    padding: var(--spacing-4);
    width: min(420px, calc(100vw - 48px));
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    animation: modal-in 160ms ease-out;
  }
  @keyframes modal-in {
    from { opacity: 0; transform: translateY(8px) scale(0.97); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }

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

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-2);
    margin-top: var(--spacing-3);
  }
  .ghost {
    background: transparent;
    border: 1px solid var(--border-default);
    color: var(--text-primary);
    border-radius: 8px;
    padding: var(--spacing-2) var(--spacing-4);
    font-size: var(--font-size-sm);
    cursor: pointer;
  }
  .ghost:hover { background: var(--bg-overlay); }
  .primary {
    background: var(--accent);
    color: var(--accent-on);
    border: none;
    border-radius: 8px;
    padding: var(--spacing-2) var(--spacing-5);
    font-size: var(--font-size-sm);
    font-weight: 600;
    cursor: pointer;
  }
  .primary:hover { background: var(--accent-hover); }
  button:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
