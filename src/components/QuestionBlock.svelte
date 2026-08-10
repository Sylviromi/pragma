<script lang="ts">
  import Icon from './Icon.svelte'

  // The design's "Needs your answer" block (`Pragma UI Kit` → buildBlock kind
  // 'question'): rendered from both the live `question` agent event (picking
  // calls answerQuestion → resumes the paused loop) and from a persisted
  // ask_question tool_use in a completed message (answer recovered from
  // "User answered: …"). Pending = warning border + thinkGlow + numbered
  // chips + hint; answered = success icon + answer pill + subtle border.
  let {
    text,
    options,
    answer = null,
    onSelect,
  }: {
    text: string
    options: string[]
    answer?: string | null
    onSelect?: (option: string) => void
  } = $props()

  const answered = $derived(answer !== null && answer !== undefined && answer !== '')
  const pending = $derived(!answered)

  const chip = $derived.by(() =>
    options.map((label, oi) => ({
      label,
      badge: String(oi + 1),
      picked: answer === label,
    }))
  )

  // Free-text fallback: questions without options (e.g. "which folder?") get a
  // text input instead of chips; the answer flows through onSelect →
  // answerQuestion the same way a chip pick does.
  let draft = $state('')
  function submitFreeText() {
    if (!draft.trim() || answer) return
    onSelect?.(draft.trim())
  }

  // Recover the picked option from an ask_question tool result free text
  // (the loop writes "User answered: <answer>").
  function qAnswer(out: string | undefined): string | null {
    if (!out) return null
    const m = /^User answered: (.+)$/s.exec(out.trim())
    return m ? m[1] : null
  }
</script>

<div class="question" class:pending class:answered>
  <div class="q-head">
    <span class="q-ico" aria-hidden="true"><Icon name={answered ? 'task_alt' : 'help'} size={16} /></span>
    <span class="q-label">{answered ? 'Answered' : 'Needs your answer'}</span>
    <span class="q-pill" title={answered ? answer : undefined}>
      {answered ? answer : 'run paused'}
    </span>
  </div>
  <div class="q-text">{text}</div>
  {#if options.length > 0}
    <div class="q-options">
      {#each chip as c (c.label)}
        <button
          class="chip"
          class:picked={c.picked}
          disabled={answered}
          onclick={() => pending && onSelect?.(c.label)}
        >
          <span class="badge" class:picked={c.picked}>{c.badge}</span>
          <span class="chip-label">{c.label}</span>
          {#if c.picked}<span class="check" aria-hidden="true"><Icon name="check" size={15} /></span>{/if}
        </button>
      {/each}
    </div>
    {#if pending}<div class="q-hint">Pick one, press {options.length === 1 ? '1' : `1–${options.length}`}, or type your own answer below.</div>{/if}
  {:else if answered}
    <div class="q-answer">
      <Icon name="check" size={14} />
      <span>{answer}</span>
    </div>
  {:else}
    <div class="q-input">
      <input
        class="free"
        type="text"
        bind:value={draft}
        placeholder="Type your answer…"
        aria-label="Answer"
        onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); submitFreeText() } }}
      />
      <button class="chip send" onclick={submitFreeText} disabled={!draft.trim()}>Send</button>
    </div>
  {/if}
</div>

<style>
  /* Design: warning-bordered card while pending (thinkGlow), success-pill "run
     paused / Answered" header, mono label, numbered pill chips. */
  .question {
    background: var(--bg-elevated);
    border: 1px solid var(--warning);
    border-radius: 5px 18px 18px 18px;
    padding: 13px 16px 14px;
    display: flex;
    flex-direction: column;
    gap: 11px;
    transition: border-color 0.35s ease;
    animation: bubbleIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both;
  }
  /* Pending: breathing warning glow; answered: quiet subtle border. */
  .question.pending {
    animation: bubbleIn 0.34s cubic-bezier(0.22, 0.8, 0.3, 1) both,
      thinkGlow 2.4s ease-in-out infinite;
  }
  .question.answered {
    border-color: var(--border-subtle);
  }
  .q-head {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }
  .q-ico {
    flex: none;
    display: flex;
    color: var(--warning);
  }
  .question.answered .q-ico {
    color: var(--success);
  }
  .q-ico :global(.msr) {
    font-size: 16px;
    line-height: 1;
  }
  .question.pending .q-ico {
    animation: dotPulse 1.6s ease-in-out infinite;
  }
  .q-label {
    flex: none;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--warning);
  }
  .question.answered .q-label {
    color: var(--text-muted);
  }
  .q-pill {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    background: var(--bg-overlay);
    border-radius: 9px;
    padding: 2px 9px;
    white-space: nowrap;
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .q-text {
    font-family: var(--font-body);
    font-size: 14px;
    color: var(--text-primary);
    line-height: 1.55;
  }
  .q-options {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  /* Pill chips with mono number badges (design opt.style). */
  .chip {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 7px 13px 7px 8px;
    border-radius: 16px;
    cursor: pointer;
    font-family: var(--font-body);
    font-size: 12.5px;
    border: 1px solid var(--border-default);
    background: transparent;
    color: var(--text-primary);
    transition: border-color 0.2s ease, background 0.2s ease, color 0.2s ease;
  }
  .chip:hover:not(:disabled) {
    border-color: var(--warning);
  }
  .chip.picked {
    border-color: var(--success);
    background: color-mix(in oklch, var(--success) 16%, transparent);
    color: var(--text-primary);
  }
  .chip:disabled {
    cursor: default;
    color: var(--text-muted);
    border-color: var(--border-subtle);
  }
  .chip.picked:disabled {
    color: var(--text-primary);
    border-color: var(--success);
    background: color-mix(in oklch, var(--success) 16%, transparent);
  }
  /* Unpicked chips dim once answered (design: opacity 0.5). */
  .question.answered .chip:not(.picked) {
    opacity: 0.5;
  }
  .badge {
    width: 17px;
    height: 17px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--text-muted);
    background: var(--bg-overlay);
  }
  .badge.picked {
    color: var(--success);
  }
  .chip-label {
    white-space: nowrap;
  }
  .check {
    display: flex;
    flex: none;
  }
  .check :global(.msr) {
    color: var(--success);
  }
  .q-hint {
    font-family: var(--font-body);
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .q-input {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .q-input .free {
    flex: 1;
    min-width: 0;
    background: var(--bg-base);
    border: 1px solid var(--border-default);
    border-radius: 5px;
    padding: 7px 11px;
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12.5px;
  }
  .q-input .free:focus {
    outline: none;
    border-color: var(--accent);
  }
  .q-input .send:disabled {
    opacity: 0.5;
  }
  .q-answer {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--font-mono);
    font-size: 12.5px;
    color: var(--text-primary);
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: 5px;
    padding: 8px 12px;
  }
  .q-answer :global(.msr) {
    color: var(--success);
    font-size: 14px;
  }
</style>