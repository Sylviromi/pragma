import {
  runAgenticLoop,
  listMessages as fetchMessages,
  approveTool as apiApprove,
  denyTool as apiDeny
} from '../lib/tauri'
import type { AgentEvent, Message } from '../lib/types'

export interface ToolItem {
  toolUseId: string
  name: string
  input: Record<string, unknown>
  output?: string
  isError?: boolean
  /** waiting = needs user approval, running = approved/executing, done/error = finished */
  status: 'waiting' | 'running' | 'done' | 'error'
}

// Read-only tools: auto-approve without asking
const SAFE_TOOLS = new Set([
  'read_file', 'list_files', 'search_files',
  'git_status', 'git_diff', 'git_log',
  'web_search', 'fetch_url',
])

export const chat = $state({
  messages:     {} as Record<string, Message[]>,
  streamingToken: {} as Record<string, string | null>,
  toolItems:    {} as Record<string, ToolItem[]>,
  running:      {} as Record<string, boolean>,
  error:        {} as Record<string, string | null>,
  // per-session set of tool names the user has said "always approve"
  alwaysApprove: {} as Record<string, Set<string>>,
})

export async function loadMessages(sessionId: string) {
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
  chat.running[sessionId] = true
  chat.streamingToken[sessionId] = ''
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
    await runAgenticLoop(sessionId, content, model, provider, workingDir, autoApprove, (ev: AgentEvent) => {
      if (ev.event === 'token') {
        chat.streamingToken[sessionId] = (chat.streamingToken[sessionId] ?? '') + ev.content

      } else if (ev.event === 'tool_call') {
        const alwaysSet = chat.alwaysApprove[sessionId] ?? new Set<string>()
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

        if (shouldAutoApprove) {
          // Fire-and-forget: signal backend immediately
          apiApprove(ev.toolUseId)
        }

      } else if (ev.event === 'tool_result') {
        chat.toolItems[sessionId] = (chat.toolItems[sessionId] ?? []).map(t =>
          t.toolUseId === ev.toolUseId
            ? { ...t, output: ev.output, isError: ev.isError, status: ev.isError ? 'error' : 'done' }
            : t
        )

      } else if (ev.event === 'error') {
        chat.error[sessionId] = ev.message
        chat.streamingToken[sessionId] = null
        chat.running[sessionId] = false
        loadMessages(sessionId)

      } else if (ev.event === 'done') {
        chat.streamingToken[sessionId] = null
        chat.running[sessionId] = false
        loadMessages(sessionId)
      }
    })
  } catch (err) {
    chat.error[sessionId] = String(err)
  } finally {
    chat.running[sessionId] = false
    chat.streamingToken[sessionId] = null
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
