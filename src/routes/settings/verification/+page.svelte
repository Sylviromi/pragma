<script lang="ts">
  import Toggle from '../../../components/Toggle.svelte'
  import { getVerificationSettings, setVerificationSettings } from '../../../lib/tauri'
  import type { VerificationSettings } from '../../../lib/types'
  import SettingsHeader from '../../../components/SettingsHeader.svelte'

  let s = $state<VerificationSettings>({
    enabled: true,
    runChecks: true,
    independentReview: true,
    maxRounds: 2,
    reviewerModel: null,
    testCommand: null,
  })
  let loaded = $state(false)

  $effect(() => {
    getVerificationSettings().then((v) => {
      s = v
      loaded = true
    })
  })

  function patch(p: Partial<VerificationSettings>) {
    s = { ...s, ...p }
    setVerificationSettings(s)
  }
</script>

<div class="page">
  <SettingsHeader
    kicker="Quality"
    title="Verification"
    blurb="When the agent says it's done, the harness runs an independent check before the turn closes: the project's real checks plus a fresh, isolated review of the diff."
  />

  <div class="card">
    <div class="row">
      <div class="row-text">
        <div class="label">Verify finished turns</div>
        <div class="desc">Master switch — off means the loop ends exactly as before (no checks, no review)</div>
      </div>
      <Toggle
        label="Verify finished turns"
        bind:checked={s.enabled}
        onChange={(v) => patch({ enabled: v })}
      />
    </div>
    <div class="row">
      <div class="row-text">
        <div class="label">Run project checks</div>
        <div class="desc">Deterministic — runs the project's test/check command (cargo test, npm test…) after the agent finishes. No model involved, zero tokens</div>
      </div>
      <Toggle
        label="Run project checks"
        bind:checked={s.runChecks}
        disabled={!s.enabled}
        onChange={(v) => patch({ runChecks: v })}
      />
    </div>
    <div class="row">
      <div class="row-text">
        <div class="label">Independent review</div>
        <div class="desc">One small model call with only the task + diff + check output (never the conversation). Costs a few thousand tokens per finished turn</div>
      </div>
      <Toggle
        label="Independent review"
        bind:checked={s.independentReview}
        disabled={!s.enabled}
        onChange={(v) => patch({ independentReview: v })}
      />
    </div>
    <div class="row">
      <div class="row-text">
        <div class="label">Fix rounds</div>
        <div class="desc">When verification fails, how many targeted fix attempts the agent gets before the report is surfaced to you (0 = report only)</div>
      </div>
      <input
        class="num"
        type="number"
        min="0"
        max="5"
        step="1"
        bind:value={s.maxRounds}
        disabled={!s.enabled}
        onchange={() => patch({ maxRounds: Math.max(0, Math.min(5, Number(s.maxRounds) || 0)) })}
        aria-label="Fix rounds"
      />
    </div>
  </div>

  <div class="card">
    <div class="row">
      <div class="row-text">
        <div class="label">Test command override</div>
        <div class="desc">Leave empty to auto-detect (Cargo.toml → cargo check &amp;&amp; cargo test; package.json → npm test / npm run check)</div>
      </div>
      <input
        class="text"
        type="text"
        placeholder="auto-detect"
        value={s.testCommand ?? ''}
        disabled={!s.enabled || !s.runChecks}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLInputElement).value.trim()
          patch({ testCommand: v ? v : null })
        }}
        aria-label="Test command override"
      />
    </div>
    <div class="row">
      <div class="row-text">
        <div class="label">Reviewer model</div>
        <div class="desc">Model used for the independent review (same provider). Empty = same model as the session</div>
      </div>
      <input
        class="text"
        type="text"
        placeholder="same as session"
        value={s.reviewerModel ?? ''}
        disabled={!s.enabled || !s.independentReview}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLInputElement).value.trim()
          patch({ reviewerModel: v ? v : null })
        }}
        aria-label="Reviewer model"
      />
    </div>
  </div>

  {#if loaded && !s.enabled}
    <div class="hint">Verification is off — finished turns close as before.</div>
  {/if}
</div>

<style>
  .card {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    overflow: hidden;
    max-width: 640px;
    margin-bottom: 16px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .row:last-child {
    border-bottom: none;
  }
  .row-text {
    min-width: 0;
  }
  .label {
    color: var(--text-primary);
    font-size: 13.5px;
  }
  .desc {
    color: var(--text-muted);
    font-size: 12px;
    margin-top: 1px;
    max-width: 420px;
  }
  .num {
    width: 56px;
    background: var(--bg-input, var(--bg-base));
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm, 4px);
    color: var(--text-primary);
    padding: 6px 8px;
    font-size: 13px;
    text-align: center;
  }
  .text {
    width: 240px;
    background: var(--bg-input, var(--bg-base));
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm, 4px);
    color: var(--text-primary);
    padding: 6px 10px;
    font-size: 12.5px;
  }
  .text::placeholder {
    color: var(--text-muted);
    opacity: 0.7;
  }
  .num:disabled,
  .text:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .hint {
    color: var(--text-muted);
    font-size: 12px;
    max-width: 640px;
  }
</style>
