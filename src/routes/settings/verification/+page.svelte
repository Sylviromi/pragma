<script lang="ts">
  import Toggle from '../../../components/Toggle.svelte';
  import Card from '../../../components/Card.svelte';
  import SettingRow from '../../../components/SettingRow.svelte';
  import { getVerificationSettings, setVerificationSettings } from '../../../lib/tauri';
  import type { VerificationSettings } from '../../../lib/types';
  import SettingsHeader from '../../../components/SettingsHeader.svelte';

  let s = $state<VerificationSettings>({
    enabled: true,
    runChecks: true,
    independentReview: true,
    maxRounds: 2,
    reviewerModel: null,
    testCommand: null,
  });
  let loaded = $state(false);

  $effect(() => {
    getVerificationSettings().then((v) => {
      s = v;
      loaded = true;
    });
  });

  function patch(p: Partial<VerificationSettings>) {
    s = { ...s, ...p };
    setVerificationSettings(s);
  }
</script>

<div class="page">
  <SettingsHeader
    kicker="Quality"
    title="Verification"
    blurb="When the agent says it's done, the harness runs an independent check before the turn closes: the project's real checks plus a fresh, isolated review of the diff."
  />

  <Card>
    <SettingRow
      label="Verify finished turns"
      desc="Master switch — off means the loop ends exactly as before (no checks, no review)"
    >
      <Toggle
        label="Verify finished turns"
        bind:checked={s.enabled}
        onChange={(v) => patch({ enabled: v })}
      />
    </SettingRow>
    <SettingRow
      label="Run project checks"
      desc="Deterministic — runs the project's test/check command (cargo test, npm test…) after the agent finishes. No model involved, zero tokens"
    >
      <Toggle
        label="Run project checks"
        bind:checked={s.runChecks}
        disabled={!s.enabled}
        onChange={(v) => patch({ runChecks: v })}
      />
    </SettingRow>
    <SettingRow
      label="Independent review"
      desc="One small model call with only the task + diff + check output (never the conversation). Costs a few thousand tokens per finished turn"
    >
      <Toggle
        label="Independent review"
        bind:checked={s.independentReview}
        disabled={!s.enabled}
        onChange={(v) => patch({ independentReview: v })}
      />
    </SettingRow>
    <SettingRow
      label="Fix rounds"
      desc="When verification fails, how many targeted fix attempts the agent gets before the report is surfaced to you (0 = report only)"
    >
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
    </SettingRow>
  </Card>

  <Card style="margin-bottom: 16px">
    <SettingRow
      label="Test command override"
      desc="Leave empty to auto-detect (Cargo.toml → cargo check && cargo test; package.json → npm test / npm run check)"
    >
      <input
        class="text"
        type="text"
        placeholder="auto-detect"
        value={s.testCommand ?? ''}
        disabled={!s.enabled || !s.runChecks}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLInputElement).value.trim();
          patch({ testCommand: v ? v : null });
        }}
        aria-label="Test command override"
      />
    </SettingRow>
    <SettingRow
      label="Reviewer model"
      desc="Model used for the independent review (same provider). Empty = same model as the session"
    >
      <input
        class="text"
        type="text"
        placeholder="same as session"
        value={s.reviewerModel ?? ''}
        disabled={!s.enabled || !s.independentReview}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLInputElement).value.trim();
          patch({ reviewerModel: v ? v : null });
        }}
        aria-label="Reviewer model"
      />
    </SettingRow>
  </Card>

  {#if loaded && !s.enabled}
    <div class="hint">Verification is off — finished turns close as before.</div>
  {/if}
</div>

<style>
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
