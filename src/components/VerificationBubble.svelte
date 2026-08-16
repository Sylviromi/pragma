<script lang="ts">
  // Dedicated bubble for the harness verification phase (loop-done checks +
  // independent review). Distinct from user/assistant bubbles AND from the
  // error/notice rows: a self-contained card that (a) shows the live phase
  // with a Stop button while running, and (b) as a persisted role='verification'
  // message once finished — it does NOT disappear after the next prompt.
  import type { VerificationInfo } from '../lib/types'
  import Icon from './Icon.svelte'

  let {
    run = null,
    phase = null,
    onStop,
  }: {
    run?: VerificationInfo | null
    phase?: 'checks' | 'review' | null
    onStop?: () => void
  } = $props()

  const live = $derived(phase !== null)

  // Live elapsed timer — ticks only while a phase is active.
  let elapsed = $state(0)
  let liveStart = $state(0)
  $effect(() => {
    if (!live) {
      elapsed = 0
      return
    }
    liveStart = Date.now()
    elapsed = 0
    const t = setInterval(() => {
      elapsed = Date.now() - liveStart
    }, 100)
    return () => clearInterval(t)
  })

  function fmtDuration(ms: number): string {
    if (ms < 1000) return `${ms}ms`
    const s = ms / 1000
    if (s < 60) return `${s.toFixed(1)}s`
    return `${Math.floor(s / 60)}m ${Math.round(s % 60)}s`
  }

  function fmtTokens(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}m`
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
    return String(n)
  }

  const title = $derived.by(() => {
    if (phase) return phase === 'checks' ? 'Running project checks' : 'Reviewing changes'
    if (!run) return 'Verification'
    if (run.cancelled) return 'Verification stopped'
    return run.passed ? 'Verification passed' : 'Verification found issues'
  })

  const statusClass = $derived.by(() => {
    if (phase) return 'running'
    if (!run) return 'running'
    if (run.cancelled) return 'cancelled'
    return run.passed ? 'pass' : 'fail'
  })

  const icon = $derived.by(() => {
    if (phase) return 'progress_activity'
    if (!run) return 'fact_check'
    if (run.cancelled) return 'stop'
    return run.passed ? 'verified' : 'error'
  })

  // Meta line: duration + tokens + round, shown for finished bubbles.
  const meta = $derived.by(() => {
    if (!run) return ''
    const bits: string[] = []
    if (run.durationMs > 0) bits.push(fmtDuration(run.durationMs))
    if (run.tokens > 0) bits.push(`${fmtTokens(run.tokens)} tokens`)
    if (run.rounds > 0 && !run.passed && !run.cancelled) {
      bits.push(`fix round ${run.rounds}`)
    }
    return bits.join(' · ')
  })
</script>

<div class="vbubble" class:pass={statusClass === 'pass'} class:fail={statusClass === 'fail'} class:running={live} class:cancelled={statusClass === 'cancelled'}>
  <div class="vhead">
    <span class="vicon {statusClass}" class:spin={live} aria-hidden="true">
      <Icon name={icon as never} size={15} />
    </span>
    <span class="vtitle">{title}</span>
    <span class="vtag">harness check</span>
  </div>

  {#if live}
    <div class="vbody">
      <div class="vline">
        <span class="vline-icon" aria-hidden="true"><Icon name={phase === 'checks' ? 'terminal' : 'rate_review'} size={13} /></span>
        <span>{phase === 'checks' ? 'Running the project’s test/check command — no model involved' : 'One isolated model call — task + diff + check output only (never the conversation)'}</span>
      </div>
      <div class="vmeta-row">
        <span class="vmeta-live">{elapsed > 0 ? fmtDuration(elapsed) : ''}</span>
        {#if onStop}
          <button class="vstop" onclick={onStop} aria-label="Stop verification">
            <Icon name="stop" size={13} />Stop
          </button>
        {/if}
      </div>
    </div>
  {:else if run}
    <div class="vbody">
      {#if run.summary}
        <pre class="vsummary">{run.summary}</pre>
      {/if}
      {#if run.findings.length > 0}
        <ul class="vfindings">
          {#each run.findings as f (f)}
            <li>{f}</li>
          {/each}
        </ul>
      {/if}
      {#if run.checksLabel}
        <div class="vline vline-sub">
          <span class="vline-icon" aria-hidden="true"><Icon name="terminal" size={13} /></span>
          <span>Checks: {run.checksLabel} — {run.passed ? 'passed' : 'failed'}</span>
        </div>
      {/if}
      {#if run.reviewUsed}
        <div class="vline vline-sub">
          <span class="vline-icon" aria-hidden="true"><Icon name="rate_review" size={13} /></span>
          <span>Independent review: {run.passed ? 'accepted' : 'rejected'}</span>
        </div>
      {/if}
    </div>
    {#if meta}
      <div class="vmeta">{meta}</div>
    {/if}
  {/if}
</div>

<style>
  .vbubble {
    align-self: flex-start;
    max-width: min(560px, 100%);
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-left: 3px solid var(--text-muted);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    color: var(--text-primary);
    font-size: var(--font-size-sm);
  }
  .vbubble.pass {
    border-left-color: var(--success, var(--accent));
  }
  .vbubble.fail {
    border-left-color: var(--warning, var(--accent));
  }
  .vbubble.running {
    border-left-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 4%, var(--bg-elevated));
  }
  .vbubble.cancelled {
    border-left-color: var(--text-muted);
  }

  .vhead {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .vicon {
    display: inline-flex;
    font-size: 15px;
    color: var(--text-muted);
  }
  .vicon.pass { color: var(--success, var(--accent)); }
  .vicon.fail { color: var(--warning, var(--accent)); }
  .vicon.running { color: var(--accent); }
  .vicon.spin :global(.msr) { animation: spin 1s linear infinite; }
  .vtitle {
    font-weight: 600;
    font-size: 12.5px;
  }
  .vtag {
    margin-left: auto;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }

  .vbody {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .vsummary {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--text-secondary);
  }
  .vfindings {
    margin: 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .vfindings li {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
  }
  .vline {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--text-secondary);
  }
  .vline-sub { margin-top: 2px; }
  .vline-icon { font-size: 13px; color: var(--text-muted); }

  .vmeta-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 8px;
  }
  .vmeta-live {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
  }
  .vstop {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--text-secondary);
    background: transparent;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm, 4px);
    padding: 3px 8px;
    cursor: pointer;
  }
  .vstop:hover {
    color: var(--danger, var(--text-primary));
    border-color: color-mix(in srgb, var(--danger, var(--text-primary)) 40%, var(--border-default));
  }
  .vmeta {
    margin-top: 8px;
    padding-top: 6px;
    border-top: 1px solid var(--border-subtle);
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
  }
</style>
