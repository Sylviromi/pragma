<script lang="ts">
  import { sessions, openTab, closeTab, newSession } from '../stores/sessions.svelte'
  import { chat, STATUS_META, type AgentStatus } from '../stores/chat.svelte'
  import { ui } from '../stores/ui.svelte'
  import { sessionTint } from '../lib/tint'

  // Design session rail (`Pragma UI Kit v2.dc.html`): sessions grouped by
  // working directory, tinted per-session accents, "New session" on top.
  // Width toggles like the right sidebar (224px / 200px narrow + slide).

  const NARROW = typeof window !== 'undefined' ? window.innerWidth < 900 : false
  let narrow = $state(NARROW)
  $effect(() => {
    const onResize = () => (narrow = window.innerWidth < 900)
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  })

  const open = $derived(ui.railOpen)
  const railW = $derived(narrow ? 200 : 224)

  // Compact relative age ("2m", "1h", "yest", "3d") for idle sessions —
  // matches the rail's mono meta column.
  function relTime(iso: string): string {
    if (!iso) return ''
    const t = new Date(iso).getTime()
    if (!Number.isFinite(t)) return ''
    const mins = Math.max(0, Math.round((Date.now() - t) / 60000))
    if (mins < 1) return 'now'
    if (mins < 60) return `${mins}m`
    const hrs = Math.round(mins / 60)
    if (hrs < 24) return `${hrs}h`
    const days = Math.round(hrs / 24)
    if (days === 1) return 'yest'
    if (days < 7) return `${days}d`
    return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
  }

  // Group open sessions by working dir (design `railGroups`): active dir first
  // with an open-folder icon, then the rest in insertion order.
  const groups = $derived.by(() => {
    const order: string[] = []
    const byDir = new Map<string, { id: string; name: string; dir: string }[]>()
    for (const id of sessions.openTabs) {
      const s = sessions.list.find((x) => x.id === id)
      if (!s) continue
      const dir = s.workingDir || ''
      if (!byDir.has(dir)) {
        byDir.set(dir, [])
        order.push(dir)
      }
      byDir.get(dir)!.push({ id, name: s.name, dir })
    }
    return order.map((dir) => ({
      dir,
      count: byDir.get(dir)!.length,
      sessions: byDir.get(dir)!.map((s) => {
        const status = (chat.status[s.id] ?? 'idle') as AgentStatus
        const meta = STATUS_META[status]
        const active = sessions.activeTabId === s.id
        const tint = sessionTint(s.id)
        return {
          ...s,
          status,
          live: status !== 'idle',
          meta: status === 'idle' ? relTime(sessions.list.find((x) => x.id === s.id)?.createdAt ?? '') : meta.label,
          active,
          tint,
          dot: status === 'idle' ? tint : meta.color,
        }
      }),
    }))
  })
</script>

<aside class="rail" class:closed={!open} style={`--rail-w:${railW}px`} aria-label="Sessions">
  <div class="rail-inner" style={`width:${railW}px`}>
    <div class="rail-top">
      <button class="new-session" onclick={() => newSession()}>
        <span class="msr" aria-hidden="true">add</span>New session
      </button>
    </div>

    <div class="rail-scroll">
      {#each groups as g, gi (g.dir)}
        <div class="group stagger" style={`--i:${gi}`}>
          <div class="group-head">
            <span class="msr g-icon" class:active={!g.dir} aria-hidden="true">{g.dir ? 'folder' : 'folder_open'}</span>
            <span class="g-dir" title={g.dir || 'No working directory'}>{g.dir || 'no directory'}</span>
            <span class="g-count">{g.count}</span>
          </div>
          <div class="g-rows">
            {#each g.sessions as row (row.id)}
              <div
                class="s-row"
                class:active={row.active}
                style={`--tint:${row.tint}`}
                role="button"
                tabindex="0"
                aria-current={row.active ? 'page' : undefined}
                onclick={() => openTab(row.id)}
                onkeydown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    openTab(row.id)
                  }
                }}
              >
                <span class="s-dot" class:live={row.live} style={`background:${row.dot}`} aria-hidden="true"></span>
                <span class="s-name" title={row.name}>{row.name}</span>
                <span class="s-meta">{row.meta}</span>
                <button
                  class="s-close"
                  aria-label={`Close ${row.name}`}
                  title="Close session"
                  onclick={(e) => {
                    e.stopPropagation()
                    closeTab(row.id)
                  }}
                ><span class="msr" aria-hidden="true">close</span></button>
              </div>
            {/each}
          </div>
        </div>
      {/each}

      {#if groups.length === 0}
        <div class="rail-empty">No open sessions — start one above.</div>
      {/if}
    </div>
  </div>
</aside>

<style>
  /* Design rail: width 224/200px, slides closed with inner translateX.
     Border-width uses step-start on open (appears immediately) and step-end
     on close (stays until the end) — per the design's transition spec. */
  .rail {
    width: var(--rail-w);
    flex: none;
    min-width: 0;
    overflow: hidden;
    border-right: 1px solid var(--border-default);
    background: var(--bg-elevated);
    display: flex;
    flex-direction: column;
    min-height: 0;
    transition: width 0.34s cubic-bezier(0.22, 0.8, 0.3, 1), border-right-width 0.34s step-start;
  }
  .rail.closed {
    width: 0;
    border-right-width: 0;
    pointer-events: none;
    transition: width 0.34s cubic-bezier(0.22, 0.8, 0.3, 1), border-right-width 0.34s step-end;
  }
  .rail-inner {
    flex: none;
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    transition: transform 0.34s cubic-bezier(0.22, 0.8, 0.3, 1);
  }
  .rail.closed .rail-inner {
    transform: translateX(-16px);
  }
  .rail-top {
    flex: none;
    padding: 11px 11px 9px;
    border-bottom: 1px solid var(--border-subtle);
  }
  /* "New session" — full-width bordered button (design newSessionStyle). */
  .new-session {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    padding: 7px 10px;
    background: none;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-family: var(--font-body);
    font-size: 12.5px;
    cursor: pointer;
    transition: border-color 0.2s ease, color 0.2s ease, background 0.2s ease;
  }
  .new-session:hover {
    border-color: var(--accent);
    color: var(--text-primary);
    background: var(--surface-hover);
  }
  .new-session .msr {
    font-size: 16px;
    line-height: 1;
  }
  .rail-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .group {
    display: flex;
    flex-direction: column;
  }
  .group-head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 5px 5px;
  }
  .g-icon {
    font-size: 14px;
    line-height: 1;
    flex: none;
    color: var(--text-muted);
  }
  .g-icon.active {
    color: var(--accent);
  }
  .g-dir {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .g-count {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
  }
  .g-rows {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  /* Session row: tinted left border + dot, tint-tinted active background. */
  .s-row {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 5px 7px;
    border-radius: var(--radius-sm);
    border-left: 2px solid color-mix(in oklch, var(--tint) 40%, transparent);
    cursor: pointer;
    color: var(--text-primary);
    transition: background 0.2s ease, border-color 0.2s ease;
  }
  .s-row:hover {
    background: var(--surface-hover);
  }
  .s-row.active {
    border-left-color: var(--tint);
    background: color-mix(in oklch, var(--tint) 15%, transparent);
  }
  .s-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex: none;
  }
  .s-dot.live {
    animation: cursorblink 1.2s ease-in-out infinite;
  }
  .s-name {
    flex: 1;
    min-width: 0;
    font-family: var(--font-body);
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .s-meta {
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--text-muted);
    white-space: nowrap;
    flex: none;
  }
  .s-close {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 1px;
    display: flex;
    border-radius: 2px;
    opacity: 0.45;
    transition: opacity 0.2s ease, color 0.2s ease;
  }
  .s-close:hover {
    opacity: 1;
    color: var(--text-primary);
  }
  .s-row.active .s-close {
    opacity: 0.9;
  }
  .s-close .msr {
    font-size: 14px;
    line-height: 1;
  }
  .rail-empty {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    padding: 10px 6px;
    text-align: center;
  }
  /* Design stagger entrance on first open. */
  .stagger {
    animation: rowIn 0.32s cubic-bezier(0.22, 0.8, 0.3, 1) both;
    animation-delay: calc(var(--i) * 45ms);
  }
</style>
