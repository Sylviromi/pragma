<script lang="ts">
  // Design process block (`Pragma UI Kit v2.dc.html` → buildBlock kind
  // 'process'): dashed-bordered card showing a detached/running command with a
  // state-colored icon + chip, an expandable log, and action buttons.
  //
  // TODO(agents): no backend event produces this block yet (see TODO.md) —
  // renderer branches stay inert until detached-process events land.
  // Card shell (chevron + collapse) lives in Collapsible; the log is the
  // shared OutputLog pane (audit D4).
  import Icon from './Icon.svelte'
  import Collapsible from './Collapsible.svelte'
  import OutputLog from './OutputLog.svelte'
  import Button from './Button.svelte'
  import StatusDot from './StatusDot.svelte'
  import Pill from './Pill.svelte'

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

  function colorFor(line: string): string | undefined {
    return /error|fail/i.test(line) ? 'var(--danger)' : undefined
  }
</script>

<Collapsible
  bind:open
  clickable={false}
  chevron={log.length > 0}
  label="Toggle log"
  class="process"
  style={`--pcolor:${color}`}
>
  {#snippet header()}
    <span class="p-ico" aria-hidden="true"><Icon name={icon as never} size={15} /></span>
    <span class="p-cmd">{cmd}</span>
    <Pill color={color}>
      <StatusDot color={color} live={runState === 'running'} size={5} />{stateLabel}
    </Pill>
    {#if meta}<span class="p-meta">{meta}</span>{/if}
  {/snippet}

  {#if log.length > 0}
    <div class="log">
      <OutputLog lines={log} {colorFor} maxHeight={160} />
    </div>
  {/if}

  {#snippet footer()}
    {#each actions as act (act.label)}
      <Button size="xs" variant="ghost" icon={act.icon as never} onclick={act.onClick}>{act.label}</Button>
    {/each}
  {/snippet}
</Collapsible>

<style>
  /* Root theme — dashed border tinted by state. */
  :global(.process) {
    background: var(--bg-base);
    border: 1px dashed color-mix(in oklch, var(--pcolor) 45%, var(--border-default));
  }
  .p-ico {
    flex: none;
    display: flex;
    color: var(--pcolor);
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
  .log {
    background: var(--bg-elevated);
    border-top: 1px solid var(--border-subtle);
    padding: 8px 12px;
  }
</style>
