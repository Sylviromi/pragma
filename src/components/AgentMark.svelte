<script lang="ts">
  // Design agent mark (`Pragma UI Kit v2.dc.html`): diamond node + dashed
  // orbit + square corners, stroked in the session's tint. Replaces the old
  // "P" circle for assistant turns + the live indicator.
  let {
    tint = 'var(--accent)',
    size = 26,
  }: {
    tint?: string
    size?: number
  } = $props()
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 32 32"
  fill="none"
  aria-label="Pragma agent"
  role="img"
  style="display:block;"
>
  <path d="M16 9.5 8.5 21.5M16 9.5 23.5 21.5" stroke={tint} stroke-width="1.2" opacity="0.55"></path>
  <path d="M8.5 21.5h15" stroke={tint} stroke-width="1.2" stroke-dasharray="2 2.4" opacity="0.4"></path>
  <rect x="13.4" y="6.9" width="5.2" height="5.2" fill={tint}></rect>
  <rect x="6.1" y="19" width="4.6" height="4.6" stroke={tint} stroke-width="1.4"></rect>
  <rect x="21.3" y="19" width="4.6" height="4.6" stroke={tint} stroke-width="1.4"></rect>
  <circle cx="16" cy="16" r="15.3" stroke={tint} stroke-width="1.1" stroke-dasharray="1.6 3.4" opacity="0.5"></circle>
</svg>
