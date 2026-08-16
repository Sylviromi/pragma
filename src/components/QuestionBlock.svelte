<script lang="ts">
  import CornerMarks from './CornerMarks.svelte'
  // Design "Input requested" block (`Pragma UI Kit v2.dc.html` → buildBlock
  // kind 'question'): accent-bordered card with blueprint corner marks, a
  // header (icon + mono label + count + "run paused · k/n" pill), per-question
  // progress segments, and question rows that are choice (A/B/C), scale (1..5)
  // or free-text input. Rendered from the live `question` agent event (single
  // question — picking resolves the paused loop) and from a persisted
  // ask_question tool_use (answer recovered from "User answered: …").
  //
  // The `questions` prop is the future multi-question shape (one block, many
  // rows, single submit) — TODO(agents): wire when the backend sends several
  // questions per pause. Until then it degrades to the single live question.
  export interface QuestionRow {
    id: string
    text: string
    kind: 'choice' | 'scale' | 'input'
    options?: string[]
    placeholder?: string
    unit?: string
    lo?: string
    hi?: string
  }

  let {
    text = '',
    options = [] as string[],
    answer = null as string | null,
    onSelect,
    questions = null as QuestionRow[] | null,
    onSubmit,
  }: {
    /** Intro text shown under the header (the question itself). */
    text?: string
    /** Single-question options (live path) — auto-detects choice vs scale. */
    options?: string[]
    /** Picked / recovered answer for the single-question path. */
    answer?: string | null
    /** Pick callback (single question, live) — resolves the paused loop. */
    onSelect?: (option: string) => void
    /** Future multi-question shape. */
    questions?: QuestionRow[] | null
    /** Future multi-question submit. */
    onSubmit?: (answers: Record<string, string>) => void
  } = $props()

  const answered = $derived(answer !== null && answer !== undefined && answer !== '')

  // Kind detection for the single-question path: all-numeric/short keys with
  // ≥3 options reads as a scale; no options reads as free-text input.
  function detectKind(opts: string[]): 'choice' | 'scale' | 'input' {
    if (opts.length === 0) return 'input'
    if (opts.length >= 3 && opts.every((o) => /^\d+(\.\d+)?(k|m)?$/i.test(o.trim()))) return 'scale'
    return 'choice'
  }

  const rows = $derived.by<QuestionRow[]>(() => {
    if (questions && questions.length > 0) return questions
    return [{ id: 'q1', text, kind: detectKind(options), options, placeholder: 'Type your answer…' }]
  })

  // Multi-question draft answers (only used by the future `questions` shape).
  let drafts = $state<Record<string, string>>({})
  function pick(q: QuestionRow, value: string) {
    drafts = { ...drafts, [q.id]: value }
  }
  const multiFilled = $derived(rows.filter((q) => {
    const v = drafts[q.id]
    return v != null && String(v).trim() !== ''
  }).length)
  const multiComplete = $derived(multiFilled === rows.length)

  const keyFor = (q: QuestionRow, oi: number) =>
    q.kind === 'scale' ? String(q.options?.[oi] ?? oi + 1) : String.fromCharCode(65 + oi)

  // isPicked: single-question path uses `answer`; multi path uses drafts.
  const isPicked = (q: QuestionRow, label: string) =>
    questions ? drafts[q.id] === label : answer === label

  const isAnsweredRow = (q: QuestionRow) => {
    if (!questions) return answered
    const v = drafts[q.id]
    return v != null && String(v).trim() !== ''
  }

  function submit() {
    if (questions) onSubmit?.(drafts)
    // single path: picking already resolved via onSelect
  }
</script>

<div class="question" class:answered>
  <!-- blueprint corner marks: shared, continue the box border (accent while
       pending, border-default once answered) -->
  <CornerMarks color={answered ? 'var(--border-default)' : 'var(--accent)'} />

  <div class="q-head">
    <span class="q-ico" aria-hidden="true"><span class="msr">{answered ? 'task_alt' : 'contact_support'}</span></span>
    <span class="q-label">{answered ? 'Answers recorded' : 'Input requested'}</span>
    <span class="q-count">{rows.length === 1 ? '1 question' : `${rows.length} questions`}</span>
    <span class="q-pill">
      {answered
        ? (rows.length === 1 ? '1/1 answered' : `${multiFilled}/${rows.length} answered`)
        : `run paused · ${rows.length === 1 ? 0 : multiFilled}/${rows.length}`}
    </span>
  </div>

  <!-- progress segments: one 2px bar per question -->
  <div class="q-progress" aria-hidden="true">
    {#each rows as q, qi (q.id)}
      <span class="q-seg" class:on={isAnsweredRow(q)} style={`--i:${qi}`}></span>
    {/each}
  </div>

  {#if text && !questions}
    <div class="q-text">{text}</div>
  {/if}

  <div class="q-rows">
    {#each rows as q, qi (q.id)}
      <div class="q-row" style={`--i:${qi}`} class:first={qi === 0}>
        <div class="q-row-head">
          <span class="q-num">Q{qi + 1}</span>
          <span class="q-row-text">{q.text}</span>
        </div>

        {#if q.kind === 'input'}
          <!-- free-text input (design: caret + field + unit) -->
          <div class="q-input-wrap" class:filled={isAnsweredRow(q)}>
            <span class="q-caret" aria-hidden="true">›</span>
            <input
              class="q-field"
              type="text"
              value={questions ? (drafts[q.id] ?? '') : (answer ?? '')}
              placeholder={q.placeholder ?? 'Type an answer'}
              aria-label={q.text}
              disabled={answered && !questions}
              oninput={(e) => questions && pick(q, (e.target as HTMLInputElement).value)}
              onkeydown={(e) => {
                if (e.key === 'Enter' && !questions) {
                  e.preventDefault()
                  const v = (e.target as HTMLInputElement).value.trim()
                  if (v) onSelect?.(v)
                }
              }}
            />
            {#if q.unit}<span class="q-unit">{q.unit}</span>{/if}
          </div>
        {:else if q.kind === 'scale'}
          <!-- scale: key squares + lo/hi caps -->
          <div class="q-scale">
            <div class="q-scale-keys">
              {#each (q.options ?? []) as label, oi (label)}
                <button
                  class="q-key"
                  class:picked={isPicked(q, label)}
                  aria-pressed={isPicked(q, label)}
                  title={label}
                  disabled={answered && !questions}
                  onclick={() => questions ? pick(q, label) : onSelect?.(label)}
                >{keyFor(q, oi)}</button>
              {/each}
            </div>
            {#if q.lo || q.hi}
              <div class="q-scale-caps"><span>{q.lo ?? ''}</span><span>{q.hi ?? ''}</span></div>
            {/if}
          </div>
        {:else}
          <!-- choice: full-width option rows with letter key column + check -->
          <div class="q-choices">
            {#each (q.options ?? []) as label, oi (label)}
              <button
                class="q-opt"
                class:picked={isPicked(q, label)}
                aria-pressed={isPicked(q, label)}
                disabled={answered && !questions}
                onclick={() => questions ? pick(q, label) : onSelect?.(label)}
              >
                <span class="q-opt-key">{keyFor(q, oi)}</span>
                <span class="q-opt-label">{label}</span>
                {#if isPicked(q, label)}<span class="q-opt-check" aria-hidden="true"><span class="msr">check</span></span>{/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  </div>

  <div class="q-foot">
    <span class="q-hint">
      {answered
        ? 'Answers sent — the run resumed from here.'
        : questions
          ? (multiComplete ? 'All set — send the answers back.' : 'Answer every question to resume the run.')
          : 'Answer to resume the run.'}
    </span>
    {#if questions}
      <button
        class="q-submit"
        class:ready={multiComplete}
        disabled={!multiComplete}
        onclick={submit}
      >{multiComplete ? 'Send answers' : 'Send answers'}</button>
    {/if}
  </div>
</div>

<style>
  /* Design: accent-bordered card (rounded 0), blueprint corner marks, mono
     uppercase header, pill, progress segments, Q rows. */
  .question {
    position: relative;
    background: var(--bg-elevated);
    border: 1px solid var(--accent);
    border-radius: 0;
    display: flex;
    flex-direction: column;
    transition: border-color 0.35s ease;
    animation: bubbleIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  .question.answered {
    border-color: var(--border-subtle);
  }
  .q-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 18px;
    border-bottom: 1px solid var(--border-subtle);
    background: color-mix(in oklch, var(--accent) 8%, transparent);
    transition: background 0.35s ease;
  }
  .question.answered .q-head {
    background: transparent;
  }
  .q-ico {
    flex: none;
    display: flex;
    color: var(--accent);
    animation: dotPulse 1.6s ease-in-out infinite;
  }
  .question.answered .q-ico {
    color: var(--text-muted);
    animation: none;
  }
  .q-ico :global(.msr) {
    font-size: 16px;
    line-height: 1;
  }
  .q-label {
    flex: none;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.11em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .question.answered .q-label {
    color: var(--text-muted);
  }
  .q-count {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }
  .q-pill {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    background: var(--bg-overlay);
    border-radius: 2px;
    padding: 2px 8px;
    white-space: nowrap;
  }
  .q-progress {
    display: flex;
    gap: 2px;
    padding: 0 18px;
    margin-top: -1px;
  }
  .q-seg {
    flex: 1;
    height: 2px;
    background: var(--border-subtle);
    transition: background 0.3s ease;
    animation-delay: calc(var(--i) * 60ms);
  }
  .q-seg.on {
    background: var(--accent);
  }
  .question.answered .q-seg.on {
    background: var(--text-muted);
  }
  .q-text {
    padding: 13px 18px 0;
    font-family: var(--font-body);
    font-size: 13.5px;
    line-height: 1.55;
    color: var(--text-secondary);
    text-wrap: pretty;
  }
  .q-rows {
    display: flex;
    flex-direction: column;
  }
  .q-row {
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 14px 18px 15px;
    border-top: 1px solid var(--border-subtle);
    animation: rowIn 0.3s cubic-bezier(0.22, 0.8, 0.3, 1) both;
    animation-delay: calc(var(--i) * 60ms);
  }
  .q-row.first {
    margin-top: 12px;
  }
  .q-row-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .q-num {
    flex: none;
    width: 16px;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }
  .q-row-text {
    font-family: var(--font-body);
    font-size: 13.5px;
    line-height: 1.5;
    color: var(--text-primary);
    text-wrap: pretty;
  }

  /* Choice rows (design opt.style): full-width, letter key column. */
  .q-choices {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin-left: 26px;
  }
  .q-opt {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    text-align: left;
    padding: 0;
    cursor: pointer;
    border: 1px solid var(--border-default);
    background: transparent;
    border-radius: 0;
    transition: border-color 0.18s ease, background 0.18s ease;
  }
  .q-opt:hover:not(:disabled) {
    border-color: var(--accent);
    background: color-mix(in oklch, var(--accent) 8%, transparent);
  }
  .q-opt.picked {
    border-color: var(--accent);
    background: color-mix(in oklch, var(--accent) 12%, transparent);
  }
  .question.answered .q-opt:not(.picked) {
    opacity: 0.45;
  }
  .q-opt-key {
    flex: none;
    width: 26px;
    align-self: stretch;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    border-right: 1px solid var(--border-subtle);
  }
  .q-opt.picked .q-opt-key {
    color: var(--accent);
    border-right-color: var(--accent);
  }
  .q-opt-label {
    flex: 1;
    padding: 8px 10px 8px 0;
    font-family: var(--font-body);
    font-size: 13px;
    line-height: 1.4;
    color: var(--text-primary);
  }
  .q-opt-check {
    display: flex;
    padding-right: 9px;
    color: var(--accent);
  }
  .q-opt-check :global(.msr) {
    font-size: 15px;
    line-height: 1;
  }

  /* Scale (design opt.style narrow): key squares + caps. */
  .q-scale {
    margin-left: 26px;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .q-scale-keys {
    display: flex;
    gap: 5px;
  }
  .q-key {
    min-width: 42px;
    padding: 0 12px;
    height: 40px;
    flex: none;
    cursor: pointer;
    border: 1px solid var(--border-default);
    background: transparent;
    border-radius: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-secondary);
    transition: border-color 0.18s ease, background 0.18s ease;
  }
  .q-key:hover:not(:disabled) {
    border-color: var(--accent);
    background: color-mix(in oklch, var(--accent) 8%, transparent);
  }
  .q-key.picked {
    border-color: var(--accent);
    background: color-mix(in oklch, var(--accent) 16%, transparent);
    color: var(--accent);
  }
  .question.answered .q-key:not(.picked) {
    opacity: 0.45;
  }
  .q-scale-caps {
    display: flex;
    justify-content: space-between;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
  }

  /* Free-text input (design inputWrapStyle). */
  .q-input-wrap {
    margin-left: 26px;
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--border-default);
    background: var(--bg-base);
    padding: 8px 11px;
    transition: border-color 0.2s ease;
  }
  .q-input-wrap:focus-within {
    border-color: var(--accent);
  }
  .q-input-wrap.filled {
    border-color: var(--accent);
  }
  .q-caret {
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1;
    color: var(--text-muted);
  }
  .q-input-wrap.filled .q-caret {
    color: var(--accent);
  }
  .q-field {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    outline: none;
    font-family: var(--font-body);
    font-size: 13px;
    color: var(--text-primary);
  }
  .q-unit {
    flex: none;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-muted);
  }

  /* Footer (design footStyle): hint + send button. */
  .q-foot {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    padding: 12px 18px 13px;
    border-top: 1px solid var(--border-subtle);
    background: var(--bg-overlay);
  }
  .q-hint {
    flex: 1;
    min-width: 0;
    font-family: var(--font-body);
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .q-submit {
    margin-left: auto;
    padding: 7px 16px;
    border-radius: 0;
    border: 1px solid var(--border-default);
    background: transparent;
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: default;
    transition: background 0.2s ease, border-color 0.2s ease;
  }
  .q-submit.ready {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--bg-base);
    cursor: pointer;
  }
  .q-submit.ready:hover {
    filter: brightness(1.1);
  }
</style>
