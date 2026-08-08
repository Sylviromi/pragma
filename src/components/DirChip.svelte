<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { getSession, sessions } from '../stores/sessions.svelte'
  import { updateSession } from '../lib/tauri'

  let { sessionId }: { sessionId: string } = $props()

  const session = $derived(getSession(sessionId))
  const workingDir = $derived(session?.workingDir ?? '')
  const displayDir = $derived(workingDir || 'Set working dir')

  async function pickDirectory() {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: workingDir || undefined
    })
    if (!selected || typeof selected !== 'string') return
    await updateSession(sessionId, session?.model ?? '', session?.provider ?? '', selected)
    sessions.list = sessions.list.map((s) =>
      s.id === sessionId ? { ...s, workingDir: selected } : s
    )
  }
</script>

<button
  class="dir-chip"
  class:empty={!workingDir}
  onclick={pickDirectory}
  title={workingDir
    ? `Working directory: ${workingDir}\nClick to change`
    : 'No working directory set — click to choose one'}
  aria-label="Working directory"
>
  <svg
    class="icon"
    viewBox="0 0 16 16"
    width="12"
    height="12"
    fill="none"
    stroke="currentColor"
    stroke-width="1.4"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path
      d="M1.5 5.25A1.75 1.75 0 0 1 3.25 3.5h2.1a1.75 1.75 0 0 1 1.4.7l.6.8h5.4a1.75 1.75 0 0 1 1.75 1.75v4.3A1.75 1.75 0 0 1 12.75 12.5H3.25A1.75 1.75 0 0 1 1.5 10.75v-5.5Z"
    />
  </svg>
  <span class="path">{displayDir}</span>
</button>

<style>
  .dir-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 240px;
    min-width: 0;
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border-default);
    border-radius: var(--radius-md);
    padding: 3px var(--spacing-2);
    color: var(--color-text-secondary);
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    cursor: pointer;
    white-space: nowrap;
    transition:
      border-color 120ms ease,
      color 120ms ease;
  }
  .dir-chip:hover {
    border-color: var(--color-border-focus);
    color: var(--color-text-primary);
  }
  .dir-chip:focus-visible {
    outline: 2px solid var(--color-border-focus);
    outline-offset: 1px;
  }
  .icon {
    flex-shrink: 0;
    color: var(--color-accent-brand);
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dir-chip.empty .path {
    color: var(--color-text-muted);
    font-style: italic;
  }
</style>
