import { marked } from 'marked'

// ── Safe Markdown for model output ─────────────────────────────────────────
// The model's text is rendered as HTML via {@html}. Two layers of defense:
//   1. Input is HTML-escaped BEFORE parsing, so raw tags in the output can
//      never reach the DOM (markdown syntax — bold, lists, fences, links —
//      is unaffected; only autolinks <https://…> become plain text).
//   2. Tags marked generates itself (<a>, <img>) are sanitized: only
//      http(s)/mailto hrefs and http(s) images survive.

function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

const SAFE_HREF = /^(https?:|mailto:)/i

let _markedConfigured = false
function configure() {
  if (_markedConfigured) return
  _markedConfigured = true
  marked.setOptions({
    gfm: true,
    breaks: true,
  })
}

/** Render model prose (already split off from fenced code blocks) as safe
 *  Markdown HTML for `{@html}`. */
export function renderMarkdown(prose: string): string {
  configure()
  const cached = _mdCache.get(prose)
  if (cached !== undefined) return cached
  const html = buildMarkdownHtml(prose)
  if (_mdCache.size >= MD_CACHE_MAX) _mdCache.clear() // crude bound — full clear beats eviction bookkeeping
  _mdCache.set(prose, html)
  return html
}

// `marked.parse` is synchronous and costs ~2–6ms per block; loading a long
// session re-parses every text block on the main thread. Cache by exact text
// so revisiting a session or re-rendering a message is a Map hit, not a
// blocking re-parse. Bounded to keep memory in check.
const MD_CACHE_MAX = 200
const _mdCache = new Map<string, string>()

function buildMarkdownHtml(prose: string): string {
  const raw = marked.parse(escapeHtml(prose), { async: false }) as string
  return (
    raw
      // Links: drop unsafe hrefs entirely, force external-friendly attrs.
      .replace(/<a\s+href="([^"]*)"[^>]*>/gi, (_m, href: string) => {
        if (!SAFE_HREF.test(href.trim())) return '' // keep the link text
        return `<a href="${href}" target="_blank" rel="noopener noreferrer">`
      })
      // Images: only remote http(s) images may load.
      .replace(/<img\s+src="([^"]*)"[^>]*\/?>/gi, (m, src: string) => {
        if (/^https?:\/\//i.test(src.trim())) return m
        return ''
      })
  )
}