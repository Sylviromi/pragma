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
  role: 'user' | 'assistant' | 'system' | 'verification'
  content: string // serialised JSON: string or ContentBlock[]
  model: string
  provider: string
  timestamp: string
  tokens: number
}

// ── Plugins (also re-exported from lib/tauri.ts for the plugin store) ───
export interface PluginContributes {
  providers: string[]
  slots: string[]
  commands: string[]
  skills: string[]
}

export interface PluginInfo {
  id: string
  name: string
  version: string
  description: string
  enabled: boolean
  contributes: PluginContributes
  hasComponent: boolean
  hasSkill: boolean
  skills: SkillInfo[]
  folder: string
}

export interface SkillInfo {
  id: string
  name: string
  description: string
  source: 'native' | 'plugin' | 'plugin-skill'
  pluginId: string | null
  hasTools: boolean
  hasHandler: boolean
}

export interface ModelInfo {
  id: string
  name: string
  contextWindow: number | null  // camelCase — Rust sends contextWindow
  isFree: boolean
  isVision: boolean
  maxTokens?: number | null      // per-model output ceiling (provider.json)
  reasoningEffort?: string | null // 'low' | 'medium' | 'high' (provider.json)
  supportsThinking: boolean      // thinking/reasoning capability (catalog/discovery/provider.json)
  supportsThinkingLevels: boolean // graduated levels: anthropic thinking_level, openai reasoning_effort
  thinkingBudget?: number | null // Anthropic budget_tokens override
}

/** User thinking preference: off / auto (model default) / explicit level. */
export type ThinkingPref = 'off' | 'auto' | 'low' | 'medium' | 'high'

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
  // A thinking block finished streaming — start a fresh thought on next reasoning.
  | { event: 'thinking_end' }
  // Fields are camelCase — Rust AgentEvent variants use #[serde(rename_all = "camelCase")]
  | { event: 'tool_call'; runId: string; toolUseId: string; name: string; input: Record<string, unknown> }
  | { event: 'tool_result'; toolUseId: string; output: string; isError: boolean }
  // The model asked the user a question (ask_question tool) — loop paused until
  // answer_question(questionId, …) is called.
  | { event: 'question'; questionId: string; text: string; options: string[] }
  | { event: 'done' }
  | { event: 'error'; message: string }
  | { event: 'interrupted' }
  // Harness verification phase (loop-done): started a sub-phase.
  | { event: 'verification_started'; phase: 'checks' | 'review' }
  // Verification finished — passed=false means the report was fed back for a
  // fix round (rounds < maxRounds) or surfaced to the user (rounds exhausted).
  // cancelled=true = user stopped it (Esc). tokens/durationMs power the
  // bubble meta; checksLabel names the command that ran.
  | {
      event: 'verification_result'
      passed: boolean
      cancelled: boolean
      summary: string
      findings: string[]
      rounds: number
      tokens: number
      durationMs: number
      checksLabel: string | null
    }
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

// ── Session telemetry (right sidebar) ────────────────────────────────────
export interface ContextSegment {
  label: string
  tokens: number
  color: string // CSS var token name — the design's legend dot color
}

export interface CostRow {
  kind: 'model' | 'verification'
  model: string
  tokens: string // humanized, e.g. '52.4k'
  cost: string // '$0.41'
}

// Plan checklist parsed from PLAN.md in the session working directory.
export interface PlanItem {
  label: string
  state: 'todo' | 'active' | 'done'
}

// Granular toggles for the harness verification layer (Settings → Verification).
export interface VerificationSettings {
  enabled: boolean
  runChecks: boolean
  independentReview: boolean
  maxRounds: number
  reviewerModel: string | null
  testCommand: string | null
}

// Live verification state for the current turn (chat store → UI card). Also
// the shape persisted as a role='verification' message (parseVerification).
export interface VerificationInfo {
  passed: boolean
  cancelled: boolean
  summary: string
  findings: string[]
  rounds: number
  tokens: number
  durationMs: number
  checksLabel: string | null
  checksRan: boolean
  reviewUsed: boolean
}

/** Parse a stored role='verification' message's content JSON. */
export function parseVerification(m: Message): VerificationInfo | null {
  try {
    const c = JSON.parse(m.content)
    if (typeof c !== 'object' || c === null || typeof c.passed !== 'boolean') return null
    return c as VerificationInfo
  } catch {
    return null
  }
}

export interface TouchedFile {
  path: string
  kind: 'read' | 'edited' | 'created' | 'deleted'
  added?: number | null
  removed?: number | null
}

export interface GitRow {
  mark: string
  path: string
  weight: number // 0..1 relative change weight for the bar
}

export interface GitWorkingTree {
  branch: string
  added: number
  removed: number
  files: number
  rows: GitRow[]
}

export interface SessionStats {
  sessionId: string
  context: {
    segments: ContextSegment[]
    used: number
    limit: number
    percent: number
  }
  cost: {
    rows: CostRow[]
    total: string
  }
  plan: {
    items: PlanItem[]
    progress: string // e.g. '2/5'
  }
  files: TouchedFile[]
  git: GitWorkingTree
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