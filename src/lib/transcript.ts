import { hasToolResults, parseContent, textBlocks } from './types';
import type { Message, ToolResultBlock, ToolUseBlock } from './types';

/**
 * Build a self-describing transcript of the exchange ending at (or containing)
 * message `toId`: the triggering user prompt, the assistant replies, every tool
 * call made, and each tool's output — in an explicit `<pragma-transcript>`
 * format that other agents can consume as plain context.
 */
export function buildAgentTranscript(messages: Message[], toId: string): string {
  const idx = messages.findIndex((m) => m.id === toId);
  if (idx < 0) return '';

  // Span: start at the most recent plain user prompt at-or-before the target,
  // end after the target — plus its tool-result turn if the target is an
  // assistant message that issued tool calls.
  let start = idx;
  while (start > 0) {
    const c = parseContent(messages[start - 1].content);
    if (messages[start - 1].role === 'user' && !hasToolResults(c)) break;
    start--;
  }
  let end = idx + 1;
  const targetContent = parseContent(messages[idx].content);
  if (
    messages[idx].role === 'assistant' &&
    Array.isArray(targetContent) &&
    targetContent.some((b) => b.type === 'tool_use') &&
    messages[idx + 1]?.role === 'user'
  ) {
    end = idx + 2;
  }
  const span = messages.slice(start, end);

  // Collect all tool results in the span so tool calls pair with their outputs.
  const results = new Map<string, { content: string; is_error: boolean }>();
  for (const m of span) {
    const c = parseContent(m.content);
    if (!Array.isArray(c)) continue;
    for (const b of c) {
      if (b.type === 'tool_result') {
        const r = b as ToolResultBlock;
        results.set(r.tool_use_id, { content: r.content, is_error: r.is_error ?? false });
      }
    }
  }

  const out: string[] = [];
  out.push('<pragma-transcript>');
  out.push(
    '<note>Copied from the Pragma desktop AI assistant — user prompt, replies, tool calls, and their outputs (paths/content are exact; long payloads are elided).</note>',
  );

  for (const m of span) {
    const c = parseContent(m.content);

    if (m.role === 'user' && !Array.isArray(c) && String(c).trim()) {
      out.push('<user-prompt>');
      out.push(String(c));
      out.push('</user-prompt>');
      continue;
    }
    if (m.role !== 'assistant') continue;

    const blocks = Array.isArray(c) ? c : null;
    if (blocks) {
      const texts = textBlocks(blocks)
        .map((b) => b.text)
        .filter(Boolean);
      if (texts.length) {
        out.push('<assistant-reply>');
        out.push(texts.join('\n'));
        out.push('</assistant-reply>');
      }
      for (const b of blocks) {
        if (b.type !== 'tool_use') continue;
        const t = b as ToolUseBlock;
        const res = results.get(t.id);
        out.push(`<tool-call name="${t.name}" use-id="${t.id}">`);
        out.push('<input>');
        out.push(compactInput(t.input ?? {}));
        out.push('</input>');
        out.push(`<output ok="${res ? !res.is_error : '?'}">`);
        out.push(clip(res ? res.content : '(no output recorded)', 400));
        out.push('</output>');
        out.push('</tool-call>');
      }
    } else if (String(c).trim()) {
      out.push('<assistant-reply>');
      out.push(String(c));
      out.push('</assistant-reply>');
    }
  }

  out.push('</pragma-transcript>');
  return out.join('\n');
}

// Keep the transcript digestible: elide any string field over 200 chars in a
// tool input (e.g. write_file's `content`) instead of
// pasting the payload verbatim, and cap the single-line JSON size.
function compactInput(input: Record<string, unknown>): string {
  const slim: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(input)) {
    if (typeof v === 'string' && v.length > 200) {
      slim[k] = `[${v.length} chars omitted]`;
    } else {
      slim[k] = v;
    }
  }
  let s = JSON.stringify(slim, null, 1);
  if (s.length > 600) s = s.slice(0, 600) + '\n…[truncated]';
  return s;
}

function clip(s: string, max: number): string {
  if (s.length <= max) return s;
  return s.slice(0, max) + `\n…[${s.length - max} more chars]`;
}
