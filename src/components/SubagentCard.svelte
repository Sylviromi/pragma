<script lang="ts">
  // Design sub-agent block (`Pragma UI Kit v2.dc.html` → buildBlock kind
  // 'subagent'): tinted card with a left rail, agent mark, name + "sub-agent"
  // tag + meta, the task copy, per-step rows (tool icon + label + detail),
  // and a footer with a token chip + result.
  //
  // TODO(agents): no backend event produces this block yet (see TODO.md) —
  // the renderer branch in MessageList stays inert until sub-agent events
  // land. Data shape mirrors the design's TURN_SCRIPTS.

  export interface SubagentStep {
    tool: string
    label: string
    detail?: string
    state?: 'running' | 'done' | 'pending'
  }

  let {
    name,
    tint = 'var(--accent)',
    meta = '',
    runState = 'done' as 'running' | 'done',
    task = '',
    steps = [] as SubagentStep[],
    tokens = '',
    result = '',
  }: {
    name: string
    tint?: string
    meta?: string
    runState?: 'running' | 'done'
    task?: string
    steps?: SubagentStep[]
    tokens?: string
    result?: string
  } = $props()

  let open = $state(true)

  const TOOL_ICONS: Record<string, string> = {
    read: 'description', edit: 'edit_document', run: 'terminal',
    search: 'travel_explore', grep: 'search', write: 'note_add',
  }
  const running = $derived(runState === 'running')
</script>

<div class="subagent" style={`--tint:${tint}`} class:streaming={running}>
  <span class="rail" class:streaming={running} aria-hidden="true"></span>
  <button
    class="head"
    onclick={() => (open = !open)}
    aria-expanded={open}
    aria-label={open ? 'Collapse sub-agent' : 'Expand sub-agent'}
  >
    <span class="mark" aria-hidden="true">
      <svg width="15" height="15" viewBox="0 0 32 32" fill="none">
        <path d="M16 9.5 8.5 21.5M16 9.5 23.5 21.5" stroke={tint} stroke-width="1.6" opacity="0.6"></path>
        <rect x="13.2" y="6.6" width="5.6" height="5.6" fill={tint}></rect>
        <rect x="5.8" y="18.8" width="5.2" height="5.2" stroke={tint} stroke-width="1.8"></rect>
        <rect x="21" y="18.8" width="5.2" height="5.2" stroke={tint} stroke-width="1.8"></rect>
      </svg>
    </span>
    <span class="name">{name}</span>
    <span class="kind">sub-agent</span>
    {#if meta}<span class="meta">{meta}</span>{/if}
    <span class="chev" class:open aria-hidden="true"><span class="msr">expand_more</span></span>
  </button>

  <div class="body-grid" class:closed={!open}>
    <div class="body">
      {#if task}<div class="task">{task}</div>{/if}
      <div class="steps">
        {#each steps as st, i (st.label + i)}
          <div class="step" style={`--i:${i}`}>
            <span class="step-ico" class:running={st.state === 'running'} aria-hidden="true">
              <span class="msr">{TOOL_ICONS[st.tool] ?? 'build'}</span>
            </span>
            <span class="step-label">{st.label}</span>
            {#if st.detail}<span class="step-detail">{st.detail}</span>{/if}
          </div>
        {/each}
      </div>
      <div class="foot">
        {#if tokens}<span class="token-chip">{tokens} tok</span>{/if}
        {#if result}<span class="result">{result}</span>{/if}
      </div>
    </div>
  </div>
</div>

<style>
  /* Design: tinted card, 2px rail, agent mark, mono name. */
  .subagent {
    position: relative;
    display: flex;
    flex-direction: column;
    margin-left: 10px;
    border: 1px solid color-mix(in oklch, var(--tint) 32%, transparent);
    border-radius: var(--radius-sm);
    background: color-mix(in oklch, var(--tint) 6%, transparent);
    overflow: hidden;
    transition: border-color 0.3s ease;
    animation: blockIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  .rail {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--tint);
    opacity: 0.7;
  }
  .rail.streaming {
    opacity: 1;
    animation: railPulse 1.4s ease-in-out infinite;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 11px 7px 12px;
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    transition: background 0.2s ease;
  }
  .head:hover {
    background: color-mix(in oklch, var(--tint) 8%, transparent);
  }
  .mark {
    display: flex;
    flex: none;
  }
  .name {
    flex: 1 1 auto;
    min-width: 40px;
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--tint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .kind {
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
    background: var(--bg-overlay);
    border-radius: 2px;
    padding: 1px 6px;
    flex: none;
  }
  .meta {
    flex: 0 1 auto;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chev {
    flex: none;
    display: flex;
    color: var(--text-muted);
    transition: transform 0.25s cubic-bezier(0.22, 0.8, 0.3, 1);
  }
  .chev .msr {
    font-size: 16px;
    line-height: 1;
  }
  .chev.open {
    transform: rotate(180deg);
  }
  .body-grid {
    display: grid;
    grid-template-rows: 1fr;
    min-height: 0;
    transition: grid-template-rows 0.32s cubic-bezier(0.4, 0, 0.2, 1);
  }
  .body-grid.closed {
    grid-template-rows: 0fr;
  }
  .body {
    min-height: 0;
    overflow: hidden;
    padding: 2px 13px 11px 12px;
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  .task {
    font-family: var(--font-body);
    font-size: 12.5px;
    line-height: 1.55;
    color: var(--text-secondary);
    text-wrap: pretty;
  }
  .steps {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .step {
    display: flex;
    align-items: center;
    gap: 7px;
    animation: rowIn 0.3s cubic-bezier(0.22, 0.8, 0.3, 1) both;
    animation-delay: calc(var(--i) * 60ms);
  }
  .step-ico {
    flex: none;
    display: flex;
    color: var(--text-muted);
  }
  .step-ico .msr {
    font-size: 13px;
    line-height: 1;
  }
  .step-ico.running {
    color: var(--warning);
    animation: dotPulse 1.6s ease-in-out infinite;
  }
  .step-label {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .step-detail {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    flex: none;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
  }
  .token-chip {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    background: var(--bg-overlay);
    border-radius: 9px;
    padding: 2px 8px;
  }
  .result {
    flex: 1;
    min-width: 0;
    font-family: var(--font-body);
    font-size: 11.5px;
    color: var(--text-muted);
    text-align: right;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
