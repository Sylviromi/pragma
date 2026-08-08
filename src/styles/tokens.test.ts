import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';

const dark = readFileSync('src/styles/dark.css', 'utf-8');
const required = [
  '--color-bg-base',
  '--color-bg-elevated',
  '--color-bg-overlay',
  '--color-text-primary',
  '--color-text-secondary',
  '--color-text-muted',
  '--color-text-accent',
  '--color-border-default',
  '--color-border-subtle',
  '--color-border-focus',
  '--color-accent-brand',
  '--color-accent-success',
  '--color-accent-warning',
  '--color-accent-danger',
  '--radius-sm',
  '--radius-md',
  '--radius-lg',
  '--spacing-1',
  '--spacing-2',
  '--spacing-3',
  '--spacing-4',
  '--spacing-6',
  '--font-sans',
  '--font-mono'
];

describe('CSS token system', () => {
  it('dark.css defines all required tokens', () => {
    for (const t of required) expect(dark).toContain(t);
  });
});