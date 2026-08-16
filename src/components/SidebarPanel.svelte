<script lang="ts">
  import { statsFor, hasSessionStats, loadSessionStats } from '../stores/stats.svelte'
  import { sessions } from '../stores/sessions.svelte'
  import { ui, setPanelTab } from '../stores/ui.svelte'
  import { agents, agentsSummary, selectNode, toggleFollow } from '../stores/agents.svelte'
  import { hueTint } from '../lib/tint'
  import SlotStack from './SlotStack.svelte'
  import Icon from './Icon.svelte'
  import StatusDot from './StatusDot.svelte'
  import OutputLog from './OutputLog.svelte'
  import Pill from './Pill.svelte'

  let { sessionId }: { sessionId: string } = $props()

  const st = $derived(statsFor(sessionId))

  // Sidebar only mounts when opened. Load telemetry once per session (the
  // run loop pushes `session:stats` updates while active, and a session that
  // already has a payload must show THAT — not a re-fetch — when the panel
  // reopens or the tab is switched).
  $effect(() => {
    if (!hasSessionStats(sessionId)) loadSessionStats(sessionId)
  })

  // Ring: 58px canvas (r=24), segments scaled to `used` (design v2).
  const CIRC = 2 * Math.PI * 24
  const fmt = (n: number) => (n >= 1000 ? (n / 1000).toFixed(1) + 'k' : String(n))
  const totalUsed = $derived(st.context.used)
  const sessionName = $derived(
    sessions.list.find((x) => x.id === sessionId)?.name ?? sessionId
  )
  const segments = $derived.by(() => {
    const limit = Math.max(st.context.limit, 1)
    let acc = 0
    return st.context.segments.map((p) => {
      const frac = p.tokens / limit
      const clamped = Math.max(0, Math.min(frac, 1 - acc))
      const seg = {
        ...p,
        tokensLabel: fmt(p.tokens),
        dash: `${(CIRC * clamped).toFixed(2)} ${CIRC.toFixed(2)}`,
        offset: (-CIRC * acc).toFixed(2),
      }
      acc += frac
      return seg
    })
  })

  const badgeColor = { read: 'var(--text-muted)', edited: 'var(--warning)', created: 'var(--success)', deleted: 'var(--danger)' }
  const badgeLetter = { read: 'R', edited: 'E', created: 'C', deleted: 'D' }

  function onOpenFile(path: string) {
    // TODO(milestone 6) — open the diff/editor; toast placeholder for now.
    import('../stores/toast.svelte').then(({ pushToast }) =>
      pushToast(`opened diff: ${path.split('/').pop()}`, 'info')
    )
  }
  const hasDiff = (added: number | null | undefined, removed: number | null | undefined) =>
    added != null || removed != null

  // ---- Plan pane ----
  const plan = $derived(st.plan)
  const planDone = $derived(plan.items.filter((i) => i.state === 'done').length)
  const planProgress = $derived(`${planDone}/${plan.items.length}`)

  // ---- Changes pane (files + working tree merged) ----
  const files = $derived(st.files)
  const git = $derived(st.git)
  const changedCount = $derived(files.filter((f) => f.kind !== 'read').length)
  // Files first (with diffs), then pure git rows; each row's churn bar scales
  // against the max churn (design `changeRows`).
  const changeRows = $derived.by(() => {
    const withDiff = files.filter((f) => hasDiff(f.added, f.removed))
    const sorted = [...withDiff].sort((a, b) => ((b.added ?? 0) + (b.removed ?? 0)) - ((a.added ?? 0) + (a.removed ?? 0)))
    const maxChurn = Math.max(1, ...sorted.map((f) => (f.added ?? 0) + (f.removed ?? 0)))
    return sorted.map((f, i) => {
      const churn = (f.added ?? 0) + (f.removed ?? 0)
      return {
        path: f.path,
        badge: badgeLetter[f.kind] ?? '?',
        color: badgeColor[f.kind] ?? 'var(--text-muted)',
        added: f.added ?? 0,
        removed: f.removed ?? 0,
        barW: Math.round(34 * (churn / maxChurn)),
        delay: i * 70,
      }
    })
  })
  const gitRows = $derived(
    git.rows.map((g) => ({
      ...g,
      color: g.mark === 'A' ? 'var(--success)' : 'var(--warning)',
      barW: Math.round(34 * g.weight),
    }))
  )

  // ---- Agents pane ----
  const NODE_STATE = {
    running: { color: 'var(--success)', pulse: true },
    thinking: { color: 'var(--slot)', pulse: true },
    detached: { color: 'var(--accent)', pulse: false },
    done: { color: 'var(--text-muted)', pulse: false },
    exited: { color: 'var(--text-muted)', pulse: false },
    failed: { color: 'var(--danger)', pulse: false },
  }
  const tree = $derived(
    agents.nodes.map((n, i) => {
      const stt = NODE_STATE[n.state] ?? NODE_STATE.done
      const isAgent = n.kind === 'agent'
      const tint = isAgent ? hueTint(n.name) : 'var(--text-muted)'
      return {
        ...n,
        tint,
        isAgent,
        selected: agents.selectedId === n.id,
        icon: isAgent ? (n.depth === 0 ? 'hub' : 'account_tree') : 'terminal',
        iconColor: tint,
        dotColor: stt.color,
        pulse: stt.pulse,
        depth: n.depth,
      }
    })
  )
  const proc = $derived(
    agents.nodes.find((n) => n.id === agents.selectedId) ?? agents.nodes[0] ?? null
  )
  const procState = $derived(proc?.state ?? 'exited')
  const procIsAgent = $derived(!!proc && proc.kind === 'agent')
  const procColor = $derived(
    procIsAgent
      ? (procState === 'done' ? 'var(--text-muted)' : hueTint(proc?.name ?? ''))
      : procState === 'exited' ? 'var(--text-muted)'
        : procState === 'failed' ? 'var(--danger)'
          : procState === 'detached' ? 'var(--accent)' : 'var(--success)'
  )
  const procLive = $derived(
    procState === 'running' || procState === 'detached' || procState === 'thinking'
  )
  const procLines = $derived((proc && agents.logs[proc.id]) ?? [])
  const summary = $derived(agentsSummary())

  // Log line coloring (design consoleLines): errors → danger, warn → warning,
  // ✓ → success, › → muted, ⎇ → proc color, else secondary.
  function lineStyle(text: string): string {
    if (/error|fail|panic/i.test(text)) return 'var(--danger)'
    if (/warn/i.test(text)) return 'var(--warning)'
    if (text.startsWith('✓')) return 'var(--success)'
    if (text.startsWith('›')) return 'var(--text-muted)'
    if (text.startsWith('⎇')) return procColor
    return 'var(--text-secondary)'
  }

  const tabs = [
    { key: 'plan' as const, label: 'Plan', icon: 'checklist' },
    { key: 'agents' as const, label: 'Agents', icon: 'account_tree' },
    { key: 'changes' as const, label: 'Changes', icon: 'difference' },
  ]
</script>

<aside class="sidebar" aria-label="Session telemetry">
  <!-- pinned: context ring + cost → always labeled with its session -->
  <div class="pinned">
    <div class="pinned-head" title={sessionId}>
      <span class="ring-session">{sessionName}</span>
    </div>
    <div class="ring-row">
      <div class="ring-wrap">
        <svg width="58" height="58" viewBox="0 0 58 58" style="transform:rotate(-90deg)">
          <circle cx="29" cy="29" r="24" fill="none" stroke="var(--border-subtle)" stroke-width="7"></circle>
          {#each segments as seg (seg.label)}
            <circle
              cx="29" cy="29" r="24" fill="none"
              stroke={seg.color} stroke-width="7"
              stroke-dasharray={seg.dash} stroke-dashoffset={seg.offset}
              style="transition: stroke-dasharray 0.9s cubic-bezier(0.22, 0.8, 0.3, 1), stroke-dashoffset 0.9s cubic-bezier(0.22, 0.8, 0.3, 1);"
            ></circle>
          {/each}
        </svg>
        <div class="ring-center">
          <span class="ring-pct" class:over={st.context.percent > 100}>{st.context.percent}%</span>
        </div>
      </div>
      <div class="legend">
        {#each segments as seg, i (seg.label)}
          <div class="legend-row stagger" style={`--i:${i}`}>
            <span class="legend-dot" style={`background:${seg.color}`} aria-hidden="true"></span>
            <span class="legend-label">{seg.label}</span>
            <span class="legend-tokens">{seg.tokensLabel}</span>
          </div>
        {/each}
        <div class="legend-footer" title="Context in the next call / model window — used is the provider-counted input of the last call (system + full history); segments are its estimated composition">
          <span class="legend-used">{fmt(totalUsed)}</span>
          <span class="legend-div">/</span>
          <span class="legend-limit">{fmt(st.context.limit)}</span>
          <span class="legend-hint">in window</span>
        </div>
      </div>
    </div>

    <div class="cost">
      <div class="cost-head">
        <span class="section-label">Session cost</span>
        <span class="cost-total">{st.cost.total}</span>
      </div>
      {#each st.cost.rows as row, i (row.kind + row.model)}
        <div class:verify={row.kind === 'verification'} class="cost-row stagger" style={`--i:${i}`}>
          {#if row.kind === 'verification'}
            <span class="verify-badge" title="Harness verification (checks + review)">✓</span>
          {/if}
          <span class="cost-model">{row.model}</span>
          <span class="cost-meta">{row.tokens}</span>
          <span class="cost-amt">{row.cost}</span>
        </div>
      {/each}
      {#if st.cost.rows.length === 0}
        <div class="empty-hint">No usage yet this session</div>
      {/if}
    </div>
  </div>

  <!-- panel tabs (design panelTabs): Plan / Agents / Changes -->
  <div class="panel-tabs" role="tablist" aria-label="Sidebar panels">
    {#each tabs as t (t.key)}
      <button
        class="ptab"
        class:active={ui.panelTab === t.key}
        role="tab"
        aria-selected={ui.panelTab === t.key}
        onclick={() => setPanelTab(t.key)}
      >
        <span class="ptab-icon" aria-hidden="true"><Icon name={t.icon as never} size={15} /></span>
        <span class="ptab-label">{t.label}</span>
        {#if t.key === 'agents' && agents.nodes.length > 0}
          <Pill radius="pill" class="ptab-badge">{agents.nodes.length}</Pill>
        {/if}
        {#if t.key === 'changes' && files.length > 0}
          <Pill radius="pill" class="ptab-badge">{files.length}</Pill>
        {/if}
      </button>
    {/each}
  </div>

  <div class="panes">
    {#if ui.panelTab === 'plan'}
      <div class="pane" role="tabpanel" aria-label="Plan">
        <div class="pane-head">
          <span class="pane-ico" aria-hidden="true"><Icon name="checklist" size={15} /></span>
          <span class="pane-label">Plan</span>
          {#if plan.items.length > 0}<span class="pane-meta">{planProgress}</span>{/if}
        </div>
        <div class="pane-body">
          {#if plan.items.length === 0}
            <div class="empty-hint">No plan yet — drop a PLAN.md in the working directory.</div>
          {/if}
          {#each plan.items as item, i (item.label)}
            <div class="plan-row stagger" style={`--i:${i}`}>
              <span
                class="plan-icon"
                style={`--plan-color: ${item.state === 'done' ? 'var(--success)' : item.state === 'active' ? 'var(--accent)' : 'var(--text-muted)'}`}
                aria-hidden="true"
              >
                <Icon name={item.state === 'done' ? 'task_alt' : item.state === 'active' ? 'radio_button_checked' : 'radio_button_unchecked'} size={15} />
              </span>
              <span class="plan-label" class:done={item.state === 'done'} class:todo={item.state === 'todo'}>{item.label}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    {#if ui.panelTab === 'agents'}
      <div class="pane" role="tabpanel" aria-label="Agents">
        <div class="pane-head">
          <span class="pane-ico" aria-hidden="true"><Icon name="account_tree" size={15} /></span>
          <span class="pane-label">Agents</span>
          <Pill radius="pill" class="pane-badge">{summary}</Pill>
        </div>
        <div class="pane-body">
          {#if agents.nodes.length === 0}
            <!-- TODO(agents): backend events — this pane is ready but empty. -->
            <div class="upcoming">
              <span class="upcoming-pill">upcoming</span>
              <span class="upcoming-text">Sub-agents and detached processes will appear here with a live console.</span>
            </div>
          {:else}
            <div class="agent-tree">
              {#each tree as n, i (n.id)}
                <button
                  class="node"
                  class:selected={n.selected}
                  style={`--depth:${n.depth * 13}px; --tint:${n.iconColor}`}
                  aria-pressed={n.selected}
                  onclick={() => selectNode(n.id)}
                >
                  {#if n.depth > 0}<span class="stem" aria-hidden="true"></span>{/if}
                  <span class="node-ico" aria-hidden="true"><Icon name={n.icon as never} size={14} /></span>
                  <span class="node-name" class:proc={!n.isAgent}>{n.name}</span>
                  <StatusDot color={n.dotColor} live={n.pulse} size={5} />
                  <span class="node-meta">{n.meta}</span>
                </button>
              {/each}
            </div>

            <div class="console">
              <div class="console-head">
                <span class="c-ico" style={`color:${procColor}`} aria-hidden="true">
                  <Icon name={procIsAgent ? 'account_tree' : 'terminal'} size={14} />
                </span>
                <span class="c-cmd">{proc?.name ?? 'nothing selected'}</span>
                <span class="c-state" style={`color:${procColor}; background:color-mix(in oklch, ${procColor} 13%, transparent)`}>
                  {procIsAgent ? (procState === 'done' ? 'finished' : procState) : procState}
                </span>
                {#if proc}<span class="c-meta">{proc.meta}</span>{/if}
              </div>
              <div class="console-body" class:live={procLive && agents.follow}>
                {#if procLines.length === 0 && !procLive}
                  <div class="console-empty">No output yet.</div>
                {/if}
                <OutputLog
                  lines={procLines}
                  colorFor={lineStyle}
                  maxHeight={132}
                  anchorBottom={procLive && agents.follow}
                />
                {#if procLive && agents.follow}<div class="c-caret" style={`color:${procColor}`}>▌</div>{/if}
              </div>
              <div class="console-foot">
                <button class="follow" class:on={agents.follow} onclick={toggleFollow}>
                  <Icon name={agents.follow ? 'vertical_align_bottom' : 'pause'} size={13} />follow
                </button>
                {#if proc}<span class="c-meta">{procLines.length} lines</span>{/if}
              </div>
            </div>
          {/if}
        </div>
      </div>
    {/if}

    {#if ui.panelTab === 'changes'}
      <div class="pane" role="tabpanel" aria-label="Changes">
        <div class="pane-head">
          <span class="pane-ico" aria-hidden="true"><Icon name="difference" size={15} /></span>
          <span class="pane-label">Changes</span>
          {#if git.branch}
            <Pill radius="pill" icon="fork_right" class="branch-pill">{git.branch}</Pill>
          {/if}
        </div>
        <div class="git-counts">
          <span class="git-add">+{git.added}</span>
          <span class="git-del">−{git.removed}</span>
          <span class="git-files">{git.files} changed · {files.length} touched</span>
        </div>
        <div class="pane-body">
          {#each changeRows as f, i (f.path)}
            <button class="file-row stagger" style={`--i:${i}`} onclick={() => onOpenFile(f.path)}>
              <span class="file-badge" style={`--badge: ${f.color}`}>{f.badge}</span>
              <span class="file-path" title={f.path}>{f.path}</span>
              <span class="add">+{f.added}</span>
              <span class="remove">−{f.removed}</span>
              <span
                class="churn"
                style={`--bar: ${f.color}; --w: ${f.barW}px; --d: ${f.delay}ms`}
              ></span>
            </button>
          {/each}
          {#if changeRows.length === 0 && gitRows.length === 0}
            <div class="empty-hint">Nothing changed yet.</div>
          {/if}
          {#each gitRows as g, i (g.path)}
            <div class="git-row stagger" style={`--i:${i}`}>
              <span class="git-mark" style={`--mark: ${g.color}`}>{g.mark}</span>
              <span class="git-path">{g.path}</span>
              <span class="churn" style={`--bar: ${g.color}; --w: ${g.barW}px; --d: ${i * 70}ms`}></span>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- plugin-injectable sections -->
    <SlotStack slot="sidebar-section" context={sessionId} />
  </div>
</aside>

<style>
  /* Design: 300px (270px narrow) right sidebar on --bg-elevated. */
  .sidebar {
    width: 300px;
    flex: none;
    border-left: 1px solid var(--border-default);
    background: var(--bg-elevated);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .pinned {
    flex: none;
    padding: 12px 14px 11px;
    border-bottom: 1px solid var(--border-default);
  }
  .pinned-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 9px;
    min-width: 0;
  }
  .ring-session {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-body);
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .ring-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .ring-wrap {
    position: relative;
    width: 58px;
    height: 58px;
    flex: none;
  }
  .ring-center {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .ring-pct {
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 15px;
    color: var(--text-primary);
    line-height: 1;
  }
  /* Over the window — the point where auto-compaction will kick in. */
  .ring-pct.over {
    color: var(--danger);
  }
  .legend {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .legend-row {
    display: flex;
    align-items: center;
    gap: 7px;
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-secondary);
  }
  .legend-dot {
    width: 7px;
    height: 7px;
    border-radius: 2px;
    flex: none;
  }
  .legend-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .legend-tokens {
    color: var(--text-muted);
  }
  .legend-footer {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    padding-top: 4px;
    border-top: 1px solid var(--border-subtle);
    display: flex;
    align-items: baseline;
    gap: 3px;
  }
  .legend-used {
    color: var(--text-secondary);
  }
  .legend-hint {
    margin-left: auto;
    color: var(--text-muted);
    opacity: 0.8;
  }
  .cost {
    margin-top: 11px;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .cost-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .section-label {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .cost-total {
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 16px;
    color: var(--text-primary);
  }
  .cost-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-secondary);
  }
  .cost-row.verify {
    color: var(--accent, var(--text-secondary));
    border-top: 1px dashed var(--border-subtle);
    margin-top: 5px;
    padding-top: 5px;
  }
  .cost-row.verify .cost-model {
    font-weight: 600;
  }
  .cost-row.verify .cost-amt {
    color: var(--accent, var(--text-secondary));
    font-weight: 600;
  }
  .verify-badge {
    font-size: 9px;
    color: var(--accent, var(--text-primary));
  }
  .cost-model {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cost-meta { color: var(--text-muted); }
  .cost-amt { color: var(--text-secondary); }

  /* Panel tabs (design panelTabs): flex-1 buttons, accent underline. */
  .panel-tabs {
    display: flex;
    align-items: stretch;
    flex: none;
    padding: 0 8px;
    border-bottom: 1px solid var(--border-default);
  }
  .ptab {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 9px 2px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
    font-family: var(--font-body);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
    transition: color 0.2s ease, border-color 0.2s ease, background 0.2s ease;
  }
  .ptab:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }
  .ptab.active {
    color: var(--text-primary);
    border-bottom-color: var(--accent);
  }
  .ptab-icon :global(.msr) {
    font-size: 15px;
    line-height: 1;
    color: var(--text-muted);
    transition: color 0.2s ease;
  }
  .ptab.active .ptab-icon :global(.msr) {
    color: var(--accent);
  }
  /* Sidebar badges — shared Pill. */
  :global(.ptab-badge) {
    padding: 1px 5px;
  }
  :global(.pane-badge) {
    color: var(--text-secondary);
  }

  .panes {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .pane {
    padding: 13px 16px;
    animation: fadeIn 0.26s ease both;
  }
  .pane-head {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-bottom: 9px;
  }
  .pane-ico {
    font-size: 15px;
    line-height: 1;
    color: var(--text-muted);
  }
  .pane-label {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .pane-meta {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
  }
  .pane-body {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  /* Design stagger entrance: rows fade up 45ms apart (sidebar only renders
     when opened, so the animation plays once per open) — global .stagger. */

  .plan-row {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .plan-icon {
    font-size: 15px;
    line-height: 1.3;
    flex: none;
    color: var(--plan-color);
  }
  .plan-label {
    font-family: var(--font-body);
    font-size: 12.5px;
    line-height: 1.4;
    color: var(--text-primary);
  }
  .plan-label.done {
    text-decoration: line-through;
    opacity: 0.7;
    color: var(--text-secondary);
  }
  .plan-label.todo {
    color: var(--text-muted);
  }

  /* Changes rows */
  .git-counts {
    display: flex;
    gap: 12px;
    font-family: var(--font-mono);
    font-size: 11px;
    margin-bottom: 10px;
  }
  .git-add { color: var(--success); }
  .git-del { color: var(--danger); }
  .git-files { color: var(--text-muted); }
  .file-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 7px;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
    transition: background 0.2s ease;
  }
  .file-row:hover { background: var(--surface-hover); }
  .file-badge {
    width: 16px;
    height: 16px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 2px;
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 600;
    color: var(--badge);
    background: color-mix(in oklch, var(--badge) 18%, transparent);
  }
  .file-path {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .add {
    font-family: var(--font-mono);
    font-size: 10px;
    flex: none;
    color: var(--success);
  }
  .remove {
    font-family: var(--font-mono);
    font-size: 10px;
    flex: none;
    color: var(--danger);
  }
  .churn {
    width: var(--w);
    height: 4px;
    border-radius: 2px;
    background: var(--bar);
    transition: width 0.7s cubic-bezier(0.22, 0.8, 0.3, 1);
    transition-delay: var(--d, 0ms);
    flex: none;
  }
  .git-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .git-mark {
    width: 14px;
    flex: none;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    color: var(--mark);
  }
  .git-path {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Agents pane */
  .upcoming {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    border: 1px dashed color-mix(in oklch, var(--accent) 35%, var(--border-default));
    border-radius: var(--radius-sm);
    padding: 12px;
  }
  .upcoming-pill {
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--accent);
    border: 1px solid color-mix(in oklch, var(--accent) 42%, transparent);
    padding: 2px 7px;
  }
  .upcoming-text {
    font-family: var(--font-body);
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-muted);
  }
  .agent-tree {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .node {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 4px 6px 4px calc(6px + var(--depth));
    border-radius: var(--radius-sm);
    cursor: pointer;
    background: none;
    border: none;
    text-align: left;
    transition: background 0.2s ease;
  }
  .node:hover {
    background: var(--surface-hover);
  }
  .node.selected {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 0 0 var(--tint);
  }
  .stem {
    width: 6px;
    height: 1px;
    flex: none;
    background: color-mix(in oklch, var(--tint) 55%, transparent);
  }
  .node-ico {
    flex: none;
    display: flex;
    color: var(--tint);
  }
  .node-ico :global(.msr) {
    font-size: 14px;
    line-height: 1;
  }
  .node-name {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .node-name.proc {
    color: var(--text-secondary);
  }
  .node-meta {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    flex: none;
    max-width: 96px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .console {
    margin-top: 12px;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    background: var(--bg-base);
    overflow: hidden;
  }
  .console-head {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 7px 9px;
    border-bottom: 1px solid var(--border-subtle);
    min-width: 0;
  }
  .c-ico {
    flex: none;
    display: flex;
  }
  .c-ico :global(.msr) {
    font-size: 14px;
    line-height: 1;
  }
  .c-cmd {
    flex: 1 1 auto;
    min-width: 90px;
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .c-state {
    flex: none;
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    border-radius: 2px;
    padding: 1px 6px;
  }
  .c-meta {
    flex: 0 1 auto;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .console-body {
    height: 132px;
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: color-mix(in oklch, var(--text-primary) 4%, transparent);
  }
  .console-body.live {
    /* anchor to the bottom so new lines are visible */
    justify-content: flex-end;
  }
  .c-caret {
    font-family: var(--font-mono);
    font-size: 10px;
    animation: cursorblink 1s step-start infinite;
  }
  .console-empty {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
  }
  .console-foot {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    border-top: 1px solid var(--border-subtle);
  }
  .follow {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
    font-family: var(--font-mono);
    font-size: 9.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 7px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    border: 1px solid var(--border-default);
    background: transparent;
    color: var(--text-muted);
    transition: background 0.2s ease, color 0.2s ease, border-color 0.2s ease;
  }
  .follow :global(.msr) {
    font-size: 13px;
    line-height: 1;
  }
  .follow.on {
    border-color: color-mix(in oklch, var(--accent) 45%, transparent);
    background: var(--accent-soft);
    color: var(--accent);
  }
  .empty-hint {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    padding: 2px 0;
  }
</style>
