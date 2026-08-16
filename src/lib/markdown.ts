import { marked } from 'marked';

// ── Safe Markdown for model output ─────────────────────────────────────────
// The model's text is rendered as HTML via {@html}. Two layers of defense:
//   1. Input is HTML-escaped BEFORE parsing, so raw tags in the output can
//      never reach the DOM (markdown syntax — bold, lists, fences, links —
//      is unaffected; only autolinks <https://…> become plain text).
//   2. Tags marked generates itself (<a>, <img>) are sanitized: only
//      http(s)/mailto hrefs and http(s) images survive.

function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

const SAFE_HREF = /^(https?:|mailto:)/i;

let _markedConfigured = false;
function configure() {
  if (_markedConfigured) return;
  _markedConfigured = true;
  marked.setOptions({
    gfm: true,
    breaks: true,
  });
}

/** Render model prose (already split off from fenced code blocks) as safe
 *  Markdown HTML for `{@html}`. */
export function renderMarkdown(prose: string): string {
  configure();
  const cached = _mdCache.get(prose);
  if (cached !== undefined) return cached;
  const html = buildMarkdownHtml(prose);
  if (_mdCache.size >= MD_CACHE_MAX) _mdCache.clear(); // crude bound — full clear beats eviction bookkeeping
  _mdCache.set(prose, html);
  return html;
}

// `marked.parse` is synchronous and costs ~2–6ms per block; loading a long
// session re-parses every text block on the main thread. Cache by exact text
// so revisiting a session or re-rendering a message is a Map hit, not a
// blocking re-parse. Bounded to keep memory in check.
const MD_CACHE_MAX = 200;
const _mdCache = new Map<string, string>();

function buildMarkdownHtml(prose: string): string {
  const raw = marked.parse(escapeHtml(prose), { async: false }) as string;
  return (
    raw
      // Links: drop unsafe hrefs entirely, force external-friendly attrs.
      .replace(/<a\s+href="([^"]*)"[^>]*>/gi, (_m, href: string) => {
        if (!SAFE_HREF.test(href.trim())) return ''; // keep the link text
        return `<a href="${href}" target="_blank" rel="noopener noreferrer">`;
      })
      // Images: only remote http(s) images may load.
      .replace(/<img\s+src="([^"]*)"[^>]*\/?>/gi, (m, src: string) => {
        if (/^https?:\/\//i.test(src.trim())) return m;
        return '';
      })
      // marked always ends its output with "\n"; inside the pre-wrap bubble
      // that stays a real text node and renders as a phantom empty line under
      // every response. Strip trailing whitespace from the generated HTML.
      .replace(/\s+$/, '')
  );
}

/** Place the streaming caret on the LAST rendered line — injected just
 *  before the final `</p>` — so it sits after the last word instead of
 *  starting its own empty line (a sibling span after the closing tag). */
export function withCaret(html: string): string {
  const p = html.lastIndexOf('</p>');
  if (p === -1) return html + '<span class="caret">▌</span>';
  return html.slice(0, p) + '<span class="caret">▌</span>' + html.slice(p);
}

/** Expose the app's markdown renderer to plugins. Plugin components load as
 *  blob ES modules with no bundler, so they can't `import` this module — the
 *  host installs the exact renderer on `window.__pragmaMarkdown` for them
 *  (rendering is shared verbatim; only the CSS classes are the plugin's own).
 *  Called once from +layout.svelte on mount. */
export function installMarkdownPluginBridge() {
  if (typeof window === 'undefined') return;
  const w = window as unknown as Record<string, unknown>;
  if (w.__pragmaMarkdown) return;
  w.__pragmaMarkdown = { renderMarkdown, segmentsOf, withCaret };
}

/** Split prose from fenced code blocks so code renders as a styled block and
 *  prose goes through renderMarkdown. Shared by ChatMessage (persisted turns)
 *  and MessageList (streaming live bubbles). */
export type Segment = { kind: 'prose' | 'code'; text: string };
export function segmentsOf(text: string): Segment[] {
  const re = /```(\w*)\n?([\s\S]*?)```/g;
  const out: Segment[] = [];
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) out.push({ kind: 'prose', text: text.slice(last, m.index) });
    out.push({ kind: 'code', text: m[2] });
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ kind: 'prose', text: text.slice(last) });
  if (out.length === 0 && text.trim()) out.push({ kind: 'prose', text });
  return out;
}

/** Trim trailing whitespace/newlines before display — stored text often ends
 *  with a stray newline which pre-wrap would render as an extra empty line
 *  inside the bubble. Shared so streaming and finished bubbles treat text
 *  identically. */
export function tidy(s: string): string {
  return s.replace(/\s+$/, '');
}
