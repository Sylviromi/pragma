import {
  runAgenticLoop,
  getRunningSessionIds,
  listMessages as fetchMessages,
  approveTool as apiApprove,
  denyTool as apiDeny,
  interruptSession,
  answerQuestion as apiAnswerQuestion,
  revertSession
} from '../lib/tauri'
import type { AgentEvent, Message, RevertSummary, ThinkingPref, VerificationInfo } from '../lib/types'
import { sessions } from './sessions.svelte'
import { pushToast } from './toast.svelte'

export interface ToolItem {
  toolUseId: string
  name: string
  input: Record<string, unknown>
  output?: string
  isError?: boolean
  /** waiting = needs user approval, running = approved/executing, done/error = finished */
  status: 'waiting' | 'running' | 'done' | 'error'
}

/** A message queued behind a still-running loop (post-reload reconnect). */
interface PendingMessage {
  content: string
  model: string
  provider: string
  workingDir: string
  autoApprove: boolean
  thinking: ThinkingPref
  /** id of the optimistic user bubble shown while queued — swapped for a
   *  fresh one when the message actually sends. */
  optimisticId: string
}

/** A block in the live (in-progress) assistant turn — mirrors the ContentBlock
 *  order of the persisted message so the streamed view looks exactly like the
 *  completed one (thoughts separate, in place; tools interleaved). */
type LiveBlock =
  | { type: 'thinking'; text: string; done: boolean }
  | { type: 'text'; text: string }
  | { type: 'tool'; toolUseId: string }
  | { type: 'question'; questionId: string; text: string; options: string[]; picked?: string }

// Read-only tools: auto-approve without asking
const SAFE_TOOLS = new Set([
  'read_file', 'list_files', 'search_files',
  'git_status', 'git_diff', 'git_log',
  'web_search', 'fetch_url',
  'load_skill',
  // change_directory is harmless (sets the session's working dir) and is
  // required for the auto-resolution flow to proceed without friction.
  'change_directory',
])

export type AgentStatus = 'idle' | 'thinking' | 'working' | 'generating' | 'waiting' | 'verifying'

/** Design status meta shared by TabBar dots + sidebar sessions list. */
export const STATUS_META: Record<AgentStatus, { label: string; color: string }> = {
  idle:       { label: 'Idle', color: 'var(--text-muted)' },
  thinking:   { label: 'Thinking', color: 'var(--slot)' },
  working:    { label: 'Working', color: 'var(--warning)' },
  generating: { label: 'Generating', color: 'var(--accent)' },
  waiting:    { label: 'Waiting for you', color: 'var(--danger)' },
  verifying:  { label: 'Verifying', color: 'var(--accent)' },
}

export const chat = $state({
  messages:       {} as Record<string, Message[]>,
  streamingToken: {} as Record<string, string | null>,
  liveBlocks:     {} as Record<string, LiveBlock[]>,
  toolItems:      {} as Record<string, ToolItem[]>,
  running:        {} as Record<string, boolean>,
  error:          {} as Record<string, string | null>,
  /** Neutral notice (e.g. "Turn interrupted by user") — rendered like error, less alarming. */
  notice:         {} as Record<string, string | null>,
  alwaysApprove:  {} as Record<string, Set<string>>,
  autoApprove:    {} as Record<string, boolean>,
  /** Per-session thinking preference: 'off' forces thinking off (cost control);
   *  'auto' uses the model's default level; 'low'/'medium'/'high' request a
   *  graduated thinking level where the model supports levels. Default 'auto'. */
  thinking:       {} as Record<string, ThinkingPref>,
  /** When the current run started (ms epoch) — drives the live indicator's
   *  elapsed timer. Cleared in finishTurn. */
  startedAt:      {} as Record<string, number>,
  /** Aggregate session status for the design's status dots / labels. */
  status:         {} as Record<string, AgentStatus>,
  /** Live verification phase while the harness is checking/reviewing. */
  verifying:      {} as Record<string, 'checks' | 'review' | null>,
  /** Last verification phase result (live-only — the persisted bubble comes
   *  from the role='verification' message after 'done'). Cleared at the next
   *  sendMessage. */
  verification:   {} as Record<string, VerificationInfo | null>,
  /** A message the user sent while a previous loop was still running (e.g.
   *  after a reload re-attached to a live loop). Auto-sent when that loop
   *  reports 'done' — never silently dropped. Survives HMR via live state. */
  pending:        {} as Record<string, PendingMessage | null>,
})

/** Turn-completion notices: in-app toast only. Native OS notifications live on
 *  the Rust side (src-tauri/src/notify.rs) so they fire even when this frontend
 *  module is busy; they are focus-gated there too — this toast covers the
 *  focused-window case. */
function notifyTurn(sessionId: string, kind: 'done' | 'error' | 'waiting' | 'question', detail?: string) {
  const name = sessions.list.find((s) => s.id === sessionId)?.name ?? 'Session'
  const trimmed = detail ? (detail.length > 200 ? detail.slice(0, 197) + '…' : detail) : ''
  if (kind === 'error') {
    pushToast(`Error — ${name}: ${trimmed}`, 'error')
  } else if (kind === 'question') {
    pushToast(`${name} — needs your answer: ${trimmed || 'the agent asked a question'}`, 'info')
  } else if (kind === 'waiting') {
    pushToast(`${name} — needs approval: ${trimmed || 'a tool is waiting for review'}`, 'info')
  } else {
    pushToast(`Done — ${name}`, 'success')
  }
}

/** End a turn's transient state: stop the spinner and detach the event
 *  listener (if any). The streamed live blocks stay visible until the next
 *  sendMessage (which clears them) or revertToMessage. */
function finishTurn(sessionId: string) {
  chat.running[sessionId] = false
  chat.status[sessionId] = 'idle'
  chat.streamingToken[sessionId] = null
  chat.startedAt[sessionId] = 0
}

function handleEvent(sessionId: string, ev: AgentEvent) {
  if (ev.event === 'reasoning') {
    // Streaming thinking tokens. Append to the open thought block — or open a
    // new one (mirrors ContentBlock::Thinking separation in persisted form).
    const blocks = chat.liveBlocks[sessionId] ?? []
    const last = blocks.length > 0 ? blocks[blocks.length - 1] : null
    if (last?.type === 'thinking' && !last.done) {
      chat.liveBlocks[sessionId] = [
        ...blocks.slice(0, -1),
        { type: 'thinking', text: last.text + ev.content, done: false },
      ]
    } else {
      chat.liveBlocks[sessionId] = [...blocks, { type: 'thinking', text: ev.content, done: false }]
    }
    chat.status[sessionId] = 'thinking'

  } else if (ev.event === 'thinking_end') {
    // Seal the thought block so the NEXT reasoning chunk opens a new one
    // (multiple thoughts are rendered separated, like the persisted message).
    const blocks = chat.liveBlocks[sessionId] ?? []
    const last = blocks.length > 0 ? blocks[blocks.length - 1] : null
    if (last?.type === 'thinking' && !last.done) {
      chat.liveBlocks[sessionId] = [...blocks.slice(0, -1), { ...last, done: true }]
    }

  } else if (ev.event === 'token') {
    chat.streamingToken[sessionId] = (chat.streamingToken[sessionId] ?? '') + ev.content
    chat.status[sessionId] = 'generating'

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
    // ask_question is handled by the `question` event ("Needs your answer"
    // block) — never show a tool card or an approval prompt for it.
    if (ev.name === 'ask_question') {
      return
    }
    const alwaysSet = chat.alwaysApprove[sessionId] ?? new Set<string>()
    const autoApprove = chat.autoApprove[sessionId] ?? false
    const shouldAutoApprove = autoApprove || SAFE_TOOLS.has(ev.name) || alwaysSet.has(ev.name)
    chat.status[sessionId] = shouldAutoApprove ? 'working' : 'waiting'
    if (!shouldAutoApprove) notifyTurn(sessionId, 'waiting', ev.name)

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
    chat.status[sessionId] = 'generating'

  } else if (ev.event === 'question') {
    // The model asked a question — the loop is paused until the user answers
    // (answer_question). Render the "Needs your answer" block.
    chat.status[sessionId] = 'waiting'
    notifyTurn(sessionId, 'question', ev.text)
    chat.liveBlocks[sessionId] = [
      ...(chat.liveBlocks[sessionId] ?? []),
      { type: 'question', questionId: ev.questionId, text: ev.text, options: ev.options },
    ]

  } else if (ev.event === 'working_dir_changed') {
    sessions.list = sessions.list.map(s =>
      s.id === sessionId ? { ...s, workingDir: ev.newPath } : s
    )

  } else if (ev.event === 'interrupted') {
    chat.notice[sessionId] = 'Turn interrupted by user.'
    chat.pending[sessionId] = null // respect the stop — don't auto-send later
    finishTurn(sessionId)
    loadMessages(sessionId)

  } else if (ev.event === 'error') {
    let msg = ev.message
    try {
      const parsed = JSON.parse(msg)
      if (parsed?.error?.message) msg = parsed.error.message
    } catch { /* keep raw */ }
    if (!msg) msg = 'Model request failed (no error details provided).'
    chat.error[sessionId] = msg
    notifyTurn(sessionId, 'error', msg)
    finishTurn(sessionId)
    loadMessages(sessionId)
    // A queued message still deserves its turn — the lock was released.
    void flushPending(sessionId)

  } else if (ev.event === 'verification_started') {
    // Harness verification phase (checks → review). Show the phase live.
    chat.verifying[sessionId] = ev.phase
    chat.status[sessionId] = 'verifying'

  } else if (ev.event === 'verification_result') {
    chat.verifying[sessionId] = null
    chat.verification[sessionId] = {
      passed: ev.passed,
      cancelled: ev.cancelled,
      summary: ev.summary,
      findings: ev.findings,
      rounds: ev.rounds,
      tokens: ev.tokens,
      durationMs: ev.durationMs,
      checksLabel: ev.checksLabel,
      checksRan: ev.checksLabel !== null,
      reviewUsed: ev.tokens > 0,
    }
    chat.status[sessionId] = 'verifying'

  } else if (ev.event === 'done') {
    // Keep the streamed reasoning — the live thought card collapses to a chip.
    notifyTurn(sessionId, 'done')
    finishTurn(sessionId)
    loadMessages(sessionId)
    // Send any message the user queued while this loop was still running.
    void flushPending(sessionId)
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

/** User picked an option in the "Needs your answer" block — resolve the
 *  pending question on the backend (resumes the paused agentic loop) and
 *  mark the block as answered in the live turn. */
export async function answerQuestion(sessionId: string, questionId: string, option: string) {
  // Mark the live block answered immediately (optimistic UI) …
  const blocks = chat.liveBlocks[sessionId] ?? []
  chat.liveBlocks[sessionId] = blocks.map((b) =>
    b.type === 'question' && b.questionId === questionId ? { ...b, picked: option } : b
  )
  // … then resume the backend loop.
  try {
    await apiAnswerQuestion(questionId, option)
    pushToast('Answer sent — continuing', 'success')
  } catch (err) {
    pushToast(String(err), 'error')
  }
}

export async function sendMessage(
  sessionId: string,
  content: string,
  model: string,
  provider: string,
  workingDir: string,
  autoApprove: boolean,
  thinking: ThinkingPref = 'auto'
) {
  // Persist autoApprove in store so handleEvent can access it after HMR
  chat.autoApprove[sessionId] = autoApprove
  chat.thinking[sessionId] = thinking

  // A previous turn is still running on the backend (e.g. a second message
  // sent while the first loop was still going). Queue the message and let
  // that turn finish — never wipe its live bubbles (the thinking bubble the
  // user is watching), never silently drop the text. flushPending() in the
  // 'done' handler sends it automatically afterwards.
  const runningNow = await getRunningSessionIds()
  if (runningNow.includes(sessionId)) {
    const queued: Message = {
      id: `optimistic-${Date.now()}`,
      sessionId,
      role: 'user',
      content: JSON.stringify(content),
      model,
      provider,
      timestamp: new Date().toISOString(),
      tokens: 0,
    }
    chat.messages[sessionId] = [...(chat.messages[sessionId] ?? []), queued]
    chat.pending[sessionId] = { content, model, provider, workingDir, autoApprove, thinking, optimisticId: queued.id }
    pushToast('A previous turn is still running — your message is queued and will send when it finishes')
    return
  }

  // Clear live turn state from the previous run
  chat.running[sessionId] = true
  chat.status[sessionId] = 'thinking'
  chat.startedAt[sessionId] = Date.now()
  chat.streamingToken[sessionId] = ''
  chat.liveBlocks[sessionId] = []
  chat.toolItems[sessionId] = []
  chat.error[sessionId] = null
  chat.notice[sessionId] = null
  chat.verification[sessionId] = null
  chat.verifying[sessionId] = null
  chat.pending[sessionId] = null // a stale queue entry must not fire later

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
  // Run token: the startedAt this sendMessage owns. The `finally` below only
  // finishes the turn if no newer turn replaced it — the 'done'/'error'/
  // 'interrupted' events and flushPending's follow-up sendMessage all
  // overwrite startedAt, so a stale finally can never clobber a live loop's
  // state (running=false, status='idle', streamed text cleared).
  const runStartedAt = chat.startedAt[sessionId]

  try {
    await runAgenticLoop(sessionId, content, model, provider, workingDir, autoApprove, thinking, (ev) => handleEvent(sessionId, ev))
  } catch (err) {
    const msg = String(err)
    if (msg.includes('already has an active agentic loop')) {
      // Only trust this if the backend actually reports a live loop in memory.
      // A DB-only lock (crashed instance, stale row) means the message must
      // NOT be sent yet — the lock is reclaimed once the heartbeat goes stale.
      const live = await getRunningSessionIds()
      if (!live.includes(sessionId)) {
        // No live loop: a crashed instance still holds the cross-process lock
        // (heartbeat cooling down, up to ~2 min). Be honest about it and do
        // NOT claim the message was sent.
        chat.running[sessionId] = false
        chat.streamingToken[sessionId] = null
        chat.error[sessionId] =
          'A previous (crashed) run still holds this session\u2019s lock — it is released automatically within about 2 minutes. ' +
          'Your message was not sent; try again shortly or use another session.'
        loadMessages(sessionId)
        return
      }
      // A loop IS still running (leftover from before an HMR reload, or a
      // second app instance) — raced between our check and the invoke. Queue
      // instead of dropping: the optimistic message stays visible as queued.
      chat.pending[sessionId] = { content, model, provider, workingDir, autoApprove, thinking, optimisticId: optimistic.id }
      pushToast('A previous turn is still running — your message is queued and will send when it finishes')
      keepRunning = true
      chat.running[sessionId] = true
    } else {
      chat.error[sessionId] = msg || 'Model request failed (no error details provided).'
    }
  } finally {
    // Only reset the running flag if this call owned the loop. If we hit the
    // "already running" branch above, the OTHER loop's events own the state.
    // Reasoning is preserved so the thought stays visible after the turn ends.
    if (!keepRunning && chat.startedAt[sessionId] === runStartedAt) {
      finishTurn(sessionId)
      loadMessages(sessionId)
    }
  }
}

/** Send a message that was queued behind a still-running loop. Called from the
 *  'done'/'error' handlers — after the previous turn released the loop lock. */
async function flushPending(sessionId: string) {
  const pending = chat.pending[sessionId]
  if (!pending) return
  chat.pending[sessionId] = null
  // Let the DB state land first, then swap the queued optimistic bubble for a
  // fresh one from sendMessage (the queued one was never persisted).
  await loadMessages(sessionId)
  chat.messages[sessionId] = (chat.messages[sessionId] ?? []).filter(m => m.id !== pending.optimisticId)
  try {
    await sendMessage(
      sessionId,
      pending.content,
      pending.model,
      pending.provider,
      pending.workingDir,
      pending.autoApprove,
      pending.thinking
    )
  } catch {
    chat.error[sessionId] = 'Queued message failed to send.'
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
  chat.error[sessionId] = null
  chat.notice[sessionId] = null
  chat.liveBlocks[sessionId] = []
  chat.toolItems[sessionId] = []
  finishTurn(sessionId)
  await loadMessages(sessionId)
  return summary
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
