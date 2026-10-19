import { Extension, type JSONContent, type MarkdownToken } from '@tiptap/core';
import Heading from '@tiptap/extension-heading';
import Paragraph from '@tiptap/extension-paragraph';
import TextAlign from '@tiptap/extension-text-align';
import {
  markdownExplicitAlign,
  matchMarkdownAlignBlock,
} from './editors/markdown-align-syntax';

const EMPTY_PARAGRAPH_MARKDOWN = '&nbsp;';
const NBSP_CHAR = '\u00A0';

function alignAttrs(token: MarkdownToken): { textAlign: string } | undefined {
  const align = markdownExplicitAlign(token.textAlign);
  return align ? { textAlign: align } : undefined;
}

function isEmptyParagraphToken(token: MarkdownToken): boolean {
  const tokens = token.tokens || [];
  if (tokens.length !== 1 || tokens[0]?.type !== 'text') return false;
  const raw = tokens[0].raw;
  const text = tokens[0].text;
  return (
    raw === EMPTY_PARAGRAPH_MARKDOWN ||
    text === EMPTY_PARAGRAPH_MARKDOWN ||
    raw === NBSP_CHAR ||
    text === NBSP_CHAR
  );
}

export const MarkdownAlignedParagraph = Paragraph.extend({
  parseMarkdown(token, helpers) {
    const tokens = token.tokens || [];
    if (tokens.length === 1 && tokens[0]?.type === 'image') {
      return helpers.parseChildren([tokens[0]]);
    }
    const content = helpers.parseInline(tokens);
    const attrs = alignAttrs(token);
    if (
      isEmptyParagraphToken(token) &&
      content.length === 1 &&
      content[0]?.type === 'text' &&
      (content[0].text === EMPTY_PARAGRAPH_MARKDOWN ||
        content[0].text === NBSP_CHAR)
    ) {
      return helpers.createNode('paragraph', attrs, []);
    }
    return helpers.createNode('paragraph', attrs, content);
  },

  renderMarkdown(node, helpers, ctx) {
    if (!node) return '';
    const content = Array.isArray(node.content) ? node.content : [];
    if (content.length === 0) {
      const previousContent = Array.isArray(ctx.previousNode?.content)
        ? ctx.previousNode.content
        : [];
      return ctx.previousNode?.type === 'paragraph' && previousContent.length === 0
        ? EMPTY_PARAGRAPH_MARKDOWN
        : '';
    }
    const body = helpers.renderChildren(content);
    const align = markdownExplicitAlign(node.attrs?.textAlign);
    if (!align) return body;
    return `<p style="text-align: ${align}">${body}</p>`;
  },
});

export const MarkdownAlignedHeading = Heading.extend({
  parseMarkdown(token, helpers) {
    const align = markdownExplicitAlign(token.textAlign);
    const level = token.depth || 1;
    return helpers.createNode(
      'heading',
      align ? { level, textAlign: align } : { level },
      helpers.parseInline(token.tokens || []),
    );
  },

  renderMarkdown(node, helpers) {
    const parsedLevel = node.attrs?.level ? Number.parseInt(String(node.attrs.level), 10) : 1;
    const level = parsedLevel >= 1 && parsedLevel <= 6 ? parsedLevel : 1;
    if (!node.content) return '';
    const body = helpers.renderChildren(node.content as JSONContent[]);
    const align = markdownExplicitAlign(node.attrs?.textAlign);
    if (!align) return `${'#'.repeat(level)} ${body}`;
    return `<h${level} style="text-align: ${align}">${body}</h${level}>`;
  },
});

export const WorkMarkdownAlignSyntax = Extension.create({
  name: 'workMarkdownAlignSyntax',

  markdownTokenizer: {
    name: 'workMarkdownAlign',
    level: 'block',
    start: (src) => (matchMarkdownAlignBlock(src) ? 0 : -1),
    tokenize: (src, _tokens, lexer) => {
      const match = matchMarkdownAlignBlock(src);
      if (!match) return undefined;
      const textAlign = markdownExplicitAlign(match.align);
      if (match.tag === 'p') {
        return {
          type: 'paragraph',
          raw: match.raw,
          textAlign,
          tokens: lexer.inlineTokens(match.inner),
        };
      }
      return {
        type: 'heading',
        raw: match.raw,
        depth: match.level ?? 1,
        textAlign,
        tokens: lexer.inlineTokens(match.inner),
      };
    },
  },
});

/**
 * Paragraph alignment for Markdown. Left stays the unset default, so ordinary
 * notes remain plain Markdown. The document editor's Mod-Shift-l bullet chord
 * is not reused here.
 */
export const MarkdownTextAlign = TextAlign.extend({
  addKeyboardShortcuts() {
    return {};
  },
}).configure({
  types: ['heading', 'paragraph'],
  alignments: ['left', 'center', 'right', 'justify'],
  defaultAlignment: null,
});
