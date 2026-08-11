import { describe, it, expect } from 'vitest'
import { renderMarkdown, segmentsOf } from './markdown'

describe('renderMarkdown', () => {
  it('renders basic markdown', () => {
    const html = renderMarkdown('# Title\n\n**bold** and *italic* and `code`')
    expect(html).toContain('<h1>Title</h1>')
    expect(html).toContain('<strong>bold</strong>')
    expect(html).toContain('<em>italic</em>')
    expect(html).toContain('<code>code</code>')
  })

  it('renders lists and links', () => {
    const html = renderMarkdown('- one\n- two\n\n[link](https://example.com)')
    expect(html).toContain('<ul>')
    expect(html).toContain('<li>one</li>')
    expect(html).toContain('<a href="https://example.com"'.replace('=', '='))
    expect(html).toContain('target="_blank"')
  })

  it('escapes raw HTML from model output', () => {
    const html = renderMarkdown('hello <script>alert(1)</script>')
    expect(html).not.toContain('<script>')
    expect(html).toContain('&lt;script&gt;')
  })

  it('drops unsafe link protocols but keeps the text', () => {
    const html = renderMarkdown('[click](javascript:alert(1))')
    expect(html).not.toContain('javascript:')
    expect(html).toContain('click')
  })

  it('drops data:/javascript: images', () => {
    const html = renderMarkdown('![x](data:image/png;base64,AAAA)')
    expect(html).not.toContain('<img')
  })
})

describe('segmentsOf', () => {
  it('splits prose and fenced code', () => {
    const segs = segmentsOf('intro\n\n```js\nconst x = 1\n```\n\noutro')
    expect(segs.map((s) => s.kind)).toEqual(['prose', 'code', 'prose'])
    expect(segs[1].text).toBe('const x = 1\n')
    expect(segs[2].text).toContain('outro')
  })

  it('keeps an unclosed fence as prose', () => {
    const segs = segmentsOf('start\n```python\nprint(1)')
    expect(segs.length).toBe(1)
    expect(segs[0].kind).toBe('prose')
  })

  it('falls back to a single prose segment for plain text', () => {
    expect(segmentsOf('just text').map((s) => s.kind)).toEqual(['prose'])
    expect(segmentsOf('')).toEqual([])
  })
})