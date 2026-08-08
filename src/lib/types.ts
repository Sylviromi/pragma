export interface Session {
  id: string
  name: string
  workingDir: string
  createdAt: string
  updatedAt: string
  model: string
  provider: string
  archived: boolean
  metadataJson: string
}

export type ContentBlock =
  | { type: 'text'; text: string }
  | { type: 'tool_use'; id: string; name: string; input: Record<string, unknown> }
  | { type: 'tool_result'; tool_use_id: string; content: string; is_error?: boolean }

export type MessageContent = string | ContentBlock[]

export interface Message {
  id: string
  sessionId: string
  role: 'user' | 'assistant' | 'system'
  content: string // serialised JSON: string or ContentBlock[]
  model: string
  provider: string
  timestamp: string
  tokens: number
}

export interface ModelInfo {
  id: string
  name: string
  contextWindow: number | null  // camelCase — Rust sends contextWindow
  isFree: boolean
  isVision: boolean
}

export interface ProviderInfo {
  id: string
  name: string
  // Rust: ProviderSource::Builtin → "builtin", Plugin { plugin_id } → { plugin: { pluginId } }
  source: 'builtin' | { plugin: { pluginId: string } }
  models: ModelInfo[]
}

export type AgentEvent =
  | { event: 'token'; content: string }
  // Fields are camelCase — Rust AgentEvent variants use #[serde(rename_all = "camelCase")]
  | { event: 'tool_call'; runId: string; toolUseId: string; name: string; input: Record<string, unknown> }
  | { event: 'tool_result'; toolUseId: string; output: string; isError: boolean }
  | { event: 'done' }
  | { event: 'error'; message: string }

// Parsed message content helper
export function parseContent(raw: string): MessageContent {
  try {
    return JSON.parse(raw)
  } catch {
    return raw
  }
}