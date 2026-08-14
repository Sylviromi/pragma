<script lang="ts">
  import { statsFor, hasSessionStats, loadSessionStats } from '../stores/stats.svelte'
  import { sessions, openTab } from '../stores/sessions.svelte'
  import { chat, STATUS_META, type AgentStatus } from '../stores/chat.svelte'
  import SlotStack from './SlotStack.svelte'

  let { sessionId }: { sessionId: string } = $props()

  const st = $derived(statsFor(sessionId))

  // Sidebar only mounts when opened. Load telemetry once per session (the
  // run loop pushes `session:stats` updates while active, and a session that
  // already has a payload must show THAT — not a re-fetch — when the panel
  // reopens or the tab is switched).
  $effect(() => {
    if (!hasSessionStats(sessionId)) loadSessionStats(sessionId)
  })

  // Sidebar lists only the *open* sessions (the restored tabs above); the
  // full history stays on the tab bar side and in the DB. Clicking a row
  // switches to that session's already-open tab.
  const sessionRows = $derived(
    sessions.openTabs.map((id) => {
      const s = sessions.list.find((x) => x.id === id)
      const status = (chat.status[id] ?? 'idle') as AgentStatus
      const meta = STATUS_META[status]
      return {
        id,
        name: s?.name ?? id,
        status: meta.label,
        dot: meta.color,
        live: status !== 'idle',
        onClick: () => openTab(id),
      }
    })
  )

  const CIRC = 2 * Math.PI * 35
  const fmt = (n: number) => (n >= 1000 ? (n / 1000).toFixed(1) + 'k' : String(n))
  const totalUsed = $derived(st.context.used)
  // Name of the session this ring belongs to — with multiple open tabs the
  // pinned ring would otherwise look unattributed (its numbers are THIS
  // session's: used = the provider-counted tokens the last call sent).
  const sessionName = $derived(
    sessions.list.find((x) => x.id === sessionId)?.name ?? sessionId
  )
  const segments = $derived.by(() => {
    // Segments are scaled by the backend to sum exactly to `used`, so the
    // ring's filled fraction always equals percent. The cumulative clamp at
    // 1.0 only guards an over-budget session (>100%) from overflowing the
    // circumference (the % text still shows the true number).
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
  // Derived file-row helpers (class names must be valid CSS identifiers).
  const hasDiff = (added: number | null | undefined, removed: number | null | undefined) =>
    added != null || removed != null
</script>

<aside class="sidebar" aria-label="Session telemetry">
  <!-- pinned: context ring + cost → always labeled with its session -->
  <div class="pinned">
    <div class="pinned-head" title={sessionId}>
      <span class="ring-session">{sessionName}</span>
    </div>
    <div class="ring-row">
      <div class="ring-wrap">
        <svg width="84" height="84" viewBox="0 0 84 84" style="transform:rotate(-90deg)">
          <circle cx="42" cy="42" r="35" fill="none" stroke="var(--border-subtle)" stroke-width="9"></circle>
          {#each segments as seg (seg.label)}
            <circle
              cx="42" cy="42" r="35" fill="none"
              stroke={seg.color} stroke-width="9"
              stroke-dasharray={seg.dash} stroke-dashoffset={seg.offset}
              style="transition: stroke-dasharray 0.9s cubic-bezier(0.22, 0.8, 0.3, 1), stroke-dashoffset 0.9s cubic-bezier(0.22, 0.8, 0.3, 1);"
            ></circle>
          {/each}
        </svg>
        <div class="ring-center">
          <span class="ring-pct" class:over={st.context.percent > 100}>{st.context.percent}%</span>
          <span class="ring-label">context</span>
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

  <div class="sections">
    <!-- sessions -->
    <section class="sec">
      <div class="sec-label">Sessions</div>
      <div class="sec-body">
        {#each sessionRows as row, i (row.id)}
          <button class="row-btn stagger" class:active={row.id === sessionId} style={`--i:${i}`} onclick={row.onClick}>
            <span class="dot" class:live={row.live} style={`background:${row.dot}`} aria-hidden="true"></span>
            <span class="row-name">{row.name}</span>
            <span class="row-status">{row.status}</span>
          </button>
        {/each}
      </div>
    </section>

    <!-- plan: only when a PLAN.md exists in the working dir with tasks -->
    {#if st.plan.items.length > 0}
      <section class="sec">
        <div class="sec-head">
          <span class="msr sec-icon" aria-hidden="true">checklist</span>
          <span class="sec-label">Plan</span>
          <span class="sec-meta">{st.plan.progress}</span>
        </div>
        <div class="sec-body">
          {#each st.plan.items as item, i (item.label)}
            <div class="plan-row stagger" style={`--i:${i}`}>
              <span
                class="plan-icon"
                style={`--plan-color: ${item.state === 'done' ? 'var(--success)' : item.state === 'active' ? 'var(--accent)' : 'var(--text-muted)'}`}
                aria-hidden="true"
              >
                <span class="msr">{item.state === 'done' ? 'task_alt' : item.state === 'active' ? 'radio_button_checked' : 'radio_button_unchecked'}</span>
              </span>
              <span class="plan-label" class:done={item.state === 'done'}>{item.label}</span>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- files touched -->
    <section class="sec">
      <div class="sec-head">
        <span class="msr sec-icon" aria-hidden="true">folder_open</span>
        <span class="sec-label">Files touched</span>
        <span class="sec-meta">{st.files.length}</span>
      </div>
      <div class="sec-body">
        {#each st.files as f, i (f.path)}
          <button class="file-row stagger" style={`--i:${i}`} onclick={() => onOpenFile(f.path)}>
            <span class="file-badge" style={`--badge: ${badgeColor[f.kind]}`}>{badgeLetter[f.kind]}</span>
            <span class="file-path" title={f.path}>{f.path}</span>
            {#if hasDiff(f.added, f.removed)}
              <span class="add">+{f.added ?? 0}</span>
              <span class="remove">−{f.removed ?? 0}</span>
            {/if}
          </button>
        {/each}
        {#if st.files.length === 0}
          <div class="empty-hint">No files touched yet</div>
        {/if}
      </div>
    </section>

    <!-- git working tree -->
    <section class="sec">
      <div class="sec-head">
        <span class="msr sec-icon" aria-hidden="true">difference</span>
        <span class="sec-label">Working tree</span>
        <span class="sec-meta">{st.git.branch}</span>
      </div>
      <div class="git-counts">
        <span class="git-add">+{st.git.added}</span>
        <span class="git-del">−{st.git.removed}</span>
        <span class="git-files">{st.git.files} files</span>
      </div>
      <div class="git-rows">
        {#each st.git.rows as g, i (g.path)}
          <div class="git-row stagger" style={`--i:${i}`}>
            <span class="git-mark" style={`--mark: ${g.mark === 'A' ? 'var(--success)' : 'var(--warning)'}`}>{g.mark}</span>
            <span class="git-path">{g.path}</span>
            <span
              class="git-bar"
              style={`--bar: ${g.mark === 'A' ? 'var(--success)' : 'var(--warning)'}; --w: ${Math.round(38 * g.weight)}px; --d: ${i * 70}ms`}
            ></span>
          </div>
        {/each}
      </div>
    </section>

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
    padding: 16px 16px 14px;
    border-bottom: 1px solid var(--border-default);
  }
  .pinned-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 10px;
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
    gap: 16px;
  }
  .ring-wrap {
    position: relative;
    width: 84px;
    height: 84px;
    flex: none;
  }
  .ring-center {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1px;
  }
  .ring-pct {
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 20px;
    color: var(--text-primary);
    line-height: 1;
  }
  /* Over the window — the point where auto-compaction will kick in. */
  .ring-pct.over {
    color: var(--danger);
  }
  .ring-label {
    font-family: var(--font-mono);
    font-size: 9px;
    color: var(--text-muted);
  }
  .legend {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
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
    padding-top: 2px;
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
    margin-top: 13px;
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
  /* Design stagger entrance: rows fade up 45ms apart (sidebar only renders
     when opened, so the animation plays once per open). */
  .stagger {
    animation: rowIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
    animation-delay: calc(var(--i) * 45ms);
  }
  .cost-model {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Verification rows: the harness's own spend — accent-tinted to read as a
     special row distinct from per-model agent usage. */
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
  .cost-meta { color: var(--text-muted); }
  .cost-amt { color: var(--text-secondary); }

  .sections {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .sec {
    padding: 13px 16px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .sec-label {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .sec-head {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-bottom: 9px;
  }
  .sec-icon {
    font-size: 15px;
    line-height: 1;
    color: var(--text-muted);
  }
  .sec-meta {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
  }
  .sec-body {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row-btn {
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
    color: var(--text-primary);
  }
  .row-btn:hover { background: var(--surface-hover); }
  .row-btn.active { background: var(--accent-soft); }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: none;
  }
  .dot.live { animation: dotPulse 1.2s ease-in-out infinite; }
  .row-name {
    flex: 1;
    min-width: 0;
    font-family: var(--font-body);
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-status {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
  }
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
  .git-counts {
    display: flex;
    gap: 12px;
    font-family: var(--font-mono);
    font-size: 11px;
    margin-bottom: 9px;
  }
  .git-add { color: var(--success); }
  .git-del { color: var(--danger); }
  .git-files { color: var(--text-muted); }
  .git-rows {
    display: flex;
    flex-direction: column;
    gap: 4px;
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
  .git-bar {
    width: var(--w);
    height: 4px;
    border-radius: 2px;
    background: var(--bar);
    transition: width 0.7s cubic-bezier(0.22, 0.8, 0.3, 1);
    transition-delay: var(--d, 0ms);
    flex: none;
  }
  .empty-hint {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    padding: 2px 0;
  }
</style>