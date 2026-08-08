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
  running: {} as Record<string, boolean>
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
        // Remove from pending
        chat.pendingTools[sessionId] = (chat.pendingTools[sessionId] ?? []).filter(
          (t) => t.toolUseId !== ev.toolUseId
        )
      } else if (ev.event === 'done' || ev.event === 'error') {
        chat.streamingToken[sessionId] = null
        chat.running[sessionId] = false
        // Reload messages from DB to get persisted state
        loadMessages(sessionId)
      }
    })
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