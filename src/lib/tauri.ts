import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Session, Message, ProviderInfo, AgentEvent, RevertInfo, RevertSummary, SessionStats, ThinkingPref, VerificationSettings } from './types'
import {
  isBrowserMock,
  mockSessions,
  mockMessages,
  mockProviders,
  mockPlugins,
  mockSkills,
  mockStats,
} from './mock'

// ── Browser mock mode ─────────────────────────────────────────────────────
// When the frontend runs without the Tauri backend (plain `vite dev` in a
// browser — design iteration / plugin development), the data-layer functions
// below fall back to sample data so the full UI still renders.
function mock<T>(fallback: T, cmd: string): Promise<T> {
  if (isBrowserMock) return Promise.resolve(fallback)
  return tauriInvoke<T>(cmd)
}

// Contributes as serialized by Rust (all camelCase) — shared with the plugin
// store (which imports from this module for compatibility).
import type { PluginContributes, PluginInfo, SkillInfo } from './types'
export type { PluginContributes, PluginInfo } from './types'

export const listSessions = () => mock<Session[]>(mockSessions, 'list_sessions')

/** Backend auto-named a session from its first prompt — update it live so the
 *  tab bar and sidebar show the new title without a reload. */
export function listenSessionRenamed(onRename: (e: { id: string; name: string }) => void) {
  if (isBrowserMock) return Promise.resolve(() => {})
  return listen<{ id: string; name: string }>('session:renamed', (e) => onRename(e.payload))
}
export const createSession = (name: string) => tauriInvoke<Session>('create_session', { name })
export const getSession = (id: string) =>
  isBrowserMock
    ? Promise.resolve(mockSessions.find((s) => s.id === id) ?? null)
    : tauriInvoke<Session>('get_session', { id })
export const updateSession = (id: string, model: string, provider: string, workingDir: string) =>
  isBrowserMock ? Promise.resolve() : tauriInvoke<void>('update_session', { id, model, provider, workingDir })
export const archiveSession = (id: string) => tauriInvoke<void>('archive_session', { id })

export const listMessages = (sessionId: string) =>
  isBrowserMock
    ? Promise.resolve(mockMessages[sessionId] ?? [])
    : tauriInvoke<Message[]>('list_messages', { sessionId })

export const listProviders = () => mock<ProviderInfo[]>(mockProviders, 'list_providers')
/** Save a provider's API key list (rotation order — first is primary). */
export const saveApiKeys = (provider: string, keys: string[]) => tauriInvoke<void>('save_api_keys', { provider, keys })
export const getApiKeys = () => mock<Record<string, string[]>>({}, 'get_api_keys')

// ── Verification settings ──────────────────────────────────────────────────
export const getVerificationSettings = (): Promise<VerificationSettings> =>
  isBrowserMock
    ? Promise.resolve({
        enabled: true,
        runChecks: true,
        independentReview: true,
        maxRounds: 2,
        reviewerModel: null,
        testCommand: null,
      })
    : tauriInvoke<VerificationSettings>('get_verification_settings')

export const setVerificationSettings = (settings: VerificationSettings) =>
  isBrowserMock
    ? Promise.resolve()
    : tauriInvoke<void>('set_verification_settings', { settings })

export const approveTool = (toolUseId: string) => tauriInvoke<void>('approve_tool', { toolUseId })
export const denyTool = (toolUseId: string) => tauriInvoke<void>('deny_tool', { toolUseId })
export const getPendingApprovals = () => mock<string[]>([], 'get_pending_approvals')

/** Answer a pending ask_question (resumes the paused agentic loop). */
export const answerQuestion = (questionId: string, answer: string | null) =>
  isBrowserMock ? Promise.resolve() : tauriInvoke<void>('answer_question', { questionId, answer })

/** OS notification preference (used by the Rust notifier). */
export const setNotificationsEnabled = (enabled: boolean) =>
  isBrowserMock ? Promise.resolve() : tauriInvoke<void>('set_notifications_enabled', { enabled })
export const getNotificationsEnabled = () =>
  isBrowserMock ? Promise.resolve(true) : tauriInvoke<boolean>('get_notifications_enabled')

export const listPlugins = () => mock<PluginInfo[]>(mockPlugins, 'list_plugins')
export const setPluginEnabled = (id: string, enabled: boolean) =>
  isBrowserMock ? Promise.resolve() : tauriInvoke<void>('set_plugin_enabled', { id, enabled })
export const readPluginComponent = (id: string) =>
  isBrowserMock
    ? // Minimal placeholder component so plugin slots visibly render in mock mode.
      Promise.resolve(`export default class extends HTMLElement {
        connectedCallback() {
          const slot = this.getAttribute('data-slot') ?? 'slot'
          this.attachShadow({ mode: 'open' }).innerHTML = '<div style="font:600 10px monospace;letter-spacing:.06em;color:#7a8b99;padding:6px 8px;border:1px dashed #3a4650;border-radius:4px">mock-plugin · ' + slot + '</div>'
        }
      }`)
    : tauriInvoke<string>('read_plugin_component', { id })
export const readPluginSkill = (id: string) => tauriInvoke<string>('read_plugin_skill', { id })
export const listSkills = () => mock<SkillInfo[]>(mockSkills, 'list_skills')
export const openPluginFolder = (id: string) => tauriInvoke<void>('open_plugin_folder', { id })

// ── Plugin skill sidecars ───────────────────────────────────────────────────
// Bridge for UI widgets to see model-driven skill sidecars and drive their
// tools (used by the pragma-dev status-bar widget).
export interface SkillSidecarInfo {
  skillId: string
  pid: number
  running: boolean
  tools: string[]
}

export const skillSidecarStatus = (): Promise<SkillSidecarInfo[]> =>
  isBrowserMock ? Promise.resolve([]) : tauriInvoke<SkillSidecarInfo[]>('skill_sidecar_status')

export const callSkillTool = (skillId: string, name: string, args: Record<string, unknown> = {}) =>
  isBrowserMock ? Promise.resolve('{}') : tauriInvoke<string>('call_skill_tool', { skillId, name, arguments: args })

// ── Plugin process management ────────────────────────────────────────────────
export const spawnPluginProcess = (pluginId: string, program: string, args: string[], cwd?: string | null) =>
  tauriInvoke<number>('spawn_plugin_process', { pluginId, program, args, cwd: cwd ?? null })
export const stopPluginProcess = (pluginId: string) =>
  tauriInvoke<void>('stop_plugin_process', { pluginId })
export const pluginProcessStatus = (pluginId: string) =>
  tauriInvoke<{ running: boolean; pid: number | null }>('plugin_process_status', { pluginId })
export const writePluginStdin = (pluginId: string, data: string) =>
  tauriInvoke<void>('write_plugin_stdin', { pluginId, data })
export const probePort = (port: number) => tauriInvoke<boolean>('probe_port', { port })

// ── Plugin config ────────────────────────────────────────────────────────────
export const getPluginConfig = (pluginId: string) =>
  isBrowserMock ? Promise.resolve({}) : tauriInvoke<Record<string, unknown>>('get_plugin_config', { pluginId })
export const setPluginConfig = (pluginId: string, config: Record<string, unknown>) =>
  isBrowserMock ? Promise.resolve() : tauriInvoke<void>('set_plugin_config', { pluginId, config })

// ── Plugin confirmation ──────────────────────────────────────────────────────
export const requestPluginConfirmation = (pluginId: string, title: string, message: string) =>
  tauriInvoke<boolean>('request_plugin_confirmation', { pluginId, title, message })
export const respondPluginConfirmation = (requestId: string, accepted: boolean) =>
  tauriInvoke<void>('respond_plugin_confirmation', { requestId, accepted })

export const getRunningSessionIds = () => mock<string[]>([], 'get_running_sessions')

export const interruptSession = (sessionId: string) =>
  isBrowserMock ? Promise.resolve() : tauriInvoke<void>('interrupt_session', { sessionId })

export const getSessionStats = (sessionId: string) =>
  isBrowserMock ? Promise.resolve(mockStats[sessionId] ?? null) : tauriInvoke<SessionStats | null>('get_session_stats', { sessionId })

export const getRevertInfo = (sessionId: string, toMessageId: string) =>
  tauriInvoke<RevertInfo>('get_revert_info', { sessionId, toMessageId })

export const revertSession = (sessionId: string, toMessageId: string, revertFiles: boolean) =>
  tauriInvoke<RevertSummary>('revert_session', { sessionId, toMessageId, revertFiles })

/** Subscribe to agent events for a session via Tauri's global event bus.
 *  Returns an unlisten function. Survives frontend HMR reloads. */
export function listenAgentEvents(sessionId: string, onEvent: (e: AgentEvent) => void) {
  return listen<AgentEvent>(`agent:${sessionId}`, (e) => onEvent(e.payload))
}

export async function runAgenticLoop(
  sessionId: string,
  userMessage: string,
  model: string,
  provider: string,
  workingDir: string,
  autoApprove: boolean,
  thinking: ThinkingPref,
  onEvent: (e: AgentEvent) => void,
): Promise<void> {
  if (isBrowserMock) {
    // Fake a structured stream mirroring Anthropic content blocks (thought →
    // text → tool → thought → text) so the live render can be compared
    // against the persisted, block-separated message.
    const say = (ev: AgentEvent) => onEvent(ev)
    say({ event: 'reasoning', content: 'Reading the hook to see how its signature is used across the codebase. ' })
    setTimeout(() => say({ event: 'reasoning', content: 'The callers pass positional arguments, so renaming is risky.' }), 200)
    setTimeout(() => say({ event: 'thinking_end' }), 320)
    setTimeout(() => say({ event: 'token', content: 'I\u2019ve analyzed the call sites. ' }), 420)
    setTimeout(() => say({ event: 'token', content: 'Here\u2019s what I\u2019ll do.' }), 520)
    setTimeout(() => say({ event: 'tool_call', runId: 'r1', toolUseId: 'mock-read', name: 'read_file', input: { path: 'src/hook.ts' } }), 700)
    setTimeout(() => say({ event: 'tool_result', toolUseId: 'mock-read', output: 'export function useThing(a, b) { … }', isError: false }), 1000)
    setTimeout(() => say({ event: 'reasoning', content: 'Second thought: handle the deprecation wrapper so existing imports keep working.' }), 1200)
    setTimeout(() => say({ event: 'thinking_end' }), 1400)
    setTimeout(() => say({ event: 'token', content: 'Both approaches are fine — finished on your call.' }), 1600)
    // ask_question renders ONLY via the `question` event — deliberately no
    // ToolCall event here (backend skips it), so no tool card / approval UI.
    setTimeout(() => say({ event: 'question', questionId: 'mock-q1', text: 'Want me to keep the old signature as a deprecated wrapper?', options: ['Keep a wrapper', 'Clean break', 'Show me both'] }), 1850)
    // No working dir set → path tools are blocked and the user is asked for a
    // folder. Options omitted → free-text answer (which folder?) UI.
    setTimeout(() => say({ event: 'question', questionId: 'mock-q2', text: 'Which project folder should I use as the working directory? I can\u2019t read or write any files until one is set \u2014 give me a path.', options: [] }), 2300)
    setTimeout(() => { say({ event: 'done' }); return }, 2950)
    return
  }
  // Subscribe BEFORE invoking so no events are missed.
  const unlisten = await listenAgentEvents(sessionId, onEvent)
  try {
    await tauriInvoke<void>('run_agentic_loop', {
      sessionId, userMessage, model, provider, workingDir, autoApprove, thinking,
    })
  } finally {
    unlisten()
  }
}