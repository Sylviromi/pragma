<script lang="ts">
  // Design settings section header (v2): mono uppercase accent kicker,
  // 26px Barlow Condensed title, muted blurb (max 56ch), mono meta right.
  let {
    kicker,
    title,
    blurb = '',
    meta = '',
  }: {
    kicker: string;
    title: string;
    blurb?: string;
    meta?: string;
  } = $props();
</script>

<div class="header">
  <div>
    <div class="kicker">{kicker}</div>
    <h2>{title}</h2>
    {#if blurb}<p>{blurb}</p>{/if}
  </div>
  {#if meta}
    <span class="meta">{meta}</span>
  {/if}
</div>

<style>
  .header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 20px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--border-default);
    margin-bottom: 22px;
    /* as a flex item (e.g. Plugins page title-row) take the remaining width;
       block contexts ignore it */
    flex: 1;
    min-width: 0;
  }
  .header > div:first-child {
    flex: 1;
    min-width: 0;
  }
  .kicker {
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--accent);
    margin-bottom: 5px;
  }
  h2 {
    font-family: var(--font-heading);
    font-size: 26px;
    font-weight: 600;
    letter-spacing: 0.01em;
    color: var(--text-primary);
    margin: 0;
  }
  p {
    font-family: var(--font-body);
    font-size: 13px;
    color: var(--text-muted);
    margin: 5px 0 0;
    /* no width cap: blurbs use the available header width and only wrap at
       the window edge; \n in the blurb breaks onto its own line */
    text-wrap: pretty;
    white-space: pre-line;
  }
  .meta {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    white-space: nowrap;
    padding-bottom: 4px;
  }
</style>
