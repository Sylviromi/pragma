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
  | { type: 'reasoning'; content: string }
  | { type: 'thinking'; thinking: string; signature?: string }

export type TextBlock = Extract<ContentBlock, { type: 'text' }>
export type ToolUseBlock = Extract<ContentBlock, { type: 'tool_use' }>
export type ToolResultBlock = Extract<ContentBlock, { type: 'tool_result' }>
export type ReasoningBlock = Extract<ContentBlock, { type: 'reasoning' }>
export type ThinkingBlock = Extract<ContentBlock, { type: 'thinking' }>

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
  | { event: 'reasoning'; content: string }
  // Fields are camelCase — Rust AgentEvent variants use #[serde(rename_all = "camelCase")]
  | { event: 'tool_call'; runId: string; toolUseId: string; name: string; input: Record<string, unknown> }
  | { event: 'tool_result'; toolUseId: string; output: string; isError: boolean }
  | { event: 'done' }
  | { event: 'error'; message: string }
  | { event: 'interrupted' }
  | { event: 'working_dir_changed'; newPath: string }

export interface RevertInfo {
  files: string[]
  messagesRemoved: number
  preview: string
}

export interface RevertSummary {
  messagesRemoved: number
  filesReverted: string[]
}

// Parsed message content helpers — shared by the message components and the
// transcript builder so text/tool extraction stays in one place.
export function parseContent(raw: string): MessageContent {
  try {
    return JSON.parse(raw)
  } catch {
    return raw
  }
}

/** Does this parsed content contain any tool_result block? */
export function hasToolResults(c: MessageContent): boolean {
  return Array.isArray(c) && c.some((b) => b.type === 'tool_result')
}

/** Is this parsed content *only* tool results (i.e. not a real user prompt)? */
export function isToolResultTurn(c: MessageContent): boolean {
  return Array.isArray(c) && c.every((b) => b.type === 'tool_result')
}

/** The text blocks of parsed content, in order. */
export function textBlocks(c: MessageContent): TextBlock[] {
  return Array.isArray(c) ? c.filter((b): b is TextBlock => b.type === 'text') : []
}

/** Concatenated text of a stored message's JSON content, separated by `sep`. */
export function messageText(raw: string, sep = '\n'): string {
  const c = parseContent(raw)
  if (typeof c === 'string') return c
  return textBlocks(c)
    .map((b) => b.text)
    .join(sep)
}