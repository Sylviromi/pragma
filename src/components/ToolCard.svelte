<script lang="ts">
  import { approveTool, denyTool } from '../stores/chat.svelte'
  import type { PendingTool, ToolResult } from '../stores/chat.svelte'

  let {
    sessionId,
    tool,
    result,
    autoApprove
  }: {
    sessionId: string
    tool: PendingTool
    result?: ToolResult
    autoApprove: boolean
  } = $props()

  const statusColor = $derived(
    result
      ? result.isError
        ? 'var(--color-accent-danger)'
        : 'var(--color-accent-success)'
      : 'var(--color-accent-warning)'
  )
</script>

<div class="tool-card">
  <div class="tool-header">
    <span class="dot" style="background:{statusColor}" aria-hidden="true"></span>
    <span class="tool-name">{tool.name}</span>
    {#if !result && !autoApprove}
      <button
        class="approve-btn"
        onclick={() => approveTool(sessionId, tool.toolUseId)}
        aria-label="Approve {tool.name}"
      >Approve</button>
      <button
        class="deny-btn"
        onclick={() => denyTool(sessionId, tool.toolUseId)}
        aria-label="Deny {tool.name}"
      >Deny</button>
    {:else if !result}
      <span class="status-text">running…</span>
    {:else}
      <span class="status-text">{result.isError ? 'error' : 'done'}</span>
    {/if}
  </div>

  <pre class="tool-input">{JSON.stringify(tool.input, null, 2)}</pre>

  {#if result}
    <pre class="tool-output" class:error={result.isError}>{result.output}</pre>
  {/if}
</div>

<style>
  .tool-card {
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-md);
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    margin: var(--spacing-2) 0;
  }
  .tool-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    background: var(--color-bg-elevated);
    border-bottom: 1px solid var(--color-border-subtle);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .tool-name {
    flex: 1;
    font-weight: 600;
    color: var(--color-text-primary);
  }
  .status-text {
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
  }
  .approve-btn {
    background: var(--color-accent-brand);
    color: white;
    border: none;
    border-radius: var(--radius-sm);
    padding: 2px 10px;
  }
  .deny-btn {
    background: transparent;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-sm);
    padding: 2px 10px;
  }
  .tool-input {
    margin: 0;
    padding: var(--spacing-2) var(--spacing-3);
    color: var(--color-text-secondary);
    background: var(--color-bg-base);
    overflow-x: auto;
    white-space: pre-wrap;
  }
  .tool-output {
    margin: 0;
    padding: var(--spacing-2) var(--spacing-3);
    color: var(--color-text-secondary);
    background: var(--color-bg-base);
    border-top: 1px solid var(--color-border-subtle);
    overflow-x: auto;
    white-space: pre-wrap;
    max-height: 200px;
    overflow-y: auto;
  }
  .tool-output.error {
    color: var(--color-accent-danger);
  }
</style>