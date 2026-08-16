<script lang="ts">
  // Shared mono output/log pane (audit D4) — the per-line colored console
  // used by ToolCard (tool output), ProcessCard (process log) and the
  // SidebarPanel agents console. Owns the char/line truncation + the
  // "… truncated N more lines" footer ToolCard used to hand-roll, so the
  // three panes can never drift apart again.
  let {
    lines = [] as string[],
    colorFor,
    maxHeight = 200,
    charLimit = 4000,
    lineLimit = 300,
    anchorBottom = false,
  }: {
    lines?: string[];
    /** Per-line color override; undefined falls back to text-secondary. */
    colorFor?: (line: string) => string | undefined;
    maxHeight?: number;
    charLimit?: number;
    lineLimit?: number;
    /** Pin short content to the bottom (live console follow mode). */
    anchorBottom?: boolean;
  } = $props();

  // Split the FULL text into one <div> per line is bounded so the scrollable
  // list stays small: tool results can be tens of KB to MBs (git diffs, file
  // reads, command output). Cap both characters and line count.
  const rendered = $derived.by(() => {
    const raw = lines.join('\n');
    const sliced = raw.trimEnd().slice(0, charLimit);
    const all = sliced.split('\n');
    const clipped = all.length > lineLimit;
    const shown = clipped ? all.slice(0, lineLimit) : all;
    return {
      rows: shown.map((t) => ({ text: t, color: colorFor?.(t) ?? 'var(--text-secondary)' })),
      charsTruncated: raw.length > charLimit,
      linesTruncated: clipped,
      hiddenLines: all.length - shown.length,
    };
  });
</script>

<div class="output-log" class:anchor={anchorBottom} style={`max-height:${maxHeight}px`}>
  {#each rendered.rows as line, i (i)}
    <div class="line" style={`color:${line.color}`}>{line.text}</div>
  {/each}
  {#if rendered.linesTruncated || rendered.charsTruncated}
    <div class="more">
      … truncated
      {#if rendered.hiddenLines > 0}{rendered.hiddenLines} more lines{/if}
    </div>
  {/if}
</div>

<style>
  .output-log {
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .output-log.anchor {
    justify-content: flex-end;
  }
  .line {
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.55;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .more {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    padding-top: 5px;
  }
</style>
