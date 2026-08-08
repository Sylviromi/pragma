<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { getSession } from '../stores/sessions.svelte'
  import { sessions } from '../stores/sessions.svelte'
  import { updateSession } from '../lib/tauri'
  import SlotStack from './SlotStack.svelte'

  let { sessionId }: { sessionId: string } = $props()

  const session = $derived(getSession(sessionId))
  const workingDir = $derived(session?.workingDir ?? '')
  const displayDir = $derived(workingDir || '(no directory set)')

  async function pickDirectory() {
    const selected = await open({ directory: true, multiple: false, defaultPath: workingDir || undefined })
    if (!selected || typeof selected !== 'string') return
    await updateSession(sessionId, session?.model ?? '', session?.provider ?? '', selected)
    sessions.list = sessions.list.map(s =>
      s.id === sessionId ? { ...s, workingDir: selected } : s
    )
  }
</script>

<div class="status-bar" data-session-id={sessionId}>
  <button class="dir-btn" onclick={pickDirectory} title="Click to change working directory">
    <span class="dir-icon">⌂</span>
    <span class="dir-path">{displayDir}</span>
  </button>
  <SlotStack slot="chat-toolbar-extra" context={sessionId} />
</div>

<style>
  .status-bar {
    border-bottom: 1px solid var(--color-border-subtle);
    font-size: var(--font-size-sm);
    font-family: var(--font-mono);
    display: flex;
    align-items: center;
  }
  .dir-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    background: transparent;
    border: none;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: var(--font-size-sm);
    padding: var(--spacing-1) var(--spacing-3);
    cursor: pointer;
    min-width: 0;
    overflow: hidden;
  }
  .dir-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-overlay);
  }
  .dir-icon {
    flex-shrink: 0;
    font-style: normal;
  }
  .dir-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
