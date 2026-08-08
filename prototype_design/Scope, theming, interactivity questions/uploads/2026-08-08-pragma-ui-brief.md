# Pragma — UI Brief for Claude Design

## What is Pragma

Pragma is a desktop AI coding harness (Tauri 2 app). Think of it as a developer-focused AI chat client — similar in spirit to VS Code or a browser devtools panel, not a consumer chat app. Users are developers who will have it open all day alongside their editor and terminal.

---

## Aesthetic Direction

- **Dark-first**, dense, and precise — a developer tool, not a marketing page
- Closest references: **VS Code sidebar**, **GitHub dark theme**, **Warp terminal**
- Minimal chrome — the conversation should dominate, not the UI frame
- Monospace or near-monospace for code, clean sans-serif for UI
- Subtle, not flashy — no gradients, no shadows that distract, purposeful micro-interactions only
- Feels like something a developer *built for themselves*, not a corporate product

---

## Theme System

- **Multiple themes** — the app ships with at least: Dark (default), Light, and High Contrast
- Themes are fully swappable via CSS custom properties — plugins can ship their own themes
- All colors, borders, radii, and spacing reference design tokens (CSS variables), never hardcoded
- Theme switcher lives in Settings → Appearance
- Respect OS-level `prefers-color-scheme` by default, overridable per user preference

**Suggested design token categories:**
- `--color-bg-*` (base, elevated, overlay)
- `--color-text-*` (primary, secondary, muted, accent)
- `--color-border-*` (default, subtle, focus)
- `--color-accent-*` (brand blue, success green, warning amber, danger red)
- `--radius-*`, `--spacing-*`, `--font-*`

---

## Accessibility

- Full **keyboard navigation** — every interactive element reachable and operable via keyboard
- **ARIA labels** on all icon-only buttons, dropdowns, and toggles
- **Focus rings** — visible, high-contrast, never suppressed with `outline: none` without a replacement
- **Colour contrast** — WCAG AA minimum across all themes, AAA preferred for body text
- **Reduced motion** — respect `prefers-reduced-motion`; all transitions and animations off or simplified
- **Screen reader support** — chat messages have proper roles (`log`, `status`), live regions for streaming tokens
- **Resizable text** — layout does not break at 150% or 200% browser zoom
- **Tab order** matches visual order; no focus traps outside modals

---

## Layout

### Window Structure

Browser-style tab bar across the very top. No sidebar. Sessions live in tabs exactly like browser tabs — you can have many open at once and switch between them.

```
┌─ PRAGMA ─┬─ my-project × ─┬─ rust-game × ─┬─ + ──────── ⚙ Settings  ⧉ Plugins ─┐
│          │  (active tab)   │               │                                       │
├──────────┴─────────────────┴───────────────┴───────────────────────────────────────┤
│  [plugin status bar — e.g. handsfree progress — only visible when a plugin uses it]│
├─────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                     │
│                         conversation area                                           │
│                                                                                     │
├─────────────────────────────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────────────────────────────┐  │
│  │  textarea (auto-grows, max ~6 lines before scrolling)                         │  │
│  └───────────────────────────────────────────────────────────────────────────────┘  │
│  [● model ▾]  │  [plugin controls]  →→→  [@ skills]  [/ cmds]  [Send ↑]            │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

### Tab Bar

- Active tab has a top border accent (brand colour), slightly lighter background than inactive
- Inactive tabs are muted, hover shows close button
- `+` button opens a new session (name prompt or auto-named)
- Right side of tab bar: Settings and Plugins icon buttons
- Tabs overflow horizontally with scroll if many open — no wrapping

### Plugin Status Bar

- Sits between tab bar and conversation, only rendered when a plugin injects into it
- Visually distinct from chat (slight tint, smaller font, monospace)
- Example use: handsfree mode shows current phase + task progress here

### Conversation Area

- Messages are left-aligned for AI, right-aligned for user (chat bubble style, not full-width blocks)
- AI messages: subtle background, left-edge accent bar, avatar icon (small)
- User messages: slightly different background, no avatar
- Code blocks: syntax highlighted, monospace, copy button on hover
- Streaming tokens render in real time — no jank, smooth append
- Token count + model name shown as small muted text below each AI message
- Scroll to bottom on new message unless user has scrolled up (anchor detection)

### Input Area

**Textarea:** auto-resizes from ~1 line to ~6 lines, then scrolls internally. Full width.

**Input footer row** (below textarea):
- Left: **model picker chip** `[● claude-sonnet-4-6  Anthropic ▾]` — clicking opens the model dropdown
- Middle: **plugin-injected controls** (e.g. Build/Plan toggle from Planner plugin, Handsfree button) — separated from model picker by a subtle vertical divider
- Right: `@ skills`, `/ commands` text buttons, then `Send` primary button

### Model Picker Dropdown

Opens *upward* from the chip (so it doesn't obscure the input). Grouped by provider:

```
┌───────────────────────────────────────┐
│  🔍 Search models...                  │
├───────────────────────────────────────┤
│  ANTHROPIC                            │
│    ● claude-sonnet-4-6   200k  ✓     │  ← currently selected
│    ● claude-opus-4-6     200k         │
│  DEEPSEEK                             │
│    ● deepseek-v4-flash   64k          │
│  ZEN WARP · plugin                    │  ← plugin-contributed group
│    ● deepseek-v4 via warp   free      │
│    ● mimo-v2.5 via warp   vision      │
│  OPENCODE · local                     │
│    ● qwen3-32b           128k         │
└───────────────────────────────────────┘
```

Native providers and plugin providers use the same row design. Plugin groups are labelled `· plugin` or `· local` to indicate their origin. Currently selected model has a checkmark.

---

## Settings Screen

Two-column layout: fixed left nav, scrollable right content area.

### Left Nav

```
SETTINGS
  General
  Providers
  Skills
  Plugins
  Shortcuts
  Appearance

PLUGINS                    ← this section only appears when plugins are installed
  Zen WARP                 ← each installed plugin gets its own settings nav entry
  Handsfree
```

### Plugins Page (Settings → Plugins)

Each plugin is a card. Cards can be expanded/collapsed.

**Collapsed card:** icon · name · version · contribution badges (colour-coded: green=provider, blue=slot, purple=command) · enable/disable toggle · expand chevron

**Expanded card adds:**
- List of contributions with slot/provider names
- File list
- Status indicator (loaded ✓ / error ✕) + hot-reload badge
- Action links: Open settings · Reload · Remove

Plugin-contributed settings screens render in the right content area when selected from the nav — they are Web Components injected by the plugin, styled to match the host theme via CSS custom properties.

---

## Component Inventory (things Claude Design should generate)

1. Tab bar with active/inactive/overflow states
2. Chat message — user variant
3. Chat message — AI variant (with streaming state)
4. Code block with copy button
5. Input textarea (idle, focused, with content)
6. Input footer row (model chip + plugin slot area + send button)
7. Model picker dropdown
8. Plugin status bar (used by handsfree etc.)
9. Settings two-column shell
10. Plugin card — collapsed
11. Plugin card — expanded
12. Enable/disable toggle
13. Contribution badge (provider / slot / command variants)
14. Toast notification ("reloaded: zen-warp")
15. Theme: Dark, Light, High Contrast

---

## What to Avoid

- No rounded pill buttons or bubbly consumer-app styling
- No large hero areas, gradients, or decorative illustrations
- No full-bleed coloured backgrounds in the chat area
- No auto-playing animations
- Don't make it look like Claude.ai or ChatGPT — this is a *developer tool*
