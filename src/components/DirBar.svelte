<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { getSession, sessions } from '../stores/sessions.svelte'
  import { statsFor } from '../stores/stats.svelte'
  import { updateSession } from '../lib/tauri'

  // Design working-directory bar (`Pragma UI Kit`): folder icon + parent/leaf
  // split path (click → dialog picker), git branch pill, files-touched meta.
  let { sessionId }: { sessionId: string } = $props()

  const session = $derived(getSession(sessionId))
  const workingDir = $derived(session?.workingDir ?? '')
  const st = $derived(statsFor(sessionId))

  const dirParent = $derived.by(() => {
    const i = workingDir.lastIndexOf('/')
    return i > 0 ? workingDir.slice(0, i + 1) : ''
  })
  const dirLeaf = $derived.by(() => {
    const i = workingDir.lastIndexOf('/')
    return i > 0 ? workingDir.slice(i + 1) : workingDir
  })

  // Recent directories: the distinct set of working dirs across sessions,
  // current session's first. Rows carry a hint of which sessions use them.
  const recents = $derived.by(() => {
    const seen = new Set<string>()
    const rows: { path: string; current: boolean; meta: string }[] = []
    const push = (path: string, current: boolean) => {
      if (!path || seen.has(path)) return
      seen.add(path)
      const users = sessions.list.filter((s) => s.workingDir === path)
      const names = users.map((u) => u.name).filter((n) => n && n !== 'new-session')
      const meta = current ? 'current session' : names.length ? names.join(', ') : 'session'
      rows.push({ path, current, meta })
    }
    if (workingDir) push(workingDir, true)
    for (const s of sessions.list) push(s.workingDir ?? '', false)
    return rows
  })

  const dirOptions = $derived(
    recents.filter((d) => !search || d.path.toLowerCase().includes(search.toLowerCase()))
  )

  const changed = $derived(st.files.filter((f) => f.kind !== 'read').length)
  const dirMeta = $derived(`${st.files.length} files touched${changed ? ` · ${changed} changed` : ''}`)

  // Picker dialog lifecycle: open → render with entrance animations; closing
  // flags `exiting` to play dialogOut/fadeOut before unmount.
  let pickerOpen = $state(false)
  let exiting = $state(false)
  let mapped = $state(false) // mounted (post entrance) — powers rowIn staggers
  let search = $state('')
  let choice = $state('')

  function openPicker() {
    choice = workingDir
    search = ''
    pickerOpen = true
    mapped = false
    requestAnimationFrame(() => (mapped = true))
  }
  function closePicker() {
    if (!pickerOpen || exiting) return
    exiting = true
    setTimeout(() => {
      pickerOpen = false
      exiting = false
    }, 240)
  }
  function confirmDir() {
    if (!choice) return
    applyDir(choice)
  }
  async function applyDir(path: string) {
    if (!path) return
    try {
      await updateSession(sessionId, session?.model ?? '', session?.provider ?? '', path)
      sessions.list = sessions.list.map((s) =>
        s.id === sessionId ? { ...s, workingDir: path } : s
      )
    } catch { /* keep previous dir on failure */ }
    closePicker()
  }
  // System file picker — falls back through the same apply path.
  async function browseDir() {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: workingDir || undefined,
    })
    if (!selected || typeof selected !== 'string') return
    await applyDir(selected)
  }

  // ESC closes the dialog; the backdrop click also closes it.
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && pickerOpen) {
      e.preventDefault()
      closePicker()
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="dir-bar">
  <span class="msr dir-ico" aria-hidden="true">folder_open</span>
  <button
    class="dir-btn"
    onclick={pickerOpen ? closePicker : openPicker}
    aria-haspopup="dialog"
    aria-expanded={pickerOpen}
    title={workingDir || 'No working directory set'}
  >
    {#if workingDir}
      <!-- parent + leaf are one flex child so the path reads as a single
           continuous string (no flex gap between muted parent / leaf). -->
      <span class="path"><span class="parent">{dirParent}</span><span class="leaf">{dirLeaf}</span></span>
    {:else}
      <span class="leaf placeholder">Set working directory</span>
    {/if}
    <span class="msr unfold" aria-hidden="true">unfold_more</span>
  </button>
  {#if st.git.branch}
    <span class="branch" title="Git branch">
      <span class="msr" aria-hidden="true">fork_right</span>{st.git.branch}
    </span>
  {/if}
  {#if workingDir}
    <span class="meta" aria-hidden="true">{dirMeta}</span>
  {/if}
</div>

{#if pickerOpen}
  <div class="backdrop" class:out={exiting} onclick={closePicker} onkeydown={(e) => { if (e.key === 'Escape') closePicker() }} role="presentation">
    <div class="dialog" role="dialog" aria-modal="true" aria-label="Select a directory to work on" class:out={exiting} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <div class="dh">
        <span class="msr dh-ico" aria-hidden="true">drive_folder_upload</span>
        <span class="dh-title">Select a directory to work on</span>
        <button class="x" aria-label="Close" onclick={closePicker}><span class="msr" aria-hidden="true">close</span></button>
      </div>
      <div class="dsearch">
        <span class="msr" aria-hidden="true">search</span>
        <input
          type="search"
          placeholder="Filter directories…"
          bind:value={search}
          aria-label="Filter directories"
        />
      </div>
      <div class="dlist">
        {#if dirOptions.length === 0}
          <div class="dlist-empty">No matching directories — try Browse…</div>
        {/if}
        {#each dirOptions as d, i (d.path)}
          <button class="drow" class:picked={d.path === choice} class:staggered={mapped} style={`--i:${i}`} onclick={() => (choice = d.path)}>
            <span class="msr d-ico" class:picked={d.path === choice} aria-hidden="true">{d.current ? 'folder_open' : 'folder'}</span>
            <span class="d-copy">
              <span class="d-path">{d.path}</span>
              <span class="d-meta">{d.meta}</span>
            </span>
            {#if d.path === choice}<span class="msr d-check" aria-hidden="true">check</span>{/if}
          </button>
        {/each}
      </div>
      <div class="df">
        <button class="ghost" onclick={browseDir}>
          <span class="msr" aria-hidden="true">folder_open</span>Browse…
        </button>
        <span class="flex"></span>
        <button class="ghost" onclick={closePicker}>Cancel</button>
        <button class="go" onclick={confirmDir} disabled={!choice}>Open directory</button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Design dir bar: overlay strip under the tab bar, slideDown entrance. */
  .dir-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 16px;
    background: var(--bg-overlay);
    border-bottom: 1px solid var(--border-default);
    flex: none;
    min-width: 0;
    overflow: hidden;
    animation: slideDown 0.36s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  /* No working directory: same CSS as the path bar — identical height,
     same padding. Only the content differs: placeholder instead of path,
     and the meta (needs a dir to mean anything) is left out. */
  .dir-ico {
    font-size: 16px;
    line-height: 1;
    color: var(--text-muted);
    flex: none;
  }
  .dir-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    max-width: min(360px, 55vw);
    overflow: hidden;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    padding: 4px 8px;
    font-family: var(--font-mono);
    font-size: 12px;
    cursor: pointer;
    color: var(--text-primary);
    transition: background 0.2s ease;
    flex-shrink: 1;
  }
  .dir-btn:hover {
    background: var(--surface-hover);
  }
  /* parent + leaf share one flex child: continuous path, leaf keeps weight. */
  .path {
    display: flex;
    align-items: baseline;
    min-width: 0;
    flex: 1 1 auto;
  }
  .parent {
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 0 1 auto;
  }
  .leaf {
    font-weight: 500;
    white-space: nowrap;
    flex: none;
  }
  /* Placeholder — deliberately NOT named .empty (that class belongs to the
     picker dialog's empty-list block and would pad us 14px top/bottom). */
  .leaf.placeholder {
    color: var(--text-muted);
    font-style: italic;
    font-weight: 400;
    padding: 0;
  }
  .unfold {
    font-size: 15px;
    line-height: 1;
    color: var(--text-muted);
    flex: none;
  }
  /* Git branch pill */
  .branch {
    display: flex;
    align-items: center;
    gap: 4px;
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-secondary);
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 11px;
    padding: 2px 9px 2px 7px;
    white-space: nowrap;
    flex: none;
  }
  .branch .msr {
    font-size: 13px;
    line-height: 1;
  }
  .meta {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
  }

  /* Picker dialog — design dialogIn/dialogOut + fadeIn/fadeOut backdrop. */
  .backdrop {
    position: fixed;
    inset: 0;
    background: oklch(0% 0 0 / 0.5);
    backdrop-filter: blur(2px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 60;
    animation: fadeIn 0.2s ease both;
  }
  .backdrop.out {
    animation: fadeOut 0.2s ease both;
  }
  .dialog {
    width: min(520px, 92vw);
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: dialogIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  .dialog.out {
    animation: dialogOut 0.22s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  .dh {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 15px 18px 12px;
    border-bottom: 1px solid var(--border-default);
  }
  .dh-ico {
    font-size: 18px;
    line-height: 1;
    color: var(--accent);
    flex: none;
  }
  .dh-title {
    flex: 1;
    min-width: 0;
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 16px;
    letter-spacing: 0.01em;
    color: var(--text-primary);
  }
  .x {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    padding: 3px;
    border-radius: 3px;
    transition: color 0.18s ease, background 0.18s ease;
  }
  .x:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }
  .x .msr { font-size: 17px; line-height: 1; }
  .dsearch {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 18px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .dsearch .msr {
    font-size: 16px;
    line-height: 1;
    color: var(--text-muted);
    flex: none;
  }
  .dsearch input {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12.5px;
  }
  .dsearch input::placeholder {
    color: var(--text-muted);
  }
  .dlist {
    max-height: 280px;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .drow {
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    padding: 9px 11px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    cursor: pointer;
    text-align: left;
    color: var(--text-primary);
    transition: background 0.2s ease, border-color 0.2s ease;
  }
  .drow:hover {
    background: var(--surface-hover);
  }
  .drow.picked {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  /* rowIn stagger once the dialog mounts (design: 45ms × index) */
  .drow.staggered {
    animation: rowIn 0.3s cubic-bezier(0.22, 0.8, 0.3, 1) both;
    animation-delay: calc(var(--i) * 45ms);
  }
  .d-ico {
    font-size: 18px;
    line-height: 1;
    flex: none;
    color: var(--text-muted);
    transition: color 0.2s ease;
  }
  .d-ico.picked {
    color: var(--accent);
  }
  .d-copy {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .d-path {
    font-family: var(--font-mono);
    font-size: 12.5px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .d-meta {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .d-check {
    font-size: 15px;
    line-height: 1;
    flex: none;
    color: var(--success);
  }
  .dlist-empty {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-muted);
    padding: 14px 8px;
    text-align: center;
  }
  .df {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 18px 14px;
    border-top: 1px solid var(--border-subtle);
  }
  .flex { flex: 1; }
  .ghost {
    display: flex;
    align-items: center;
    gap: 5px;
    background: none;
    border: none;
    color: var(--text-secondary);
    font-family: var(--font-body);
    font-size: 12.5px;
    padding: 7px 10px;
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: color 0.18s ease, background 0.18s ease;
  }
  .ghost:hover {
    color: var(--text-primary);
    background: var(--surface-hover);
  }
  .ghost .msr { font-size: 15px; line-height: 1; }
  .go {
    background: var(--accent);
    color: var(--accent-on);
    border: none;
    border-radius: var(--radius-md);
    padding: 8px 15px;
    font-family: var(--font-heading);
    font-weight: 600;
    font-size: 13px;
    cursor: pointer;
    transition: background 0.2s ease, transform 0.15s ease;
  }
  .go:hover {
    background: var(--accent-hover);
  }
  .go:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>