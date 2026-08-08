import { invoke as tauriInvoke, Channel } from '@tauri-apps/api/core'
import type { Session, Message, ProviderInfo, AgentEvent } from './types'

export const invoke = tauriInvoke

export const listSessions = () => tauriInvoke<Session[]>('list_sessions')
export const createSession = (name: string) => tauriInvoke<Session>('create_session', { name })
export const getSession = (id: string) => tauriInvoke<Session>('get_session', { id })
export const updateSession = (id: string, model: string, provider: string, workingDir: string) =>
  tauriInvoke<void>('update_session', { id, model, provider, workingDir })
export const archiveSession = (id: string) => tauriInvoke<void>('archive_session', { id })

export const listMessages = (sessionId: string) => tauriInvoke<Message[]>('list_messages', { sessionId })

export const listProviders = () => tauriInvoke<ProviderInfo[]>('list_providers')
export const saveApiKey = (provider: string, key: string) => tauriInvoke<void>('save_api_key', { provider, key })
export const getApiKeys = () => tauriInvoke<Record<string, string>>('get_api_keys')

export const approveTool = (toolUseId: string) => tauriInvoke<void>('approve_tool', { toolUseId })
export const denyTool = (toolUseId: string) => tauriInvoke<void>('deny_tool', { toolUseId })

export function runAgenticLoop(
  sessionId: string,
  userMessage: string,
  model: string,
  provider: string,
  workingDir: string,
  autoApprove: boolean,
  onEvent: (e: AgentEvent) => void,
): Promise<void> {
  const channel = new Channel<AgentEvent>()
  channel.onmessage = onEvent
  return tauriInvoke<void>('run_agentic_loop', {
    sessionId, userMessage, model, provider, workingDir, autoApprove, channel,
  })
}