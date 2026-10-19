import {
  type MarkdownTextAlign,
  markdownHeadingLevel,
  renderMarkdownAlignLine,
  unwrapMarkdownAlignLine,
} from './markdown-align-syntax';

export interface MarkdownSourceSelection {
  start: number;
  end: number;
  direction: 'backward' | 'forward' | 'none';
}

export interface MarkdownSourceEdit {
  markdown: string;
  selection: MarkdownSourceSelection;
}

export interface MarkdownSourceSelectionState {
  markdown: string;
  selection: MarkdownSourceSelection;
  text: string;
}

export interface MarkdownSourceInsert {
  replacement: string;
  selectedRange: { start: number; end: number };
}

export interface MarkdownSourceLink {
  label: string;
  rawLabel: string;
  range: { start: number; end: number };
  source: string;
}

export type MarkdownSourceCommand =
  | 'align-center'
  | 'align-justify'
  | 'align-left'
  | 'align-right'
  | 'blockquote'
  | 'bold'
  | 'bullet-list'
  | 'code'
  | 'code-block'
  | 'heading-1'
  | 'heading-2'
  | 'heading-3'
  | 'heading-4'
  | 'heading-5'
  | 'heading-6'
  | 'horizontal-rule'
  | 'italic'
  | 'ordered-list'
  | 'paragraph'
  | 'strike'
  | 'table'
  | 'task-list';

const INLINE_WRAPPERS = {
  bold: '**',
  code: '`',
  italic: '*',
  strike: '~~',
} as const;

const MARKDOWN_TABLE = [
  '| Column 1 | Column 2 | Column 3 |',
  '| --- | --- | --- |',
  '|  |  |  |',
  '|  |  |  |',
].join('\n');

export function isMarkdownSourceCommandActive(
  markdown: string,
  requestedSelection: MarkdownSourceSelection,
  command: MarkdownSourceCommand,
): boolean {
  const selection = normalizeMarkdownSourceSelection(
    requestedSelection,
    markdown.length,
  );
  if (isInlineMarkdownSourceCommand(command)) {
    return sourceSelectionHasInlineWrapper(
      markdown,
      selection,
      INLINE_WRAPPERS[command],
      command,
    );
  }
  if (command === 'code-block') {
    const selected = markdown.slice(selection.start, selection.end);
    return (
      (selected.startsWith('```\n') && selected.endsWith('\n```')) ||
      (markdown.slice(0, selection.start).endsWith('```\n') &&
        markdown.slice(selection.end).startsWith('\n```'))
    );
  }
  if (command === 'horizontal-rule' || command === 'table') return false;
  if (isMarkdownAlignCommand(command)) {
    return (
      sourceSelectionAlignment(markdown, selection) ===
      markdownAlignFromCommand(command)
    );
  }

  const range = selectedLineRange(markdown, selection);
  const lines = markdown
    .slice(range.start, range.end)
    .split('\n')
    .filter((line) => line.length > 0);
  if (!lines.length) return false;
  if (command.startsWith('heading-')) {
    const level = Number(command.at(-1));
    const pattern = new RegExp(`^#{${level}}\\s+`);
    return lines.every((line) =>
      pattern.test(unwrapMarkdownAlignLine(line)?.markdown ?? line),
    );
  }
  if (command === 'paragraph') {
    return lines.every(
      (line) => !/^#{1,6}\s+/.test(unwrapMarkdownAlignLine(line)?.markdown ?? line),
    );
  }
  if (command === 'blockquote') {
    return lines.every((line) => /^>\s+/.test(line));
  }
  if (command === 'bullet-list') {
    return lines.every((line) => /^[-+*]\s+(?!\[[ xX]\]\s+)/.test(line));
  }
  if (command === 'ordered-list') {
    return lines.every((line) => /^\d+[.)]\s+/.test(line));
  }
  return lines.every((line) => /^[-+*]\s+\[[ xX]\]\s+/.test(line));
}

export function applyMarkdownSourceCommand(
  markdown: string,
  requestedSelection: MarkdownSourceSelection,
  command: MarkdownSourceCommand,
): MarkdownSourceEdit {
  const selection = normalizeMarkdownSourceSelection(
    requestedSelection,
    markdown.length,
  );
  if (isInlineMarkdownSourceCommand(command)) {
    return toggleInlineWrapper(
      markdown,
      selection,
      INLINE_WRAPPERS[command],
      command,
    );
  }
  if (command === 'horizontal-rule') {
    return insertMarkdownBlock(markdown, selection, '---', undefined, true);
  }
  if (command === 'table') {
    return insertMarkdownBlock(markdown, selection, MARKDOWN_TABLE, {
      start: 2,
      end: 10,
    });
  }
  if (command === 'code-block') {
    return toggleCodeBlock(markdown, selection);
  }
  if (isMarkdownAlignCommand(command)) {
    return applyMarkdownSourceAlignment(
      markdown,
      selection,
      markdownAlignFromCommand(command),
    );
  }
  return transformSelectedLines(markdown, selection, command);
}

function isInlineMarkdownSourceCommand(
  command: MarkdownSourceCommand,
): command is keyof typeof INLINE_WRAPPERS {
  return Object.hasOwn(INLINE_WRAPPERS, command);
}

export function replaceMarkdownSourceSelection(
  markdown: string,
  requestedSelection: MarkdownSourceSelection,
  replacement: string,
  selectedRange: { start: number; end: number } = {
    start: 0,
    end: replacement.length,
  },
): MarkdownSourceEdit {
  const selection = normalizeMarkdownSourceSelection(
    requestedSelection,
    markdown.length,
  );
  const nextMarkdown = `${markdown.slice(0, selection.start)}${replacement}${markdown.slice(selection.end)}`;
  return {
    markdown: nextMarkdown,
    selection: {
      start: selection.start + selectedRange.start,
      end: selection.start + selectedRange.end,
      direction: 'none',
    },
  };
}

export function createMarkdownLinkSourceInsert(
  label: string,
  source: string,
): MarkdownSourceInsert {
  return createMarkdownSourceInsert('', label, source, 1);
}

export function createMarkdownImageSourceInsert(
  altText: string,
  source: string,
): MarkdownSourceInsert {
  return createMarkdownSourceInsert('!', altText, source, 2);
}

export function findMarkdownSourceLink(
  markdown: string,
  requestedSelection: MarkdownSourceSelection,
): MarkdownSourceLink | null {
  const selection = normalizeMarkdownSourceSelection(
    requestedSelection,
    markdown.length,
  );
  let cursor = 0;

  while (cursor < markdown.length) {
    const openBracket = markdown.indexOf('[', cursor);
    if (openBracket < 0) return null;
    cursor = openBracket + 1;
    if (
      markdownDelimiterIsEscaped(markdown, openBracket) ||
      markdownSourceBracketStartsImage(markdown, openBracket)
    ) {
      continue;
    }

    const closeBracket = markdownSourceClosingBracket(markdown, openBracket);
    if (closeBracket < 0 || markdown[closeBracket + 1] !== '(') continue;
    const destination = markdownSourceLinkDestination(
      markdown,
      closeBracket + 1,
    );
    if (!destination) continue;
    cursor = destination.range.end;
    const range = { start: openBracket, end: destination.range.end };
    if (!markdownSourceSelectionTargetsRange(selection, range)) continue;
    const rawLabel = markdown.slice(openBracket + 1, closeBracket);
    return {
      label: unescapeMarkdownLinkLabel(rawLabel),
      rawLabel,
      range,
      source: unescapeMarkdownLinkDestination(destination.rawSource),
    };
  }
  return null;
}

export function createMarkdownSourceLinkRemoval(
  link: MarkdownSourceLink,
): MarkdownSourceInsert {
  return {
    replacement: link.rawLabel,
    selectedRange: { start: 0, end: link.rawLabel.length },
  };
}

function createMarkdownSourceInsert(
  prefix: '' | '!',
  text: string,
  source: string,
  selectionStart: 1 | 2,
): MarkdownSourceInsert {
  const escapedText = text
    .replaceAll('\\', '\\\\')
    .replaceAll('[', '\\[')
    .replaceAll(']', '\\]');
  const destination = /\s/u.test(source)
    ? `<${source}>`
    : source.replace(/([\\()])/gu, '\\$1');
  return {
    replacement: `${prefix}[${escapedText}](${destination})`,
    selectedRange: {
      start: selectionStart,
      end: selectionStart + escapedText.length,
    },
  };
}

function markdownSourceBracketStartsImage(
  markdown: string,
  openBracket: number,
): boolean {
  const marker = openBracket - 1;
  return (
    marker >= 0 &&
    markdown[marker] === '!' &&
    !markdownDelimiterIsEscaped(markdown, marker)
  );
}

function markdownSourceClosingBracket(
  markdown: string,
  openBracket: number,
): number {
  let depth = 1;
  for (let cursor = openBracket + 1; cursor < markdown.length; cursor += 1) {
    if (markdownDelimiterIsEscaped(markdown, cursor)) continue;
    if (markdown[cursor] === '[') depth += 1;
    else if (markdown[cursor] === ']') {
      depth -= 1;
      if (depth === 0) return cursor;
    }
  }
  return -1;
}

function markdownSourceLinkDestination(
  markdown: string,
  openParenthesis: number,
): { range: { start: number; end: number }; rawSource: string } | null {
  let cursor = openParenthesis + 1;
  while (markdown[cursor] === ' ' || markdown[cursor] === '\t') cursor += 1;
  if (cursor >= markdown.length || markdown[cursor] === '\n') return null;

  let rawSource = '';
  if (markdown[cursor] === '<') {
    const sourceStart = cursor + 1;
    cursor = sourceStart;
    while (
      cursor < markdown.length &&
      (markdown[cursor] !== '>' || markdownDelimiterIsEscaped(markdown, cursor))
    ) {
      if (markdown[cursor] === '\n') return null;
      cursor += 1;
    }
    if (markdown[cursor] !== '>') return null;
    rawSource = markdown.slice(sourceStart, cursor);
    cursor += 1;
  } else {
    const sourceStart = cursor;
    let depth = 0;
    while (cursor < markdown.length) {
      const character = markdown[cursor];
      if (character === '\n') return null;
      if (markdownDelimiterIsEscaped(markdown, cursor)) {
        cursor += 1;
        continue;
      }
      if (character === '(') {
        depth += 1;
      } else if (character === ')') {
        if (depth === 0) break;
        depth -= 1;
      } else if ((character === ' ' || character === '\t') && depth === 0) {
        break;
      }
      cursor += 1;
    }
    rawSource = markdown.slice(sourceStart, cursor);
  }
  if (!rawSource) return null;

  while (markdown[cursor] === ' ' || markdown[cursor] === '\t') cursor += 1;
  if (markdown[cursor] !== ')') {
    cursor = markdownSourceLinkTitleEnd(markdown, cursor);
    if (cursor < 0) return null;
    while (markdown[cursor] === ' ' || markdown[cursor] === '\t') cursor += 1;
  }
  if (markdown[cursor] !== ')') return null;
  return {
    range: { start: openParenthesis, end: cursor + 1 },
    rawSource,
  };
}

function markdownSourceLinkTitleEnd(markdown: string, start: number): number {
  const opener = markdown[start];
  const closer = opener === '(' ? ')' : opener;
  if (!closer || !['"', "'", '('].includes(opener ?? '')) return -1;
  for (let cursor = start + 1; cursor < markdown.length; cursor += 1) {
    if (markdown[cursor] === '\n') return -1;
    if (
      markdown[cursor] === closer &&
      !markdownDelimiterIsEscaped(markdown, cursor)
    ) {
      return cursor + 1;
    }
  }
  return -1;
}

function markdownSourceSelectionTargetsRange(
  selection: MarkdownSourceSelection,
  range: { start: number; end: number },
): boolean {
  if (selection.start === selection.end) {
    return selection.start >= range.start && selection.start < range.end;
  }
  return selection.start >= range.start && selection.end <= range.end;
}

function unescapeMarkdownLinkLabel(label: string): string {
  return label.replace(/\\([\\[\]])/gu, '$1');
}

function unescapeMarkdownLinkDestination(source: string): string {
  return source.replace(/\\([\\()<>])/gu, '$1');
}

function toggleInlineWrapper(
  markdown: string,
  selection: MarkdownSourceSelection,
  wrapper: string,
  command: MarkdownSourceCommand,
): MarkdownSourceEdit {
  const selected = markdown.slice(selection.start, selection.end);
  const selectedHasWrapper =
    selected.length >= wrapper.length * 2 &&
    selected.startsWith(wrapper) &&
    selected.endsWith(wrapper);
  if (selectedHasWrapper) {
    const replacement = selected.slice(wrapper.length, -wrapper.length);
    return replaceMarkdownSourceSelection(markdown, selection, replacement);
  }

  const enclosingWrapper = inlineWrapperAroundSelection(
    markdown,
    selection,
    wrapper,
    command,
  );
  if (enclosingWrapper) {
    return {
      markdown: `${markdown.slice(0, enclosingWrapper.open)}${markdown.slice(
        enclosingWrapper.open + wrapper.length,
        enclosingWrapper.close,
      )}${markdown.slice(enclosingWrapper.close + wrapper.length)}`,
      selection: {
        start: selection.start - wrapper.length,
        end: selection.end - wrapper.length,
        direction: selection.direction,
      },
    };
  }

  const before = markdown.slice(0, selection.start);
  const after = markdown.slice(selection.end);
  const wrapperOutsideSelection =
    before.endsWith(wrapper) &&
    after.startsWith(wrapper) &&
    !(command === 'italic' && before.endsWith('**') && after.startsWith('**'));
  if (wrapperOutsideSelection) {
    return {
      markdown: `${before.slice(0, -wrapper.length)}${selected}${after.slice(wrapper.length)}`,
      selection: {
        start: selection.start - wrapper.length,
        end: selection.end - wrapper.length,
        direction: selection.direction,
      },
    };
  }

  const replacement = `${wrapper}${selected}${wrapper}`;
  return replaceMarkdownSourceSelection(markdown, selection, replacement, {
    start: wrapper.length,
    end: wrapper.length + selected.length,
  });
}

function sourceSelectionHasInlineWrapper(
  markdown: string,
  selection: MarkdownSourceSelection,
  wrapper: string,
  command: MarkdownSourceCommand,
): boolean {
  const selected = markdown.slice(selection.start, selection.end);
  const selectedHasWrapper =
    selected.length >= wrapper.length * 2 &&
    selected.startsWith(wrapper) &&
    selected.endsWith(wrapper) &&
    !(
      command === 'italic' &&
      selected.startsWith('**') &&
      selected.endsWith('**')
    );
  if (selectedHasWrapper) return true;
  if (inlineWrapperAroundSelection(markdown, selection, wrapper, command)) {
    return true;
  }
  const before = markdown.slice(0, selection.start);
  const after = markdown.slice(selection.end);
  return (
    before.endsWith(wrapper) &&
    after.startsWith(wrapper) &&
    !(command === 'italic' && before.endsWith('**') && after.startsWith('**'))
  );
}

function inlineWrapperAroundSelection(
  markdown: string,
  selection: MarkdownSourceSelection,
  wrapper: string,
  command: MarkdownSourceCommand,
): { open: number; close: number } | null {
  const lineStart =
    markdown.lastIndexOf('\n', Math.max(0, selection.start - 1)) + 1;
  const followingNewline = markdown.indexOf('\n', selection.end);
  const lineEnd = followingNewline < 0 ? markdown.length : followingNewline;
  const delimiters: number[] = [];

  for (let index = lineStart; index <= lineEnd - wrapper.length; ) {
    if (
      markdown.startsWith(wrapper, index) &&
      !markdownDelimiterIsEscaped(markdown, index) &&
      !(
        command === 'italic' &&
        (markdown[index - 1] === '*' ||
          markdown[index + wrapper.length] === '*')
      )
    ) {
      delimiters.push(index);
      index += wrapper.length;
    } else {
      index += 1;
    }
  }

  for (let index = 0; index + 1 < delimiters.length; index += 2) {
    const open = delimiters[index];
    const close = delimiters[index + 1];
    if (
      open !== undefined &&
      close !== undefined &&
      selection.start >= open + wrapper.length &&
      selection.end <= close
    ) {
      return { open, close };
    }
  }
  return null;
}

function markdownDelimiterIsEscaped(markdown: string, index: number): boolean {
  let backslashes = 0;
  for (
    let cursor = index - 1;
    cursor >= 0 && markdown[cursor] === '\\';
    cursor -= 1
  ) {
    backslashes += 1;
  }
  return backslashes % 2 === 1;
}

function isMarkdownAlignCommand(
  command: MarkdownSourceCommand,
): command is 'align-center' | 'align-justify' | 'align-left' | 'align-right' {
  return (
    command === 'align-center' ||
    command === 'align-justify' ||
    command === 'align-left' ||
    command === 'align-right'
  );
}

function markdownAlignFromCommand(
  command: 'align-center' | 'align-justify' | 'align-left' | 'align-right',
): MarkdownTextAlign {
  if (command === 'align-center') return 'center';
  if (command === 'align-right') return 'right';
  if (command === 'align-justify') return 'justify';
  return 'left';
}

function applyMarkdownSourceAlignment(
  markdown: string,
  selection: MarkdownSourceSelection,
  align: MarkdownTextAlign,
): MarkdownSourceEdit {
  const range = selectedLineRange(markdown, selection);
  let fence = fenceStateBefore(markdown, range.start);
  const lines = markdown.slice(range.start, range.end).split('\n');
  const transformed = lines.map((line) => {
    const stepped = stepFence(line, fence);
    fence = stepped.state;
    if (stepped.boundary || fence.open || !isAlignableMarkdownLine(line)) {
      return line;
    }
    const unwrapped = unwrapMarkdownAlignLine(line);
    const source = unwrapped?.markdown ?? line;
    const level = markdownHeadingLevel(source);
    if (level) {
      return renderMarkdownAlignLine(source.replace(/^#{1,6}\s+/, ''), align, {
        kind: 'heading',
        level,
      });
    }
    return renderMarkdownAlignLine(source, align, { kind: 'paragraph' });
  });
  const replacement = transformed.join('\n');
  return {
    markdown: `${markdown.slice(0, range.start)}${replacement}${markdown.slice(range.end)}`,
    selection: {
      start: range.start,
      end: range.start + replacement.length,
      direction: selection.direction,
    },
  };
}

function sourceSelectionAlignment(
  markdown: string,
  selection: MarkdownSourceSelection,
): MarkdownTextAlign | 'mixed' {
  const range = selectedLineRange(markdown, selection);
  let fence = fenceStateBefore(markdown, range.start);
  const found: MarkdownTextAlign[] = [];
  for (const line of markdown.slice(range.start, range.end).split('\n')) {
    const stepped = stepFence(line, fence);
    fence = stepped.state;
    if (stepped.boundary || fence.open || line.length === 0) continue;
    if (!isAlignableMarkdownLine(line)) {
      found.push('left');
      continue;
    }
    found.push(unwrapMarkdownAlignLine(line)?.align ?? 'left');
  }
  if (!found.length) return 'left';
  const first = found[0];
  return first && found.every((item) => item === first) ? first : 'mixed';
}

function isAlignableMarkdownLine(line: string): boolean {
  if (!line.trim()) return false;
  if (unwrapMarkdownAlignLine(line) || markdownHeadingLevel(line)) return true;
  if (/^ {0,3}>/.test(line)) return false;
  if (/^ {0,3}[-+*]\s+/.test(line)) return false;
  if (/^ {0,3}\d+[.)]\s+/.test(line)) return false;
  if (/^ {0,3}\|/.test(line)) return false;
  if (/^ {0,3}([-*_])\1{2,}\s*$/.test(line)) return false;
  if (/^ {0,3}(`{3,}|~{3,})/.test(line)) return false;
  return true;
}

function fenceStateBefore(
  markdown: string,
  index: number,
): { open: boolean; marker: '`' | '~' | null } {
  let state: { open: boolean; marker: '`' | '~' | null } = {
    open: false,
    marker: null,
  };
  const slice = markdown.slice(0, index);
  const lines = slice.split('\n');
  if (slice.endsWith('\n')) lines.pop();
  for (const line of lines) state = stepFence(line, state).state;
  return state;
}

function stepFence(
  line: string,
  state: { open: boolean; marker: '`' | '~' | null },
): {
  boundary: boolean;
  state: { open: boolean; marker: '`' | '~' | null };
} {
  const match = /^( {0,3})(`{3,}|~{3,})/.exec(line);
  if (!match?.[2]) return { boundary: false, state };
  const marker = match[2].startsWith('`') ? '`' : '~';
  if (!state.open) return { boundary: true, state: { open: true, marker } };
  if (state.marker === marker) {
    return { boundary: true, state: { open: false, marker: null } };
  }
  return { boundary: false, state };
}

function transformSelectedLines(
  markdown: string,
  selection: MarkdownSourceSelection,
  command: Exclude<
    MarkdownSourceCommand,
    | 'align-center'
    | 'align-justify'
    | 'align-left'
    | 'align-right'
    | 'bold'
    | 'code'
    | 'code-block'
    | 'horizontal-rule'
    | 'italic'
    | 'strike'
    | 'table'
  >,
): MarkdownSourceEdit {
  const range = selectedLineRange(markdown, selection);
  const lines = markdown.slice(range.start, range.end).split('\n');
  const nonEmptyLines = lines.filter((line) => line.length > 0);
  const everyNonEmptyLine = (pattern: RegExp) =>
    nonEmptyLines.length > 0 &&
    nonEmptyLines.every((line) => pattern.test(line));
  let transformed: string[];

  if (command.startsWith('heading-')) {
    const level = Number(command.at(-1));
    const prefix = `${'#'.repeat(level)} `;
    transformed = lines.map((line) => {
      if (!line) return prefix;
      const unwrapped = unwrapMarkdownAlignLine(line);
      const text = stripBlockPrefix(unwrapped?.markdown ?? line);
      if (unwrapped && unwrapped.align !== 'left') {
        return `<h${level} style="text-align: ${unwrapped.align}">${text}</h${level}>`;
      }
      return `${prefix}${text}`;
    });
  } else if (command === 'paragraph') {
    transformed = lines.map((line) => {
      const unwrapped = unwrapMarkdownAlignLine(line);
      const text = (unwrapped?.markdown ?? line).replace(/^#{1,6}\s+/, '');
      if (unwrapped && unwrapped.align !== 'left') {
        return `<p style="text-align: ${unwrapped.align}">${text}</p>`;
      }
      return text;
    });
  } else if (command === 'blockquote') {
    const remove = everyNonEmptyLine(/^>\s+/);
    transformed = lines.map((line) => {
      const text = unwrapMarkdownAlignLine(line)?.markdown ?? line;
      return remove ? text.replace(/^>\s+/, '') : text ? `> ${text}` : '> ';
    });
  } else {
    const pattern =
      command === 'bullet-list'
        ? /^[-+*]\s+(?!\[[ xX]\]\s+)/
        : command === 'ordered-list'
          ? /^\d+[.)]\s+/
          : /^[-+*]\s+\[[ xX]\]\s+/;
    const remove = everyNonEmptyLine(pattern);
    transformed = lines.map((line, index) => {
      const text = stripListPrefix(unwrapMarkdownAlignLine(line)?.markdown ?? line);
      if (remove) return text;
      if (command === 'ordered-list') return `${index + 1}. ${text}`;
      if (command === 'task-list') return `- [ ] ${text}`;
      return `- ${text}`;
    });
  }

  const replacement = transformed.join('\n');
  return {
    markdown: `${markdown.slice(0, range.start)}${replacement}${markdown.slice(range.end)}`,
    selection: {
      start: range.start,
      end: range.start + replacement.length,
      direction: selection.direction,
    },
  };
}

function toggleCodeBlock(
  markdown: string,
  selection: MarkdownSourceSelection,
): MarkdownSourceEdit {
  const selected = markdown.slice(selection.start, selection.end);
  if (selected.startsWith('```\n') && selected.endsWith('\n```')) {
    const replacement = selected.slice(4, -4);
    return replaceMarkdownSourceSelection(markdown, selection, replacement);
  }
  const before = markdown.slice(0, selection.start);
  const after = markdown.slice(selection.end);
  if (before.endsWith('```\n') && after.startsWith('\n```')) {
    return {
      markdown: `${before.slice(0, -4)}${selected}${after.slice(4)}`,
      selection: {
        start: selection.start - 4,
        end: selection.end - 4,
        direction: selection.direction,
      },
    };
  }
  return replaceMarkdownSourceSelection(
    markdown,
    selection,
    `\`\`\`\n${selected}\n\`\`\``,
    { start: 4, end: 4 + selected.length },
  );
}

function insertMarkdownBlock(
  markdown: string,
  selection: MarkdownSourceSelection,
  block: string,
  selectedRange?: { start: number; end: number },
  appendBlank = false,
): MarkdownSourceEdit {
  const before = markdown.slice(0, selection.start);
  const after = markdown.slice(selection.end);
  const leading = before
    ? before.endsWith('\n\n')
      ? ''
      : before.endsWith('\n')
        ? '\n'
        : '\n\n'
    : '';
  const trailing = after
    ? after.startsWith('\n\n')
      ? ''
      : after.startsWith('\n')
        ? '\n'
        : '\n\n'
    : appendBlank
      ? '\n\n'
      : '';
  const replacement = `${leading}${block}${trailing}`;
  const blockStart = selection.start + leading.length;
  return {
    markdown: `${before}${replacement}${after}`,
    selection: selectedRange
      ? {
          start: blockStart + selectedRange.start,
          end: blockStart + selectedRange.end,
          direction: 'none',
        }
      : {
          start: selection.start + replacement.length,
          end: selection.start + replacement.length,
          direction: 'none',
        },
  };
}

function selectedLineRange(
  markdown: string,
  selection: MarkdownSourceSelection,
): { start: number; end: number } {
  const start =
    selection.start === 0
      ? 0
      : markdown.lastIndexOf('\n', selection.start - 1) + 1;
  const inclusiveEnd =
    selection.end > selection.start && markdown[selection.end - 1] === '\n'
      ? selection.end - 1
      : selection.end;
  const newline = markdown.indexOf('\n', inclusiveEnd);
  return { start, end: newline < 0 ? markdown.length : newline };
}

function stripBlockPrefix(line: string): string {
  return line.replace(
    /^(?:#{1,6}\s+|[-+*]\s+(?:\[[ xX]\]\s+)?|\d+[.)]\s+|>\s+)/,
    '',
  );
}

function stripListPrefix(line: string): string {
  return line.replace(/^(?:[-+*]\s+(?:\[[ xX]\]\s+)?|\d+[.)]\s+)/, '');
}

function normalizeMarkdownSourceSelection(
  selection: MarkdownSourceSelection,
  markdownLength: number,
): MarkdownSourceSelection {
  const requestedStart = Math.min(selection.start, selection.end);
  const requestedEnd = Math.max(selection.start, selection.end);
  return {
    start: Math.max(0, Math.min(markdownLength, requestedStart)),
    end: Math.max(0, Math.min(markdownLength, requestedEnd)),
    direction: selection.direction,
  };
}
