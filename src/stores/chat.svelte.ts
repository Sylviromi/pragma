import {
  runAgenticLoop,
  listMessages as fetchMessages,
  approveTool as apiApprove,
  denyTool as apiDeny
} from '../lib/tauri'
import type { AgentEvent, Message } from '../lib/types'

export interface PendingTool {
  runId: string
  toolUseId: string
  name: string
  input: Record<string, unknown>
}

export interface ToolResult {
  toolUseId: string
  output: string
  isError: boolean
}

export const chat = $state({
  messages: {} as Record<string, Message[]>,
  streamingToken: {} as Record<string, string | null>,
  pendingTools: {} as Record<string, PendingTool[]>,
  toolResults: {} as Record<string, ToolResult[]>,
  running: {} as Record<string, boolean>,
  error: {} as Record<string, string | null>,
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
  chat.pendingTools[sessionId] = []
  chat.toolResults[sessionId] = []
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
        chat.pendingTools[sessionId] = [
          ...(chat.pendingTools[sessionId] ?? []),
          { runId: ev.runId, toolUseId: ev.toolUseId, name: ev.name, input: ev.input }
        ]
      } else if (ev.event === 'tool_result') {
        chat.toolResults[sessionId] = [
          ...(chat.toolResults[sessionId] ?? []),
          { toolUseId: ev.toolUseId, output: ev.output, isError: ev.isError }
        ]
        chat.pendingTools[sessionId] = (chat.pendingTools[sessionId] ?? []).filter(
          (t) => t.toolUseId !== ev.toolUseId
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

export async function approveTool(_sessionId: string, toolUseId: string) {
  await apiApprove(toolUseId)
}

export async function denyTool(_sessionId: string, toolUseId: string) {
  await apiDeny(toolUseId)
}
