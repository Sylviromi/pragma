<script lang="ts">
  // Shared modal shell (audit A4) — previously ReturnModal, PluginConfirmModal
  // and UpdateDialog each hand-rolled the same blur backdrop + centered card +
  // ESC/outside-click close. This owns the shell; header/body/footer are
  // snippets so each dialog keeps its own content layout.
  import type { Snippet } from 'svelte';

  let {
    onclose,
    label,
    closeOnBackdrop = true,
    closeOnEsc = true,
    padded = true,
    zIndex = 1000,
    footClass = '',
    header,
    children,
    footer,
    class: klass = '',
  }: {
    /** Close callback — ESC, backdrop click and any explicit close button. */
    onclose: () => void;
    /** Dialog aria-label. */
    label: string;
    closeOnBackdrop?: boolean;
    closeOnEsc?: boolean;
    /** 16px card padding (confirm dialogs). Set false for full-bleed cards
     *  with their own section padding/borders (UpdateDialog). */
    padded?: boolean;
    /** Overlay z-index (update dialog floats above everything). */
    zIndex?: number;
    /** Extra class on the footer row (callers style their own actions). */
    footClass?: string;
    header?: Snippet;
    children?: Snippet;
    footer?: Snippet;
    class?: string;
  } = $props();

  // Outside click closes the dialog (the card's own click handler stops
  // propagation so clicks inside never reach the overlay listener).
  function onDocClick(e: MouseEvent) {
    if (closeOnBackdrop && !(e.target as Element)?.closest?.('.modal')) onclose();
  }

  $effect(() => {
    if (!closeOnEsc) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onclose();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });
</script>

<div class="overlay" role="presentation" onclick={onDocClick} style={`z-index:${zIndex}`}>
  <div
    class:padded
    class={`modal${klass ? ' ' + klass : ''}`}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    aria-label={label}
    onclick={(e) => e.stopPropagation()}
    onkeydown={() => {}}
  >
    {#if header}{@render header?.()}{/if}
    {#if children}{@render children?.()}{/if}
    {#if footer}<div class={`modal-foot${footClass ? ' ' + footClass : ''}`}>
        {@render footer?.()}
      </div>{/if}
  </div>
</div>

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
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .modal {
    position: relative;
    display: flex;
    flex-direction: column;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: 14px;
    width: min(480px, calc(100vw - 48px));
    max-height: 82vh;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    animation: modal-in 160ms ease-out;
    overflow: hidden;
  }
  /* Padded dialogs (confirm style): 16px padding, whole card scrolls. */
  .modal.padded {
    padding: var(--spacing-4);
    overflow-y: auto;
  }
  @keyframes modal-in {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.97);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  .modal-foot {
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-2);
    margin-top: var(--spacing-3);
  }
  .modal:not(.padded) .modal-foot {
    margin-top: 0;
  }
</style>
