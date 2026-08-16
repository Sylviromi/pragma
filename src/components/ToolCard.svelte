<script lang="ts">
  import { approveTool, denyTool, alwaysApproveTool } from '../stores/chat.svelte';
  import Icon from './Icon.svelte';
  import Collapsible from './Collapsible.svelte';
  import Button from './Button.svelte';
  import OutputLog from './OutputLog.svelte';
  import { toolIcon } from '../lib/toolIcons';
  import type { ToolItem } from '../stores/chat.svelte';

  let {
    sessionId,
    tool,
  }: {
    sessionId: string;
    tool: ToolItem;
  } = $props();

  let outputOpen = $state(false);

  // Auto-open output when tool completes so the result is immediately visible
  $effect(() => {
    if ((tool.status === 'done' || tool.status === 'error') && tool.output) {
      outputOpen = true;
    }
  });

  // Human-readable one-liner for the tool input
  function summarize(name: string, input: Record<string, unknown>): string {
    const s = (k: string) => String(input[k] ?? '');
    switch (name) {
      case 'run_command':
        return `$ ${s('command')}`;
      case 'read_file':
        return s('path');
      case 'write_file':
        return s('path');
      case 'edit_file':
        return `${s('start_line')}..${s('end_line')} ${s('path')}`;
      case 'list_files':
        return s('path') || '.';
      case 'search_files':
        return `"${s('pattern')}" in ${s('path') || '.'}`;
      case 'delete_file':
        return s('path');
      case 'git_add':
        return Array.isArray(input.paths) ? (input.paths as string[]).join(' ') : s('paths') || '.';
      case 'git_commit':
        return s('message');
      case 'git_diff':
        return s('path') || 'staged';
      case 'git_log':
        return s('path') || '';
      case 'git_status':
        return '';
      case 'web_search':
        return s('query');
      case 'fetch_url':
        return s('url');
      default: {
        const first = Object.entries(input)[0];
        return first ? String(first[1]) : '';
      }
    }
  }

  const summary = $derived(summarize(tool.name, tool.input));
  const hasOutput = $derived(tool.output !== undefined && tool.output !== '');
  const running = $derived(tool.status === 'running');
  const waiting = $derived(tool.status === 'waiting');
  const icon = $derived(toolIcon(tool.name) as never);
  // Root border-left color by state (design: default → accent open → warning
  // waiting → danger error). `open` is added by Collapsible itself.
  const rootClass = $derived(
    `tool-card${waiting ? ' waiting' : ''}${tool.status === 'error' ? ' error' : ''}`,
  );

  // Diff-ish coloring per output line (design: + green / − red / failures red).
  function colorFor(line: string): string | undefined {
    const add = line.startsWith('+');
    const rm = line.startsWith('-');
    const fail = /FAIL|●|Expected:|error:/i.test(line);
    if (add) return 'var(--success)';
    if (rm || fail) return 'var(--danger)';
    return undefined;
  }
</script>

<Collapsible
  bind:open={outputOpen}
  clickable={false}
  dense
  chevron={hasOutput && !waiting}
  label="Toggle output"
  class={rootClass}
>
  {#snippet header()}
    <span class="t-icon" aria-hidden="true"><Icon name={icon} size={15} /></span>
    <span class="label">{tool.name}</span>
    {#if summary}<span class="detail">{summary}</span>{/if}
  {/snippet}

  {#snippet actions()}
    {#if waiting}
      <Button size="xs" variant="primary" onclick={() => approveTool(sessionId, tool.toolUseId)}
        >Run</Button
      >
      <Button
        size="xs"
        variant="ghost"
        onclick={() => {
          alwaysApproveTool(sessionId, tool.name);
          approveTool(sessionId, tool.toolUseId);
        }}>Always</Button
      >
      <Button size="xs" variant="danger" onclick={() => denyTool(sessionId, tool.toolUseId)}
        >Deny</Button
      >
    {:else if running}
      <span class="status spin" aria-hidden="true"><Icon name="progress_activity" size={14} /></span
      >
    {:else if tool.status === 'done' || tool.status === 'error'}
      <span class="status" class:ok={tool.status === 'done'} aria-hidden="true"
        ><Icon name="check" size={14} fill={tool.status === 'done'} /></span
      >
    {/if}
  {/snippet}

  {#if hasOutput}
    <div class="output">
      <OutputLog lines={tool.output?.split('\n') ?? []} {colorFor} maxHeight={200} />
    </div>
  {/if}
</Collapsible>

<style>
  /* Root card theme — class lands on Collapsible's root. */
  :global(.tool-card) {
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-left: 2px solid var(--border-default);
    margin: 1px 0;
    transition: border-color 0.25s ease;
    max-width: 100%;
  }
  :global(.tool-card.open) {
    border-left-color: var(--accent);
  }
  :global(.tool-card.waiting) {
    border-left-color: var(--warning);
  }
  :global(.tool-card.error) {
    border-left-color: var(--danger);
  }
  .t-icon {
    color: var(--text-muted);
    flex: none;
    display: flex;
  }
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
  .status {
    color: var(--success);
    display: flex;
    flex: none;
  }
  .status.spin {
    color: var(--warning);
    animation: spin 1s linear infinite;
  }
  .output {
    border-top: 1px solid var(--border-subtle);
    background: var(--bg-base);
    padding: 9px 12px;
  }
</style>
