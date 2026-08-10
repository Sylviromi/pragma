<script lang="ts">
  import { approveTool, denyTool, alwaysApproveTool } from '../stores/chat.svelte'
  import Icon from './Icon.svelte'
  import type { ToolItem } from '../stores/chat.svelte'

  let {
    sessionId,
    tool,
  }: {
    sessionId: string
    tool: ToolItem
  } = $props()

  let outputOpen = $state(false)

  // Auto-open output when tool completes so the result is immediately visible
  $effect(() => {
    if ((tool.status === 'done' || tool.status === 'error') && tool.output) {
      outputOpen = true
    }
  })

  // Design tool icons — read/edit/run/search/grep/write families map to their
  // material glyphs, everything else falls back to a neutral build icon.
  function iconFor(name: string): string {
    switch (true) {
      case name.startsWith('read'): return 'description'
      case name.startsWith('write'): return 'note_add'
      case name.startsWith('patch') || name.startsWith('edit'): return 'edit_document'
      case name.startsWith('create') || name.startsWith('delete') || name.startsWith('move'): return 'note_add'
      case name.startsWith('run'): return 'terminal'
      case name.startsWith('list') || name.startsWith('search'): return 'travel_explore'
      case name.startsWith('git'): return 'difference'
      case name.startsWith('web'): return 'travel_explore'
      case name.startsWith('fetch'): return 'travel_explore'
      default: return 'build'
    }
  }

  // Human-readable one-liner for the tool input
  function summarize(name: string, input: Record<string, unknown>): string {
    const s = (k: string) => String(input[k] ?? '')
    switch (name) {
      case 'run_command':    return `$ ${s('command')}`
      case 'read_file':      return s('path')
      case 'write_file':     return s('path')
      case 'patch_file':     return s('path')
      case 'list_files':     return s('path') || '.'
      case 'search_files':   return `"${s('pattern')}" in ${s('path') || '.'}`
      case 'create_directory': return s('path')
      case 'delete_file':    return s('path')
      case 'move_file':      return `${s('src')} → ${s('dest')}`
      case 'git_add':        return Array.isArray(input.paths) ? (input.paths as string[]).join(' ') : (s('paths') || '.')
      case 'git_commit':     return s('message')
      case 'git_diff':       return s('path') || 'staged'
      case 'git_log':        return s('path') || ''
      case 'git_status':     return ''
      case 'web_search':     return s('query')
      case 'fetch_url':      return s('url')
      default: {
        const first = Object.entries(input)[0]
        return first ? String(first[1]) : ''
      }
    }
  }

  const summary = $derived(summarize(tool.name, tool.input))
  const hasOutput = $derived(tool.output !== undefined && tool.output !== '')
  const outputTrimmed = $derived((tool.output ?? '').slice(0, 4000))
  const running = $derived(tool.status === 'running')
  const waiting = $derived(tool.status === 'waiting')

  // Diff-ish coloring per output line (design: + green / − red / failures red).
  const outputLines = $derived((tool.output ?? '').trimEnd().split('\n').map((text) => {
    const add = text.startsWith('+'), rm = text.startsWith('-'), fail = /FAIL|●|Expected:|error:/i.test(text)
    return { text, color: add ? 'var(--success)' : rm ? 'var(--danger)' : fail ? 'var(--danger)' : 'var(--text-secondary)' }
  }))
</script>

<div
  class="tool-card"
  class:waiting
  class:error={tool.status === 'error'}
  class:open={outputOpen}
>
  <div class="header">
    <span class="t-icon" aria-hidden="true"><Icon name={iconFor(tool.name) as never} size={15} /></span>
    <span class="label">{tool.name}</span>
    {#if summary}<span class="detail">{summary}</span>{/if}

    <span class="spacer"></span>

    {#if waiting}
      <button class="btn primary" onclick={() => approveTool(sessionId, tool.toolUseId)}>Run</button>
      <button class="btn ghost" onclick={() => { alwaysApproveTool(sessionId, tool.name); approveTool(sessionId, tool.toolUseId) }}>Always</button>
      <button class="btn danger" onclick={() => denyTool(sessionId, tool.toolUseId)}>Deny</button>
    {:else if running}
      <span class="status spin" aria-hidden="true"><Icon name="progress_activity" size={14} /></span>
    {:else if tool.status === 'done' || tool.status === 'error'}
      <span class="status" class:ok={tool.status === 'done'} aria-hidden="true"><Icon name="check" size={14} fill={tool.status === 'done'} /></span>
    {/if}

    {#if hasOutput && !waiting}
      <button class="chev" class:open={outputOpen} onclick={() => outputOpen = !outputOpen} aria-label="Toggle output" aria-expanded={outputOpen}>
        <Icon name="expand_more" size={15} />
      </button>
    {/if}
  </div>

  {#if hasOutput && outputOpen}
    <div class="output">
      {#each outputLines as line (line.text + line.color)}
        <div class="line" style={`color:${line.color}`}>{line.text}</div>
      {/each}
      {#if (tool.output ?? '').length > 4000}<div class="more">… (truncated)</div>{/if}
    </div>
  {/if}
</div>

<style>
  /* Design: tool call strip — elevated card, 2px left border colored by state,
     icon + mono label + detail + status glyph + chevron, expandable output. */
  .tool-card {
    display: flex;
    flex-direction: column;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-left: 2px solid var(--border-default);
    border-radius: var(--radius-sm);
    overflow: hidden;
    margin: 1px 0;
    transition: border-color 0.25s ease;
    max-width: 100%;
  }
  .tool-card.open {
    border-left-color: var(--accent);
  }
  .tool-card.waiting {
    border-left-color: var(--warning);
  }
  .tool-card.error {
    border-left-color: var(--danger);
  }
  .header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 11px;
    min-height: 30px;
  }
  .t-icon { color: var(--text-muted); flex: none; display: flex; }
  .label {
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .detail {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-muted);
    flex: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 180px;
  }
  .spacer { flex: 1; }
  .status { color: var(--success); display: flex; flex: none; }
  .status.ok { color: var(--success); }
  .status.spin {
    color: var(--warning);
    display: flex;
    animation: spin 1s linear infinite;
  }
  .chev {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0;
    display: flex;
    flex: none;
  }
  .chev:hover { color: var(--text-primary); }
  .chev.open { transform: rotate(180deg); }
  .btn {
    font-family: var(--font-body);
    font-size: 11.5px;
    border-radius: 2px;
    cursor: pointer;
    padding: 2px 10px;
    flex: none;
  }
  .btn.primary { background: var(--accent); color: var(--accent-on); border: none; }
  .btn.ghost { background: transparent; color: var(--text-secondary); border: 1px solid var(--border-default); }
  .btn.danger { background: transparent; color: var(--danger); border: 1px solid currentColor; }
  .btn:hover { filter: brightness(1.1); }
  .output {
    border-top: 1px solid var(--border-subtle);
    background: var(--bg-base);
    padding: 9px 12px;
    max-height: 200px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: outputIn 0.28s cubic-bezier(0.22, 0.8, 0.3, 1) both;
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