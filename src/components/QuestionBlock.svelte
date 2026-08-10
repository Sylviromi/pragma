<script lang="ts">
  import Icon from './Icon.svelte'

  // The design's "Needs your answer" block: rendered from both the live
  // `question` agent event (picking calls answerQuestion → resumes the paused
  // loop) and from a persisted ask_question tool_use in a completed message
  // (answer recovered from "User answered: …").
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

<div class="question">
  <div class="q-head">
    <Icon name="help" size={14} />
    <span>Needs your answer</span>
  </div>
  <div class="q-text">{text}</div>
  {#if options.length > 0}
    <div class="q-options">
      {#each options as opt (opt)}
        <button
          class="chip"
          class:picked={answer === opt}
          onclick={() => !answer && onSelect?.(opt)}
          disabled={!!answer}
        >{opt}</button>
      {/each}
    </div>
  {:else if answer}
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
  /* Design: elevated card with accent border, uppercase mono label, pill chips. */
  .question {
    background: var(--bg-elevated);
    border: 1px solid var(--accent);
    border-radius: 5px 18px 18px 18px;
    padding: 12px 16px 13px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .q-head {
    display: flex;
    align-items: center;
    gap: 7px;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .q-head :global(.msr) {
    font-size: 14px;
    line-height: 1;
  }
  .q-text {
    font-family: var(--font-body);
    font-size: 14px;
    color: var(--text-primary);
    line-height: 1.5;
  }
  .q-options {
    display: flex;
    flex-wrap: wrap;
    gap: 7px;
  }
  .chip {
    padding: 6px 13px;
    border-radius: 14px;
    cursor: pointer;
    font-family: var(--font-body);
    font-size: 12.5px;
    border: 1px solid var(--border-default);
    background: transparent;
    color: var(--text-primary);
    transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
  }
  .chip:hover:not(:disabled) {
    border-color: var(--accent);
  }
  .chip.picked {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-on);
  }
  .chip:disabled {
    cursor: default;
    color: var(--text-muted);
    border-color: var(--border-subtle);
  }
  .chip.picked:disabled {
    color: var(--accent-on);
    background: var(--accent);
    border-color: var(--accent);
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