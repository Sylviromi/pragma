import {
  runAgenticLoop,
  listenAgentEvents,
  getRunningSessionIds,
  listMessages as fetchMessages,
  approveTool as apiApprove,
  denyTool as apiDeny
} from '../lib/tauri'
import type { AgentEvent, Message } from '../lib/types'
import { sessions } from './sessions.svelte'

// Unlisten functions keyed by sessionId — outlive component HMR reloads.
const _listeners = new Map<string, () => void>()

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
  toolItems:      {} as Record<string, ToolItem[]>,
  running:        {} as Record<string, boolean>,
  error:          {} as Record<string, string | null>,
  alwaysApprove:  {} as Record<string, Set<string>>,
  autoApprove:    {} as Record<string, boolean>,
})

/** Re-attach to any agentic loops that are still running on the Rust side.
 *  Call this on app startup / after HMR reloads. */
export async function reconnectRunningSessions() {
  const ids = await getRunningSessionIds()
  for (const sessionId of ids) {
    if (_listeners.has(sessionId)) continue // already subscribed
    chat.running[sessionId] = true
    chat.streamingToken[sessionId] = chat.streamingToken[sessionId] ?? ''
    chat.liveBlocks[sessionId] = chat.liveBlocks[sessionId] ?? []
    chat.toolItems[sessionId] = chat.toolItems[sessionId] ?? []
    const unlisten = await listenAgentEvents(sessionId, (ev) => handleEvent(sessionId, ev))
    _listeners.set(sessionId, unlisten)
  }
}

function handleEvent(sessionId: string, ev: AgentEvent) {
  if (ev.event === 'token') {
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

  } else if (ev.event === 'tool_result') {
    chat.toolItems[sessionId] = (chat.toolItems[sessionId] ?? []).map(t => {
      if (t.toolUseId !== ev.toolUseId) return t
      const status = t.status === 'error' ? 'error' : (ev.isError ? 'error' : 'done')
      return { ...t, output: ev.output, isError: ev.isError, status }
    })

  } else if (ev.event === 'working_dir_changed') {
    sessions.list = sessions.list.map(s =>
      s.id === sessionId ? { ...s, workingDir: ev.newPath } : s
    )

  } else if (ev.event === 'error') {
    let msg = ev.message
    try {
      const parsed = JSON.parse(msg)
      if (parsed?.error?.message) msg = parsed.error.message
    } catch { /* keep raw */ }
    chat.error[sessionId] = msg
    chat.running[sessionId] = false
    chat.streamingToken[sessionId] = null
    _listeners.get(sessionId)?.()
    _listeners.delete(sessionId)
    loadMessages(sessionId)

  } else if (ev.event === 'done') {
    chat.running[sessionId] = false
    chat.streamingToken[sessionId] = null
    _listeners.get(sessionId)?.()
    _listeners.delete(sessionId)
    loadMessages(sessionId)
  }
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
  chat.toolItems[sessionId] = []
  chat.error[sessionId] = null

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

  try {
    await runAgenticLoop(sessionId, content, model, provider, workingDir, autoApprove, (ev) => handleEvent(sessionId, ev))
  } catch (err) {
    chat.error[sessionId] = String(err)
  } finally {
    chat.running[sessionId] = false
    chat.streamingToken[sessionId] = null
    loadMessages(sessionId)
  }
}

export function approveTool(sessionId: string, toolUseId: string) {
  chat.toolItems[sessionId] = (chat.toolItems[sessionId] ?? []).map(t =>
    t.toolUseId === toolUseId ? { ...t, status: 'running' as const } : t
  )
  apiApprove(toolUseId)
}

export function denyTool(sessionId: string, toolUseId: string) {
  chat.toolItems[sessionId] = (chat.toolItems[sessionId] ?? []).map(t =>
    t.toolUseId === toolUseId ? { ...t, status: 'error' as const, output: 'Denied by user.' } : t
  )
  apiDeny(toolUseId)
}

export function alwaysApproveTool(sessionId: string, toolName: string) {
  const existing = chat.alwaysApprove[sessionId] ?? new Set<string>()
  chat.alwaysApprove[sessionId] = new Set([...existing, toolName])
}
