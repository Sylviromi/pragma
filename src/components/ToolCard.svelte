<script lang="ts">
  import { approveTool, denyTool, alwaysApproveTool } from '../stores/chat.svelte'
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
</script>

<div class="tool-card" class:waiting={tool.status === 'waiting'} class:error={tool.status === 'error'}>
  <div class="header">
    <!-- status icon -->
    {#if tool.status === 'waiting'}
      <span class="icon warn" aria-hidden="true">⚡</span>
    {:else if tool.status === 'running'}
      <span class="icon spin" aria-hidden="true">◌</span>
    {:else if tool.status === 'done'}
      <span class="icon ok" aria-hidden="true">✓</span>
    {:else}
      <span class="icon err" aria-hidden="true">✗</span>
    {/if}

    <span class="name">{tool.name}</span>
    {#if summary}<span class="summary">{summary}</span>{/if}

    <span class="spacer"></span>

    {#if tool.status === 'waiting'}
      <button class="btn primary" onclick={() => approveTool(sessionId, tool.toolUseId)}>Run</button>
      <button class="btn ghost" onclick={() => { alwaysApproveTool(sessionId, tool.name); approveTool(sessionId, tool.toolUseId) }}>Always</button>
      <button class="btn danger" onclick={() => denyTool(sessionId, tool.toolUseId)}>Deny</button>
    {:else if tool.status === 'running'}
      <span class="muted">running…</span>
    {:else if hasOutput}
      <button class="toggle" onclick={() => outputOpen = !outputOpen} aria-label="Toggle output">
        {outputOpen ? '▾' : '▸'}
      </button>
    {/if}
  </div>

  {#if hasOutput && outputOpen}
    <pre class="output" class:err-output={tool.isError}>{outputTrimmed}{(tool.output ?? '').length > 4000 ? '\n… (truncated)' : ''}</pre>
  {/if}
</div>

<style>
  .tool-card {
    border: 1px solid var(--color-border-subtle);
    border-radius: var(--radius-md);
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    margin: var(--spacing-1) 0;
    max-width: 70%;
    align-self: flex-start;
  }
  .tool-card.waiting {
    border-color: color-mix(in srgb, var(--color-accent-warning, #d29922) 40%, transparent);
  }
  .tool-card.error {
    border-color: color-mix(in srgb, var(--color-accent-error, #f85149) 40%, transparent);
  }
  .header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 4px var(--spacing-3);
    background: var(--color-bg-elevated);
    min-height: 30px;
  }
  .icon { font-size: 13px; flex-shrink: 0; }
  .icon.ok   { color: var(--color-accent-success, #3fb950); }
  .icon.err  { color: var(--color-accent-error,   #f85149); }
  .icon.warn { color: var(--color-accent-warning,  #d29922); }
  .icon.spin { color: var(--color-text-muted); animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .name {
    font-weight: 600;
    color: var(--color-text-primary);
    flex-shrink: 0;
  }
  .summary {
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 340px;
  }
  .spacer { flex: 1; }
  .muted { color: var(--color-text-muted); font-size: var(--font-size-xs); }

  .btn {
    border-radius: var(--radius-sm);
    font: inherit;
    cursor: pointer;
    padding: 2px 10px;
    flex-shrink: 0;
  }
  .btn.primary { background: var(--color-accent-brand); color: #fff; border: none; }
  .btn.ghost   { background: transparent; color: var(--color-text-secondary); border: 1px solid var(--color-border-default); }
  .btn.danger  { background: transparent; color: var(--color-accent-error, #f85149); border: 1px solid currentColor; }
  .btn:hover   { filter: brightness(1.1); }

  .toggle {
    background: transparent;
    border: none;
    color: var(--color-text-muted);
    cursor: pointer;
    font-size: 15px;
    padding: 0 2px;
    line-height: 1;
  }
  .toggle:hover { color: var(--color-text-primary); }

  .output {
    margin: 0;
    padding: var(--spacing-2) var(--spacing-3);
    color: var(--color-text-secondary);
    background: var(--color-bg-base);
    border-top: 1px solid var(--color-border-subtle);
    overflow-x: auto;
    white-space: pre-wrap;
    max-height: 240px;
    overflow-y: auto;
    font-size: var(--font-size-xs);
  }
  .output.err-output { color: var(--color-accent-error, #f85149); }
</style>
