<script lang="ts">
  import { fly } from 'svelte/transition'
  import { toasts, dismissToast } from '../stores/toast.svelte'
</script>

<div class="toast-stack" role="status" aria-live="polite">
  {#each toasts as t (t.id)}
    <button
      class="toast"
      class:error={t.kind === 'error'}
      class:success={t.kind === 'success'}
      transition:fly={{ y: 8, duration: 200, easing: (t) => 1 - Math.pow(1 - t, 3) }}
      onclick={() => dismissToast(t.id)}
    >
      <span class="dot" aria-hidden="true"></span>
      <span class="msg">{t.message}</span>
    </button>
  {/each}
</div>

<style>
  /* Design toast: bottom-right, elevated card, mono text, status dot,
     shadow-lg. Click to dismiss. */
  .toast-stack {
    position: fixed;
    right: 20px;
    bottom: 20px;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
    z-index: 1000;
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 420px;
    background: var(--bg-elevated);
    color: var(--text-primary);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    padding: 8px 12px;
    font-family: var(--font-mono);
    font-size: 11.5px;
    line-height: 1.4;
    text-align: left;
    cursor: pointer;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    flex: none;
  }
  .toast.success .dot {
    background: var(--success);
  }
  .toast.error .dot {
    background: var(--danger);
  }
  .toast:hover {
    border-color: var(--text-muted);
  }
</style>