<script lang="ts">
  import Icon from './Icon.svelte'

  // Skeleton of the design's "Needs your answer" block. Full backend wiring
  // (AgentEvent::Question → loop pause → answer_question command) is tracked
  // in TODO.md — this renders the card and exercises the chip interaction.
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
</script>

<div class="question">
  <div class="q-head">
    <Icon name="help" size={14} />
    <span>Needs your answer</span>
  </div>
  <div class="q-text">{text}</div>
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
</style>