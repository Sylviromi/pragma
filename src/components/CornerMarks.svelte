<script lang="ts">
  // Blueprint corner marks — the "+" at each corner of a card/panel. Instead of
  // drawing plus glyphs, each mark is a pair of 1px lines that CONTINUE the
  // box's border outward (a horizontal segment along the border line + a
  // vertical one), so they always match the border exactly, share its color
  // var, and inherit any opacity/fade of the parent.
  //
  // Usage: place inside a `position: relative` box (it overlays inset:0 and
  // is pointer-events:none). Pass `color` when the box border changes state
  // (e.g. a question block: accent while pending, border-default answered).
  // Parents can LIGHT the marks up on hover by setting `--cm-color` on
  // themselves (e.g. `.card:hover { --cm-color: var(--accent) }`) — the marks
  // prefer the inherited `--cm-color` over the `color` prop.
  let {
    /** Border-line color the corner lines continue (fallback). */
    color = 'var(--border-default)',
    /** How far each line extends past the corner (px). */
    length = 6,
    /** Border thickness the lines align with (px). */
    thickness = 1,
  }: {
    color?: string
    length?: number
    thickness?: number
  } = $props()
</script>

<div
  class="corner-marks"
  style={`--cm-color-prop:${color}; --cm-len:${length}px; --cm-b:${thickness}px`}
  aria-hidden="true"
>
  <span class="cm tl"></span>
  <span class="cm tr"></span>
  <span class="cm bl"></span>
  <span class="cm br"></span>
</div>

<style>
  .corner-marks {
    position: absolute;
    inset: 0;
    pointer-events: none;
    overflow: visible;
  }
  .cm {
    position: absolute;
    width: 0;
    height: 0;
  }
  /* Each mark = horizontal line (::before) + vertical line (::after) that
     meet exactly at the corner and extend outward along the border lines. */
  .cm::before,
  .cm::after {
    content: '';
    position: absolute;
    /* inherited --cm-color (set by a parent on hover to light up) wins;
       otherwise the `color` prop (--cm-color-prop) is the fallback.
       Transition matches the parents' border-color transitions (0.2s ease)
       so hover highlight fades in with the border instead of snapping. */
    background: var(--cm-color, var(--cm-color-prop, var(--border-default)));
    transition: background-color 0.2s ease;
  }
  .cm::before {
    width: var(--cm-len);
    height: var(--cm-b);
  }
  .cm::after {
    width: var(--cm-b);
    height: var(--cm-len);
  }
  /* Anchored on the border line (offset by the thickness), extending out. */
  .tl {
    top: calc(var(--cm-b) * -1);
    left: calc(var(--cm-b) * -1);
  }
  .tl::before {
    right: 0;
    top: 0;
  }
  .tl::after {
    bottom: 0;
    left: 0;
  }
  .tr {
    top: calc(var(--cm-b) * -1);
    right: calc(var(--cm-b) * -1);
  }
  .tr::before {
    left: 0;
    top: 0;
  }
  .tr::after {
    bottom: 0;
    right: 0;
  }
  .bl {
    bottom: calc(var(--cm-b) * -1);
    left: calc(var(--cm-b) * -1);
  }
  .bl::before {
    right: 0;
    bottom: 0;
  }
  .bl::after {
    top: 0;
    left: 0;
  }
  .br {
    bottom: calc(var(--cm-b) * -1);
    right: calc(var(--cm-b) * -1);
  }
  .br::before {
    left: 0;
    bottom: 0;
  }
  .br::after {
    top: 0;
    right: 0;
  }
</style>
