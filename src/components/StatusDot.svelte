<script lang="ts">
  // Status dot — small colored circle with an optional live pulse (audit D2).
  // Previously duplicated as .s-dot.live / .active-dot.live / .node-dot.pulse /
  // .p-dot.blink / .status-dot.
  let {
    color = 'var(--text-muted)',
    live = false,
    size = 6,
    class: klass = '',
  }: {
    color?: string;
    /** Pulse with cursorblink while live/running. */
    live?: boolean;
    size?: number;
    class?: string;
  } = $props();
</script>

<span
  class={`sdot${klass ? ' ' + klass : ''}`}
  class:live
  style={`background:${color};width:${size}px;height:${size}px`}
  aria-hidden="true"
></span>

<style>
  .sdot {
    border-radius: 50%;
    flex: none;
    display: inline-block;
  }
  .sdot.live {
    animation: cursorblink 1.2s ease-in-out infinite;
  }
</style>
