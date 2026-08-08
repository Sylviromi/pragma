import {
  runAgenticLoop,
  listenAgentEvents,
  getRunningSessionIds,
  getPendingApprovals,
  listMessages as fetchMessages,
  approveTool as apiApprove,
  denyTool as apiDeny,
  interruptSession,
  revertSession
} from '../lib/tauri'
import type { AgentEvent, Message, RevertSummary } from '../lib/types'
import { sessions } from './sessions.svelte'
import { notifyOS } from '../lib/notify'
import { pushToast } from './toast.svelte'
// Unlisten functions keyed by sessionId — outlive component HMR reloads.
const _listeners = new Map<string, () => void>()

// --- sessionStorage persistence for live turn state ---
// Survives Vite HMR module reloads so in-progress turns aren't wiped.

function saveLiveState(sessionId: string) {
  try {
    sessionStorage.setItem(`pragma:live:${sessionId}`, JSON.stringify({
      liveBlocks:   chat.liveBlocks[sessionId]   ?? [],
      liveReasoning: chat.liveReasoning[sessionId] ?? '',
      toolItems:    chat.toolItems[sessionId]     ?? [],
      autoApprove:  chat.autoApprove[sessionId]  ?? false,
      alwaysApprove: [...(chat.alwaysApprove[sessionId] ?? new Set())],
    }))
  } catch { /* storage quota — ignore */ }
}

function restoreLiveState(sessionId: string) {
  try {
    const raw = sessionStorage.getItem(`pragma:live:${sessionId}`)
    if (!raw) return
    const s = JSON.parse(raw)
    chat.liveBlocks[sessionId]  = s.liveBlocks ?? []
    chat.liveReasoning[sessionId] = s.liveReasoning ?? ''
    chat.toolItems[sessionId]   = s.toolItems  ?? []
    chat.autoApprove[sessionId] = s.autoApprove ?? false
    if (Array.isArray(s.alwaysApprove)) {
      chat.alwaysApprove[sessionId] = new Set(s.alwaysApprove)
    }
  } catch { /* corrupt entry — ignore */ }
}

function clearLiveState(sessionId: string) {
  sessionStorage.removeItem(`pragma:live:${sessionId}`)
}

// Clean up all Tauri event listeners before Vite replaces this module.
// Without this, each HMR cycle adds a new orphaned listener on top of old ones.
if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    for (const [, unlisten] of _listeners) unlisten()
    _listeners.clear()
  })
}

export interface ToolItem {
  toolUseId: string
  name: string
  input: Record<string, unknown>
  output?: string
  isError?: boolean
  /** waiting = needs user approval, running = approved/executing, done/error = finished */
  status: 'waiting' | 'running' | 'done' | 'error'
}

/** A block in the live (in-progress) assistant turn — ordered text segments and tool references */
export type LiveBlock =
  | { type: 'text'; text: string }
  | { type: 'tool'; toolUseId: string }

// Read-only tools: auto-approve without asking
const SAFE_TOOLS = new Set([
  'read_file', 'list_files', 'search_files',
  'git_status', 'git_diff', 'git_log',
  'web_search', 'fetch_url',
  'load_skill',
])

export const chat = $state({
  messages:       {} as Record<string, Message[]>,
  streamingToken: {} as Record<string, string | null>,
  liveBlocks:     {} as Record<string, LiveBlock[]>,
  /** Streaming reasoning/thinking text for the in-progress turn (DeepSeek
   *  reasoning_content, Anthropic thinking_delta). Surfaced like other harnesses. */
  liveReasoning:  {} as Record<string, string>,
  toolItems:      {} as Record<string, ToolItem[]>,
  running:        {} as Record<string, boolean>,
  error:          {} as Record<string, string | null>,
  /** Neutral notice (e.g. "Turn interrupted by user") — rendered like error, less alarming. */
  notice:         {} as Record<string, string | null>,
  alwaysApprove:  {} as Record<string, Set<string>>,
  autoApprove:    {} as Record<string, boolean>,
})

export type AgentState = 'idle' | 'working' | 'waiting' | 'error'

/** Aggregate agent state for a session — shown in the top bar.
 *  `waiting` (a tool paused on an approval question) outranks `working`,
 *  a terminal error lingers as `error` until the next turn starts. */
export function agentState(sessionId: string): AgentState {
  if ((chat.toolItems[sessionId] ?? []).some((t) => t.status === 'waiting')) return 'waiting'
  if (chat.running[sessionId]) return 'working'
  if (chat.error[sessionId]) return 'error'
  return 'idle'
}

/** Turn-completion notifications: in-app toast + native OS notification.
 *  notifyOS stays quiet while the window is focused, so the toast covers the
 *  case where the app is open but you're on another tab. */
function notifyTurn(sessionId: string, kind: 'done' | 'error' | 'waiting', detail?: string) {
  const name = sessions.list.find((s) => s.id === sessionId)?.name ?? 'Session'
  const trimmed = detail ? (detail.length > 200 ? detail.slice(0, 197) + '…' : detail) : ''
  if (kind === 'error') {
    pushToast(`Error — ${name}: ${trimmed}`, 'error')
    notifyOS('Pragma — error', `${name}: ${trimmed}`, true)
  } else if (kind === 'waiting') {
    pushToast(`${name} — needs approval: ${trimmed || 'a tool is waiting for review'}`, 'info')
    notifyOS('Pragma — needs approval', `${name}: ${trimmed || 'a tool is waiting for review'}`)
  } else {
    pushToast(`Done — ${name}`, 'success')
    notifyOS('Pragma — done', `${name} finished its turn.`)
  }
}

/** Re-attach to any agentic loops that are still running on the Rust side.
 *  Call this on app startup / after HMR reloads. */
export async function reconnectRunningSessions() {
  const ids = await getRunningSessionIds()
  if (ids.length === 0) return

  // Fetch which tools are actually still waiting for approval in the backend.
  // Any tool the frontend thinks is "waiting"/"running" that isn't in this set
  // was already handled before HMR fired and the result event was missed.
  const pendingIds = new Set(await getPendingApprovals())

  for (const sessionId of ids) {
    if (_listeners.has(sessionId)) continue // already subscribed
    // Restore live turn state that was saved before HMR reloaded this module
    restoreLiveState(sessionId)
    chat.running[sessionId] = true // backend loop is confirmed running — enforce after restore
    chat.streamingToken[sessionId] = chat.streamingToken[sessionId] ?? ''
    if (!chat.liveBlocks[sessionId]) chat.liveBlocks[sessionId] = []
    if (!chat.toolItems[sessionId])  chat.toolItems[sessionId]  = []
    // Reconcile stale tool statuses: tools we think are pending but the backend
    // already processed should be shown as done (their ToolResult event was lost).
    chat.toolItems[sessionId] = chat.toolItems[sessionId].map(t => {
      if ((t.status === 'waiting' || t.status === 'running') && !pendingIds.has(t.toolUseId)) {
        return { ...t, status: 'done' as const }
      }
      return t
    })
    const unlisten = await listenAgentEvents(sessionId, (ev) => handleEvent(sessionId, ev))
    _listeners.set(sessionId, unlisten)
  }
}

function handleEvent(sessionId: string, ev: AgentEvent) {
  if (ev.event === 'reasoning') {
    // Streaming reasoning/thinking tokens — shown live in an expandable card.
    chat.liveReasoning[sessionId] = (chat.liveReasoning[sessionId] ?? '') + ev.content
    saveLiveState(sessionId)

  } else if (ev.event === 'token') {
    chat.streamingToken[sessionId] = (chat.streamingToken[sessionId] ?? '') + ev.content

    const blocks = chat.liveBlocks[sessionId] ?? []
    const last = blocks.length > 0 ? blocks[blocks.length - 1] : null
    if (last?.type === 'text') {
      chat.liveBlocks[sessionId] = [
        ...blocks.slice(0, -1),
        { type: 'text', text: last.text + ev.content },
      ]
    } else {
      chat.liveBlocks[sessionId] = [...blocks, { type: 'text', text: ev.content }]
    }
    saveLiveState(sessionId)

  } else if (ev.event === 'tool_call') {
    const alwaysSet = chat.alwaysApprove[sessionId] ?? new Set<string>()
    const autoApprove = chat.autoApprove[sessionId] ?? false
    const shouldAutoApprove = autoApprove || SAFE_TOOLS.has(ev.name) || alwaysSet.has(ev.name)

    chat.toolItems[sessionId] = [
      ...(chat.toolItems[sessionId] ?? []),
      {
        toolUseId: ev.toolUseId,
        name: ev.name,
        input: ev.input,
        status: shouldAutoApprove ? 'running' : 'waiting',
      }
    ]
    chat.liveBlocks[sessionId] = [
      ...(chat.liveBlocks[sessionId] ?? []),
      { type: 'tool', toolUseId: ev.toolUseId },
    ]

    if (shouldAutoApprove) {
      apiApprove(ev.toolUseId)
    }
    saveLiveState(sessionId)

  } else if (ev.event === 'tool_result') {
    chat.toolItems[sessionId] = (chat.toolItems[sessionId] ?? []).map(t => {
      if (t.toolUseId !== ev.toolUseId) return t
      const status = t.status === 'error' ? 'error' : (ev.isError ? 'error' : 'done')
      return { ...t, output: ev.output, isError: ev.isError, status }
    })
    saveLiveState(sessionId)

  } else if (ev.event === 'working_dir_changed') {
    sessions.list = sessions.list.map(s =>
      s.id === sessionId ? { ...s, workingDir: ev.newPath } : s
    )

  } else if (ev.event === 'interrupted') {
    chat.notice[sessionId] = 'Turn interrupted by user.'
    chat.running[sessionId] = false
    chat.streamingToken[sessionId] = null
    // Keep the streamed reasoning visible so the interrupted turn doesn't
    // vanish — it is cleared at the start of the next sendMessage.
    _listeners.get(sessionId)?.()
    _listeners.delete(sessionId)
    loadMessages(sessionId)

  } else if (ev.event === 'error') {
    let msg = ev.message
    try {
      const parsed = JSON.parse(msg)
      if (parsed?.error?.message) msg = parsed.error.message
    } catch { /* keep raw */ }
    if (!msg) msg = 'Model request failed (no error details provided).'
    chat.error[sessionId] = msg
    chat.running[sessionId] = false
    chat.streamingToken[sessionId] = null
    // Keep the streamed reasoning so the thought the user watched doesn't
    // vanish on failure — cleared at the start of the next sendMessage.
    _listeners.get(sessionId)?.()
    _listeners.delete(sessionId)
    loadMessages(sessionId)

  } else if (ev.event === 'done') {
    chat.running[sessionId] = false
    chat.streamingToken[sessionId] = null
    chat.liveReasoning[sessionId] = ''
    clearLiveState(sessionId)
    _listeners.get(sessionId)?.()
    _listeners.delete(sessionId)
    loadMessages(sessionId)
  }
}

/** Cancel a running turn (Esc / Stop button). Called optimistically — the
 *  backend also fires `interrupted`; first one wins for the UI cleanup. */
export async function stopTurn(sessionId: string) {
  if (!(chat.running[sessionId] ?? false)) return
  try {
    await interruptSession(sessionId)
  } catch { /* backend already idle — ignore */ }
  // The backend emits `interrupted` which finishes the cleanup; but if the
  // backend was *between* turns, make sure the UI doesn't stay stuck.
  chat.running[sessionId] = false
}

export async function loadMessages(sessionId: string) {
  // Only update messages — keep liveBlocks/toolItems alive so the live turn stays
  // visible after completion. They're cleared at the START of the next sendMessage.
  chat.messages[sessionId] = await fetchMessages(sessionId)
}

export async function sendMessage(
  sessionId: string,
  content: string,
  model: string,
  provider: string,
  workingDir: string,
  autoApprove: boolean
) {
  // Persist autoApprove in store so handleEvent can access it after HMR
  chat.autoApprove[sessionId] = autoApprove

  // Clear live turn state from the previous run
  chat.running[sessionId] = true
  chat.streamingToken[sessionId] = ''
  chat.liveBlocks[sessionId] = []
  chat.liveReasoning[sessionId] = ''
  chat.toolItems[sessionId] = []
  chat.error[sessionId] = null
  chat.notice[sessionId] = null
  clearLiveState(sessionId)
  // Save autoApprove immediately so it survives HMR even before the first event arrives
  saveLiveState(sessionId)

  // Show user message immediately — replaced by DB reload on completion
  const optimistic: Message = {
    id: `optimistic-${Date.now()}`,
    sessionId,
    role: 'user',
    content: JSON.stringify(content),
    model,
    provider,
    timestamp: new Date().toISOString(),
    tokens: 0,
  }
  chat.messages[sessionId] = [...(chat.messages[sessionId] ?? []), optimistic]

  // Set when the backend tells us another loop is already running — in that
  // case the `finally` must leave the running state alone.
  let keepRunning = false

  try {
    await runAgenticLoop(sessionId, content, model, provider, workingDir, autoApprove, (ev) => handleEvent(sessionId, ev))
  } catch (err) {
    const msg = String(err)
    if (msg.includes('already has an active agentic loop')) {
      // Only trust this if the backend actually reports a live loop in memory.
      // A DB-only lock (crashed instance, stale row) would otherwise leave the
      // UI spinning forever with the typed message silently dropped.
      const live = await getRunningSessionIds()
      if (!live.includes(sessionId)) {
        chat.running[sessionId] = false
        chat.streamingToken[sessionId] = null
        chat.liveReasoning[sessionId] = ''
        chat.error[sessionId] =
          'The previous run left a stale lock and was recovered — please send your message again.'
        loadMessages(sessionId)
        return
      }
      // A loop IS still running (leftover from before an HMR reload, or a
      // second app instance). Keep the UI running, drop our optimistic message
      // (it was never persisted), and re-attach the listener if we lost it.
      keepRunning = true
      chat.running[sessionId] = true
      chat.messages[sessionId] = (chat.messages[sessionId] ?? []).filter(m => m.id !== optimistic.id)
      if (!_listeners.has(sessionId)) {
        reconnectRunningSessions()
      }
    } else {
      chat.error[sessionId] = msg || 'Model request failed (no error details provided).'
    }
  } finally {
    // Only reset the running flag if this call owned the loop. If we hit the
    // "already running" branch above, the OTHER loop's events own the state.
    if (!keepRunning) {
      chat.running[sessionId] = false
      chat.streamingToken[sessionId] = null
      chat.liveReasoning[sessionId] = ''
      loadMessages(sessionId)
    }
  }
}

/** Delete messages after (and possibly restore files touched by) a message. */
export async function revertToMessage(
  sessionId: string,
  toMessageId: string,
  revertFiles: boolean
): Promise<RevertSummary> {
  const summary = await revertSession(sessionId, toMessageId, revertFiles)
  // Reset transient UI state and reload the shortened conversation.
  chat.running[sessionId] = false
  chat.streamingToken[sessionId] = null
  chat.liveReasoning[sessionId] = ''
  chat.error[sessionId] = null
  chat.notice[sessionId] = null
  chat.liveBlocks[sessionId] = []
  chat.toolItems[sessionId] = []
  clearLiveState(sessionId)
  await loadMessages(sessionId)
  return summary
}

export function approveTool(sessionId: string, toolUseId: string) {
  chat.toolItems[sessionId] = (chat.toolItems[sessionId] ?? []).map(t =>
    t.toolUseId === toolUseId ? { ...t, status: 'running' as const } : t
  )
  saveLiveState(sessionId) // persist before HMR can fire during tool execution
  apiApprove(toolUseId)
}

export function denyTool(sessionId: string, toolUseId: string) {
  chat.toolItems[sessionId] = (chat.toolItems[sessionId] ?? []).map(t =>
    t.toolUseId === toolUseId ? { ...t, status: 'error' as const, output: 'Denied by user.' } : t
  )
  saveLiveState(sessionId)
  apiDeny(toolUseId)
}

export function alwaysApproveTool(sessionId: string, toolName: string) {
  const existing = chat.alwaysApprove[sessionId] ?? new Set<string>()
  chat.alwaysApprove[sessionId] = new Set([...existing, toolName])
  saveLiveState(sessionId)
}
