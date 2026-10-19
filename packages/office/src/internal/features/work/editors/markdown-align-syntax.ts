export const MARKDOWN_TEXT_ALIGNS = ['left', 'center', 'right', 'justify'] as const;

export type MarkdownTextAlign = (typeof MARKDOWN_TEXT_ALIGNS)[number];

export type MarkdownExplicitAlign = Exclude<MarkdownTextAlign, 'left'>;

const ALIGN_TAG = /^( {0,3})<(p|h([1-6]))\b([^>\n]*)>/i;

export function markdownExplicitAlign(
  value: unknown,
): MarkdownExplicitAlign | null {
  if (value === 'center' || value === 'right' || value === 'justify') return value;
  return null;
}

export interface MarkdownAlignBlock {
  align: MarkdownTextAlign;
  inner: string;
  level: number | null;
  raw: string;
  tag: 'p' | 'h';
}

/**
 * A paragraph or heading written as one HTML block so CommonMark can carry
 * text-align. The inner text stays Markdown; the preview tokenizer parses it.
 */
export function matchMarkdownAlignBlock(src: string): MarkdownAlignBlock | null {
  const open = ALIGN_TAG.exec(src);
  if (!open) return null;
  const tagName = (open[2] ?? '').toLowerCase();
  const align = readStyleAlign(open[4] ?? '');
  if (!align || (tagName !== 'p' && !open[3])) return null;
  const rest = src.slice(open[0].length);
  const closeAt = rest.toLowerCase().indexOf(`</${tagName}>`);
  if (closeAt < 0 || closeAt > 20_000) return null;
  const afterClose = rest.slice(closeAt + tagName.length + 3);
  const tail = /^[ \t]*(\n|$)/.exec(afterClose);
  if (!tail) return null;
  const rawLength = open[0].length + closeAt + tagName.length + 3 + tail[0].length;
  return {
    align,
    inner: rest.slice(0, closeAt),
    level: open[3] ? Number(open[3]) : null,
    raw: src.slice(0, rawLength),
    tag: tagName === 'p' ? 'p' : 'h',
  };
}

export function unwrapMarkdownAlignLine(
  line: string,
): { align: MarkdownTextAlign; markdown: string } | null {
  const match = matchMarkdownAlignBlock(line);
  if (!match || match.raw !== line) return null;
  if (match.tag === 'h' && match.level) {
    return {
      align: match.align,
      markdown: `${'#'.repeat(match.level)} ${match.inner}`,
    };
  }
  return { align: match.align, markdown: match.inner };
}

export function markdownHeadingLevel(line: string): number | null {
  const match = /^(#{1,6})\s+/.exec(line);
  return match?.[1] ? match[1].length : null;
}

export function renderMarkdownAlignLine(
  text: string,
  align: MarkdownTextAlign,
  block: { kind: 'paragraph' } | { kind: 'heading'; level: number },
): string {
  if (align === 'left') {
    return block.kind === 'heading' ? `${'#'.repeat(block.level)} ${text}` : text;
  }
  if (block.kind === 'heading') {
    return `<h${block.level} style="text-align: ${align}">${text}</h${block.level}>`;
  }
  return `<p style="text-align: ${align}">${text}</p>`;
}

function readStyleAlign(attrs: string): MarkdownTextAlign | null {
  const quoted =
    /\bstyle\s*=\s*"([^"]*)"/i.exec(attrs) ??
    /\bstyle\s*=\s*'([^']*)'/i.exec(attrs);
  const style = quoted?.[1];
  if (!style) return null;
  const found = /(?:^|;)\s*text-align\s*:\s*(left|center|right|justify)\b/i.exec(
    style,
  );
  const value = found?.[1]?.toLowerCase();
  if (
    value === 'left' ||
    value === 'center' ||
    value === 'right' ||
    value === 'justify'
  ) {
    return value;
  }
  return null;
}
