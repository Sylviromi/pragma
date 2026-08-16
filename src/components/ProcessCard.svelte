<script lang="ts">
  // Design process block (`Pragma UI Kit v2.dc.html` → buildBlock kind
  // 'process'): dashed-bordered card showing a detached/running command with a
  // state-colored icon + chip, an expandable log, and action buttons.
  //
  // TODO(agents): no backend event produces this block yet (see TODO.md) —
  // renderer branches stay inert until detached-process events land.

  export interface ProcessAction {
    label: string
    icon: string
    onClick?: () => void
  }

  let {
    cmd = '',
    runState = 'running' as 'running' | 'detached' | 'exited' | 'failed',
    exitCode = null as number | null,
    meta = '',
    log = [] as string[],
    actions = [] as ProcessAction[],
  }: {
    cmd: string
    runState?: 'running' | 'detached' | 'exited' | 'failed'
    exitCode?: number | null
    meta?: string
    log?: string[]
    actions?: ProcessAction[]
  } = $props()

  let open = $state(false)

  const color = $derived(
    runState === 'exited' ? 'var(--text-muted)'
      : runState === 'failed' ? 'var(--danger)'
        : runState === 'detached' ? 'var(--accent)' : 'var(--success)'
  )
  const stateLabel = $derived(
    runState === 'detached' ? 'detached'
      : runState === 'exited' ? `exited ${exitCode ?? 0}`
        : runState
  )
  const icon = $derived(runState === 'detached' ? 'open_in_new' : 'terminal')
</script>

<div class="process" style={`--pcolor:${color}`}>
  <div class="head">
    <span class="p-ico" aria-hidden="true"><span class="msr">{icon}</span></span>
    <span class="p-cmd">{cmd}</span>
    <span class="p-state">
      <span class="p-dot" class:blink={runState === 'running'}></span>{stateLabel}
    </span>
    {#if meta}<span class="p-meta">{meta}</span>{/if}
    {#if log.length > 0}
      <button class="p-logbtn" aria-label="Toggle log" onclick={() => (open = !open)}>
        <span class="chev" class:open aria-hidden="true"><span class="msr">expand_more</span></span>
      </button>
    {/if}
  </div>

  <div class="log-grid" class:open>
    <div class="log">
      {#each log as line, li (li)}
        <div class="log-line" class:err={/error|fail/i.test(line)}>{line}</div>
      {/each}
    </div>
  </div>

  {#if actions.length > 0}
    <div class="actions">
      {#each actions as act (act.label)}
        <button class="act" onclick={act.onClick}>
          <span class="msr" aria-hidden="true">{act.icon}</span>{act.label}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* Design: dashed border tinted by state, state chip with dot. */
  .process {
    display: flex;
    flex-direction: column;
    background: var(--bg-base);
    border: 1px dashed color-mix(in oklch, var(--pcolor) 45%, var(--border-default));
    border-radius: var(--radius-sm);
    overflow: hidden;
    animation: blockIn 0.32s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 9px 7px 11px;
  }
  .p-ico {
    flex: none;
    display: flex;
    color: var(--pcolor);
  }
  .p-ico .msr {
    font-size: 15px;
    line-height: 1;
  }
  .p-cmd {
    flex: 1 1 auto;
    min-width: 40px;
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .p-state {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex: none;
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--pcolor);
    background: color-mix(in oklch, var(--pcolor) 13%, transparent);
    border-radius: 2px;
    padding: 2px 7px;
  }
  .p-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--pcolor);
  }
  .p-dot.blink {
    animation: cursorblink 1.2s ease-in-out infinite;
  }
  .p-meta {
    flex: 0 1 auto;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .p-logbtn {
    background: none;
    border: none;
    padding: 1px;
    display: flex;
    cursor: pointer;
    color: var(--text-muted);
    flex: none;
  }
  .chev {
    display: flex;
    transition: transform 0.22s ease;
  }
  .chev .msr {
    font-size: 15px;
    line-height: 1;
  }
  .chev.open {
    transform: rotate(180deg);
  }
  .log-grid {
    display: grid;
    grid-template-rows: 0fr;
    min-height: 0;
    background: var(--bg-elevated);
    transition: grid-template-rows 0.32s cubic-bezier(0.4, 0, 0.2, 1), border-top-width 0.32s step-start;
    border-top: 0 solid var(--border-subtle);
  }
  .log-grid.open {
    grid-template-rows: 1fr;
    border-top-width: 1px;
  }
  .log {
    min-height: 0;
    overflow: hidden;
    padding: 8px 12px;
    max-height: 160px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .log-line {
    font-family: var(--font-mono);
    font-size: 10.5px;
    line-height: 1.55;
    white-space: pre-wrap;
    color: var(--text-secondary);
  }
  .log-line.err {
    color: var(--danger);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 11px 9px;
  }
  .act {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-family: var(--font-body);
    font-size: 11.5px;
    color: var(--text-secondary);
    background: none;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    padding: 3px 9px;
    cursor: pointer;
    transition: border-color 0.2s ease, color 0.2s ease;
  }
  .act .msr {
    font-size: 13px;
    line-height: 1;
  }
  .act:hover {
    border-color: var(--text-muted);
    color: var(--text-primary);
  }
</style>
