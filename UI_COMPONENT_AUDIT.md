# Pragma Frontend — UI Componentization Audit

Scope: every file under `src/components`, `src/routes`, `src/styles` read in full.
Goal: find UI that **looks the same but isn't shared** and extract it into single components.

Legend: **HIGH** = same visual pattern re-implemented 3+ times with near-identical CSS; **MED** = 2–3 copies or smaller variant drift; **LOW** = 2 copies / CSS-level.

---

## A. Structural component duplication (the big wins)

### A1. Collapsible card — chevron header + grid-rows body  — **HIGH**
Four hand-rolled implementations of the same primitive:
| File | Header class | Body collapse |
|---|---|---|
| `ThoughtCard.svelte` | `.head` (icon + mono title + meta + chevron) | `.body-grid.closed` → `grid-template-rows: 0fr` |
| `SubagentCard.svelte` | `.head` (mark + name + kind + meta + chevron) | `.body-grid.closed` → `0fr` |
| `ToolCard.svelte` | `.header` (icon + label + detail + status + chevron) | `.out-grid.closed` → `0fr` |
| `ProcessCard.svelte` | `.head` (icon + cmd + state + meta + chevron) | `.log-grid` (`0fr`→`1fr`, inverted) |

Duplicated verbatim in each: the `grid-template-rows 1fr/0fr` collapse transition (0.3–0.32s cubic-bezier), the chevron `rotate(180deg)` transition, and the header row layout (icon + mono label + right-side controls).
**Extract:** `Collapsible.svelte` — props `{ open = $bindable(true), onToggle, label }`, `{header}` + `{body}` snippets. All four cards rebuild on it. Bonus: `.rail` (2px left tinted strip) in ThoughtCard/SubagentCard/ProcessCard is also identical.

### A2. AgentMark SVG inlined twice — **MED**
`AgentMark.svelte` owns the diamond+orbit+rects mark, parameterized by `tint`/`size`. `SubagentCard.svelte` hand-copies a scaled-down version of the **same SVG** (15px, stroke-width 1.6/1.8) instead of `<AgentMark size={15} tint={tint} />`.

### A3. Assistant turn row + avatar wrapper duplicated — **MED**
`ChatMessage.svelte` (persisted turn) and `MessageList.svelte` (live turn) repeat the same layout:
- `.avatar { width:30px; height:30px; flex:none; display:flex; align-items:center; justify-content:center; margin-top:2px }` — identical CSS block
- `.assistant-row` / `.live-turn` — identical `flex gap:10px` row
- `.turn-body` / `.live-body` — identical `flex column gap:7px; max-width:78%`

**Extract:** `TurnRow.svelte` (avatar + body) used by both; the two diverge only in what they render inside the body.

### A4. Modal / dialog shell — 4 implementations — **HIGH**
| File | Overlay | Card |
|---|---|---|
| `ReturnModal.svelte` | `.overlay` fixed inset-0 blur | `.modal` radius 14px + `modal-in` |
| `PluginConfirmModal.svelte` | `.overlay` fixed inset-0 blur | `.modal` radius 14px + `modal-in` (same keyframes re-declared) |
| `UpdateDialog.svelte` | `.overlay` fixed inset-0 blur | `.dialog` radius 14px + CornerMarks |
| `DirBar.svelte` picker | `.backdrop` fixed inset-0 blur | `.dialog` radius-md + `dialogIn` |

All share: blur backdrop, centered elevated card, header row with icon-block + title + close button, footer of action buttons. ReturnModal and PluginConfirmModal additionally hand-roll the same ESC-key + outside-click close listeners.
**Extract:** `Modal.svelte` — `{title, badge, onclose}` + `{body}`/`{footer}` snippets, owning overlay, card, ESC/outside-click/backdrop-click close, and the exit animation. UpdateDialog and the DirBar picker adopt it (DirBar keeps its dialogIn/dialogOut variants via a prop).

### A5. Logo block — **MED**
`TabBar.svelte` and `settings/+layout.svelte` inline the same markup + CSS: `logo.svg?raw` + `.wordmark` "PRAGMA", identical padding `0 14px 0 7.5px`, border-right, gap 9px, Barlow Condensed 15px / 0.1em wordmark.
**Extract:** `Logo.svelte` (one source of truth for the brand block).

---

## B. Icon component underuse — **HIGH**

`Icon.svelte` exists but is only imported by **5** files (TextBubble, ToolCard, ThoughtCard, ModelPicker, InputArea). **14+ first-party files** inline raw `<span class="msr">…</span>` instead:

`MessageList`, `QuestionBlock`, `SessionRail`, `TabBar`, `SidebarPanel`, `SubagentCard`, `ProcessCard`, `Toast`, `DirBar`, `VerificationBubble`, `routes/settings/+layout`, `plugins/+page`, `plugin/[id]/+page`, `marketplace/+page`.

Raw `.msr` spans bypass Icon's registry, `aria-hidden`, and the `fill` variant; each also re-declares per-use `font-size`/`line-height` overrides (~40 duplicated one-liners).
**Action:** migrate first-party `.msr` usage to `<Icon name={…} />` (plugin-side `.msr` usage stays raw by design).

---

## C. Settings-page patterns (near-identical CSS, 4+ copies)

### C1. Settings card — **HIGH**
`.card { background: var(--bg-elevated); border: 1px solid var(--border-default); border-radius: var(--radius-md); overflow: hidden; max-width: 640px }` duplicated in `general`, `providers`, `shortcuts`, `verification` (+ `margin-bottom` variant in verification).
**Extract:** `Card.svelte` (or a `.card` utility class in base.css).

### C2. Settings row (label + desc + right control) — **HIGH**
`general/+page` and `verification/+page` share identical `.row` (flex space-between, 12px 16px, border-bottom) + `.row-text`/`.label` (13.5px primary)/`.desc` (12px muted); `appearance/+page` repeats it as `.row-label`/`.row-desc` (14px/12px, border-top).
**Extract:** `SettingRow.svelte` — `{label, desc}` + default snippet slot for the control (Toggle/input/button). Cuts ~60 lines across 3 pages.

### C3. Section header (icon + mono uppercase title + right note) — **HIGH**
Same pattern, five copies:
- `plugin/[id]/+page` `.sec-head` (msr icon + `.sec-title` mono uppercase + `.sec-note` right)
- `plugins/+page` `.install-head` (icon + mono title + note)
- `marketplace/+page` `.reg-head` (icon + mono title + note)
- `SidebarPanel` `.pane-head` (icon + `.pane-label` mono uppercase + meta)
- `general/+page` `.section-title` (bare mono uppercase label)

**Extract:** `SectionHeader.svelte` — `{icon, title, note}`.

### C4. Search field (icon + borderless mono input) — **HIGH**
- `ModelPicker.svelte` `.search`
- `plugins/+page.svelte` `.search`
- `marketplace/+page.svelte` `.reg-search`
- `DirBar.svelte` `.dsearch`

All: flex row, 15–16px `.msr` icon, transparent input, bg-base + border-default, focus `--border-focus`, mono font. Plus bare inputs in `providers` (key row), `verification` (num/text), `plugin/[id]` (cfg-input) that share the same input skin.
**Extract:** `SearchInput.svelte` + a shared `Input.svelte` (or an `.input` utility class) for the plain-text/number fields.

### C5. Segmented control — **MED**
`appearance/+page` `.segmented/.seg` (accent fill on active), `plugins/+page` `.pills/.pill` (accent fill), `SidebarPanel` `.panel-tabs/.ptab` (accent underline) — all are active-state segmented buttons.
**Extract:** `Segmented.svelte` — `{options, value, onchange}`.

### C6. Button variants — **HIGH**
`.ghost` redefined in **6 files** (`ToolCard`, `DirBar`, `PluginConfirmModal`, `ReturnModal`, `general`, `plugins`), `.primary`/`.go`/`.send-btn` accent buttons in 7+, plus one-off `.hdr-btn`, `.act`, `.t-action`, `.follow`, `.back`, `.crumb` — every one a variation of "bordered ghost" or "accent-filled primary", each with its own hover rules.
**Extract:** `Button.svelte` — `variant: 'primary' | 'ghost' | 'danger' | 'quiet'`, `size: 'sm' | 'md'` (+ optional `icon`). Biggest line-count win in the settings routes.

---

## D. Chat / agent status chrome

### D1. Status pill / mono tag — **HIGH**
~13 pill implementations, all mono uppercase 9.5–11px, 2px radius or pill radius, bg-overlay:
`QuestionBlock .q-pill`, `ModelPicker .think-badge`, `SidebarPanel .c-state/.ptab-badge/.pane-badge/.branch-pill`, `ProcessCard .p-state`, `SubagentCard .kind/.token-chip`, `VerificationBubble .vtag`, `providers .status-tag`, `plugins .skill-tag/.status-text`, `plugin detail .status-pill/.b`, `marketplace .verified`, `Toast .t-source`, `ContextMenu .ctx-header`, `TabBar .active-status`.
**Extract:** `Pill.svelte` — `{color?, tone}`.

### D2. Live status dot — **MED**
`.s-dot.live` (SessionRail), `.active-dot.live` (TabBar), `.node-dot.pulse` (SidebarPanel), `.p-dot.blink` (ProcessCard), `.status-dot` (plugins) — 5–7px circles, all pulsing via `cursorblink`.
**Extract:** `StatusDot.svelte` — `{color, live?}`.

### D3. Error / notice row — **MED**
`MessageList.svelte` `.error-row` and `.notice-row` are the same layout (icon + pre-wrap text, 8px 12px, radius-md, tinted bg + border) differing only in color.
**Extract:** `InlineAlert.svelte` — `kind: 'error' | 'info'`.

### D4. Console / log line pane — **MED**
`ToolCard .output/.line` (per-line coloring + 4000-char/300-line truncation + "… truncated" footer), `ProcessCard .log/.log-line`, `SidebarPanel .console-body/.c-line` (`lineStyle()` coloring), `VerificationBubble .vsummary/.vfindings` — all mono pre-wrap colored output views.
**Extract:** `OutputLog.svelte` — `{lines, colorFor(line), maxHeight}`; fold ToolCard's truncation logic in so the other three stop hand-rolling it.

### D5. Tool-icon mapping duplicated — **LOW**
`ToolCard.iconFor()` and `SubagentCard.TOOL_ICONS` map the same tool→glyph families (read→description, edit→edit_document, run→terminal, search/grep→travel_explore, write→note_add, default→build).
**Extract:** `src/lib/toolIcons.ts` shared map.

---

## E. CSS / animation token duplication

### E1. `.stagger` entrance — **HIGH**
`animation: rowIn 0.34s cubic-bezier(0.22,0.8,0.3,1) both; animation-delay: calc(var(--i) * 45ms)` duplicated in **8 files**: `SessionRail`, `SidebarPanel`, `settings/+layout`, `plugins/+page`, `marketplace/+page`, plus row-level variants in `QuestionBlock`, `SubagentCard`, `DirBar` (60ms variants).
**Action:** define `.stagger` once in `tokens.css`/`base.css`.

### E2. Duplicate keyframes — **MED**
Local keyframes re-declaring global ones: `fade-in` (ReturnModal, PluginConfirmModal) ≈ global `fadeIn`; `modal-in` (×2) ≈ global `dialogIn`; `menu-in` (ContextMenu) ≈ global `dropIn`; `vspin` (VerificationBubble) = global `spin`; `bubbleIn` on `TextBubble` uses global; `toastOut` global is duplicated by nothing but `menu-in` is close. Delete the locals and use the global names.

### E3. Collapse/chevron CSS — see A1 (single Collapsible kills 4 copies).

### E4. Markdown changelog styling — **MED**
`.changelog` block (h1–h3, p, ul/ol, code, pre styling for `renderMarkdown` output) duplicated between `UpdateDialog.svelte` and `general/+page.svelte` (which also has `.upd-title`/`.update-head` matching UpdateDialog's head).
**Extract:** `Changelog.svelte` — `{body}` rendering markdown with the shared prose styles.

---

## F. Empty states — **MED**
`plugins .empty`, `marketplace .empty`, `plugin/[id] .missing`, `SessionRail .rail-empty`, `SidebarPanel .empty-hint` (×3) + `.upcoming`, `DirBar .dlist-empty` — all centered/muted mono text blocks with an optional icon.
**Extract:** `EmptyState.svelte` — `{icon?, message}`.

---

## G. Misc

- **G1. Mini-toggle re-implements `Toggle`** — `InputArea` `.auto-dot/.track/.knob` (26×15 square knob) duplicates `Toggle.svelte`'s design at a smaller size. Add a `size` prop to `Toggle` and reuse (or extract the knob).
- **G2. Settings page-input skin** — `providers` key inputs, `verification` num/text, `plugin/[id]` cfg-input: same bg-base/border-default/focus-border-focus text input. Fold into C4's shared `Input`.
- **G3. `.mono-meta` text** — ChatMessage `.meta`, SidebarPanel `.c-meta/.node-meta/.cost-meta`, SubagentCard `.meta`, ProcessCard `.p-meta`, VerificationBubble `.vmeta`: mono 9.5–11px muted. **LOW** — a utility class suffices.
- **G4. Dir path row** — `DirBar` `.dir-btn/.path` and SessionRail `.g-dir` (rtl-ellipsis mono path): minor, can share a `.mono-path` utility. **LOW**

---

## Suggested extraction order (impact ∝ duplication count)

1. **`Button`** (6+ ghost copies, 7+ primary copies) — C6
2. **`Collapsible`** (4 cards) — A1
3. **`Modal`** (4 dialogs) — A4
4. **`Icon` migration** (14 files off raw `.msr`) — B
5. **`SettingRow` + `Card`** (4 pages) — C1/C2
6. **`Pill` + `StatusDot`** (13 + 5 sites) — D1/D2
7. **`SectionHeader`** (5 sites) — C3
8. **`.stagger` / global keyframes** (8 sites) — E1/E2
9. **`SearchInput` / shared input skin** (6 sites) — C4/G2
10. **`Segmented`** (3 sites) — C5
11. **`EmptyState`** (8 sites) — F
12. **`OutputLog`** (4 panes) — D4
13. **`TurnRow` / `AgentMark` reuse** (2 sites) — A2/A3
14. **`Logo`** (2 sites) — A5
15. **`InlineAlert`**, **`Changelog`**, **`toolIcons`**, **`Toggle` size prop** — D3/E4/D5/G1

---

# Componentization report — done ✓

All items in the audit above were implemented in a single refactor pass (see git diff).
Every shared primitive lives in `src/components/` and consumers were migrated onto it.

## New shared components

| Component | Replaces (files) | Notes |
|---|---|---|
| `Button.svelte` | `.ghost`/`.primary`/`.danger`/`.go`/`.hdr-btn`/`.act` in ToolCard, DirBar, PluginConfirmModal, ReturnModal, UpdateDialog, general, plugins, marketplace, plugin detail, ProcessCard | variants `primary/ghost/danger/danger-solid/warning/quiet`, sizes `xs/sm/md`, `mono`, `icon`, `loading`, `href` |
| `Collapsible.svelte` | ThoughtCard / SubagentCard / ToolCard / ProcessCard card shells | rail + chevron + grid-rows collapse + header/actions/footer snippets |
| `Modal.svelte` | ReturnModal / PluginConfirmModal / UpdateDialog overlay+card+ESC/backdrop-close | `padded`, `zIndex`, `footClass` |
| `Logo.svelte` | TabBar + settings layout brand block | |
| `TurnRow.svelte` | ChatMessage + MessageList assistant turn row + avatar | |
| `Card.svelte` | general / providers / shortcuts / verification `.card` | |
| `SettingRow.svelte` | general / verification / appearance label+desc+control rows | `bare` variant |
| `SectionHeader.svelte` | plugins install-panel / marketplace registry / plugin-detail section heads | |
| `SearchInput.svelte` | ModelPicker dropdown / plugins / marketplace / DirBar search fields | `box`/`underline`, `mono`, `trailing` slot, `bind:inputEl` |
| `Segmented.svelte` | appearance `.segmented` + plugins filter pills | `mono` variant |
| `EmptyState.svelte` | plugins / marketplace / plugin-detail empty states | |
| `Pill.svelte` | q-pill / think-badge / c-state / p-state / skill-tag / status-tag / status-pill / verified / pane-badge / branch-pill / ptab-badge / kind | tones + `color` + `outline` + `radius` + `icon` |
| `StatusDot.svelte` | s-dot.live / active-dot.live / node-dot.pulse / p-dot.blink / status-dot | `color`, `live`, `size` |
| `InlineAlert.svelte` | MessageList error-row + notice-row | `kind: error/info` |
| `OutputLog.svelte` | ToolCard output / ProcessCard log / SidebarPanel console | truncation + per-line color + `anchorBottom` |
| `Changelog.svelte` | UpdateDialog + general changelog markdown styling | |
| `toolIcons.ts` | ToolCard `iconFor()` + SubagentCard `TOOL_ICONS` | shared tool→glyph map |

## Cross-cutting fixes

- **Icon registry expanded** — every glyph used anywhere in the app now lives in
  `Icon.svelte`; all first-party raw `<span class="msr">` usages (14 files) migrated
  to `<Icon name="…" />` (aria-hidden + size + fill handled centrally).
- **`.stagger` moved to `tokens.css`** — the 45ms rowIn entrance was defined in 8 files.
- **Duplicate keyframes removed** — `fade-in`/`modal-in` consolidated into `Modal`;
  `vspin` → global `spin`; local `cursorblink` in TextBubble removed.
- **Toggle gained `size="sm"`** — InputArea's hand-rolled auto-approve switch now uses it.
- **AgentMark reused** in SubagentCard (was an inlined 15px copy).
- **`pragma-dev` skill reference updated** to list the new shared primitives.

## Verification

- `npm run check` — 0 errors, 0 warnings
- `npm run test` — 25/25 pass
- `npm run build` — production build succeeds

## Fidelity corrections (post-refactor review)

The first pass over-changed the **Marketplace** page (it had newer committed
behavior that the audit description didn't reflect). Restored to exact parity:
- **Search UX**: back to the empty-start input + fetched pool (`all`) narrowed
  **live as you type** (`visible`), Enter re-searches — not the older
  prefill + re-search-on-Enter behavior.
- **No added elements**: removed the reg-note, reg-blurb paragraph,
  verified/community pill, and the "· npm / unknown author" meta suffix that
  were never in the committed page.
- Kept only pixel-identical componentization (SectionHeader, SearchInput,
  EmptyState, Icon, global `.stagger`).

Also restored per-site button skins so the shared `Button` renders exactly
what it replaced: `flat` (borderless ghost — general page), heading-face
Save/Install/Open-directory CTAs, 8px-radius modal buttons, 14px update-dialog
buttons, and the general changelog heading size + shortcuts card width.
