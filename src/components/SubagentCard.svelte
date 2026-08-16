<script lang="ts">
  // Design sub-agent block (`Pragma UI Kit v2.dc.html` → buildBlock kind
  // 'subagent'): tinted card with a left rail, agent mark, name + "sub-agent"
  // tag + meta, the task copy, per-step rows (tool icon + label + detail),
  // and a footer with a token chip + result.
  //
  // TODO(agents): no backend event produces this block yet (see TODO.md) —
  // the renderer branch in MessageList stays inert until sub-agent events
  // land. Data shape mirrors the design's TURN_SCRIPTS.
  // Card shell (rail + chevron + collapse) lives in Collapsible; the agent
  // mark is the shared AgentMark component (was inlined before the audit).
  import AgentMark from './AgentMark.svelte';
  import Collapsible from './Collapsible.svelte';
  import Icon from './Icon.svelte';
  import Pill from './Pill.svelte';
  import { toolIcon } from '../lib/toolIcons';

  export interface SubagentStep {
    tool: string;
    label: string;
    detail?: string;
    state?: 'running' | 'done' | 'pending';
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
    name: string;
    tint?: string;
    meta?: string;
    runState?: 'running' | 'done';
    task?: string;
    steps?: SubagentStep[];
    tokens?: string;
    result?: string;
  } = $props();

  let open = $state(true);

  const running = $derived(runState === 'running');
</script>

<Collapsible
  bind:open
  rail
  railColor={tint}
  streaming={running}
  label={open ? 'Collapse sub-agent' : 'Expand sub-agent'}
  class="subagent"
  style={`--tint:${tint}`}
>
  {#snippet header()}
    <span class="mark" aria-hidden="true"><AgentMark {tint} size={15} /></span>
    <span class="name">{name}</span>
    <Pill>sub-agent</Pill>
    {#if meta}<span class="meta">{meta}</span>{/if}
  {/snippet}

  <div class="body">
    {#if task}<div class="task">{task}</div>{/if}
    <div class="steps">
      {#each steps as st, i (st.label + i)}
        <div class="step" style={`--i:${i}`}>
          <span class="step-ico" class:running={st.state === 'running'} aria-hidden="true">
            <Icon name={toolIcon(st.tool) as never} size={13} />
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
</Collapsible>

<style>
  /* Root theme — tinted card, class + style land on Collapsible's root. */
  :global(.subagent) {
    margin-left: 10px;
    border: 1px solid color-mix(in oklch, var(--tint) 32%, transparent);
    background: color-mix(in oklch, var(--tint) 6%, transparent);
    transition: border-color 0.3s ease;
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

  .body {
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
  .step-ico :global(.msr) {
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
