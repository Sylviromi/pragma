<script lang="ts">
  import { toasts, dismissToast } from '../stores/toast.svelte'
</script>

<div class="toast-stack" role="status" aria-live="polite">
  {#each toasts as t (t.id)}
    <button
      class="toast"
      class:error={t.kind === 'error'}
      class:success={t.kind === 'success'}
      onclick={() => dismissToast(t.id)}
    >
      {t.message}
    </button>
  {/each}
</div>

<style>
  .toast-stack {
    position: fixed;
    right: var(--spacing-4);
    bottom: var(--spacing-4);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    z-index: 1000;
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    background: var(--color-bg-elevated);
    color: var(--color-text-primary);
    border: 1px solid var(--color-border-default);
    border-left: 3px solid var(--color-accent-brand);
    border-radius: var(--radius-md);
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-sm);
    font-family: var(--font-sans);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
    animation: toast-in 0.18s ease-out;
  }
  .toast.success {
    border-left-color: var(--color-success);
  }
  .toast.error {
    border-left-color: var(--color-danger);
  }
  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>