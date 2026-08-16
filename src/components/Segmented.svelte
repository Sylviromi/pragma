<script lang="ts">
  // Segmented control — active-state option buttons (audit C5). Previously
  // duplicated as appearance's `.segmented` and the plugins filter pills.
  let {
    options,
    value,
    onchange,
    ariaLabel,
    mono = false,
    class: klass = '',
  }: {
    options: { value: string; label: string }[];
    value: string;
    onchange?: (v: string) => void;
    ariaLabel?: string;
    mono?: boolean;
    class?: string;
  } = $props();
</script>

<div
  class={`segmented${mono ? ' mono' : ''}${klass ? ' ' + klass : ''}`}
  role="group"
  aria-label={ariaLabel}
>
  {#each options as o (o.value)}
    <button
      class="seg"
      class:active={value === o.value}
      onclick={() => onchange?.(o.value)}
      aria-pressed={value === o.value}>{o.label}</button
    >
  {/each}
</div>

<style>
  .segmented {
    display: flex;
    gap: 2px;
    border: 1px solid var(--border-default);
    border-radius: 4px;
    overflow: hidden;
    width: fit-content;
    flex: none;
  }
  .seg {
    padding: 6px 12px;
    font-size: 11.5px;
    font-family: var(--font-body);
    border: none;
    cursor: pointer;
    background: transparent;
    color: var(--text-secondary);
    transition:
      background 0.18s ease,
      color 0.18s ease;
  }
  .seg:hover {
    color: var(--text-primary);
  }
  .seg.active {
    background: var(--accent);
    color: var(--accent-on);
  }

  /* Mono uppercase variant (plugins filter pills). */
  .segmented.mono {
    gap: 0;
    border-radius: 0;
  }
  .segmented.mono .seg {
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.04em;
    padding: 7px 12px;
  }
  .segmented.mono .seg + .seg {
    border-left: 1px solid var(--border-default);
  }
</style>
