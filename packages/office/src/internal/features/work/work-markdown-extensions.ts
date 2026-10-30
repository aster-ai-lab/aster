import { Extension, type Extensions } from '@tiptap/core';
import { TaskItem, TaskList } from '@tiptap/extension-list';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import Image from '@tiptap/extension-image';
import Placeholder from '@tiptap/extension-placeholder';
import { TableKit } from '@tiptap/extension-table';
import { Markdown } from '@tiptap/markdown';
import StarterKit from '@tiptap/starter-kit';
import {
  MarkdownAlignedHeading,
  MarkdownAlignedParagraph,
  MarkdownTextAlign,
  WorkMarkdownAlignSyntax,
} from './work-markdown-align';

export interface SetWorkMarkdownOptions {
  emitUpdate?: boolean;
}

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    workMarkdown: {
      setWorkMarkdown: (
        markdown: string,
        options?: SetWorkMarkdownOptions,
      ) => ReturnType;
    };
  }
}

export const WorkMarkdown = Extension.create({
  name: 'workMarkdown',

  addCommands() {
    return {
      setWorkMarkdown:
        (markdown, options = {}) =>
        ({ chain }) =>
          chain()
            .setMeta('addToHistory', false)
            .setContent(markdown, {
              contentType: 'markdown',
              emitUpdate: options.emitUpdate ?? true,
            })
            .run(),
    };
  },
});

/** Match the toolbar's advertised Meta/Control aliases on both editing surfaces. */
const WorkMarkdownShortcuts = Extension.create({
  name: 'workMarkdownShortcuts',

  addKeyboardShortcuts() {
    const commands: Record<string, () => boolean> = {
      b: () => this.editor.commands.toggleBold(),
      i: () => this.editor.commands.toggleItalic(),
      e: () => this.editor.commands.toggleCode(),
      'Shift-s': () => this.editor.commands.toggleStrike(),
      'Shift-x': () => this.editor.commands.toggleStrike(),
      'Shift-8': () => this.editor.commands.toggleBulletList(),
      'Shift-7': () => this.editor.commands.toggleOrderedList(),
      'Shift-9': () => this.editor.commands.toggleTaskList(),
      'Shift-b': () => this.editor.commands.toggleBlockquote(),
      'Alt-c': () => this.editor.commands.toggleCodeBlock(),
      z: () => this.editor.commands.undo(),
      'Shift-z': () => this.editor.commands.redo(),
      y: () => this.editor.commands.redo(),
      'Alt-0': () => this.editor.commands.setParagraph(),
    };
    for (const level of [1, 2, 3, 4, 5, 6] as const) {
      commands[`Alt-${level}`] = () =>
        this.editor.commands.toggleHeading({ level });
    }
    return Object.fromEntries(
      Object.entries(commands).flatMap(([key, command]) =>
        ['Meta', 'Ctrl'].map((modifier) => [`${modifier}-${key}`, command]),
      ),
    );
  },
});

export interface WorkMarkdownExtensionOptions {
  collaborative?: boolean;
  /** Empty-document prompt. Notes pass their own copy. */
  placeholder?: string;
  /** Preview task toggle. Visual editing updates the node through TipTap instead. */
  onReadOnlyChecked?: (node: ProseMirrorNode, checked: boolean) => boolean;
}

export function createWorkMarkdownExtensions(
  options: WorkMarkdownExtensionOptions = {},
): Extensions {
  return [
    StarterKit.configure({
      // Replaced below so text-align survives the Markdown round trip.
      heading: false,
      paragraph: false,
      link: {
        autolink: true,
        defaultProtocol: 'https',
        openOnClick: false,
      },
      underline: false,
      ...(options.collaborative ? { undoRedo: false } : {}),
    }),
    MarkdownAlignedParagraph,
    MarkdownAlignedHeading,
    MarkdownTextAlign,
    WorkMarkdownAlignSyntax,
    TaskList,
    TaskItem.configure({
      nested: true,
      HTMLAttributes: {
        'data-type': 'taskItem',
      },
      a11y: {
        checkboxLabel: markdownTaskCheckboxLabel,
      },
      onReadOnlyChecked: options.onReadOnlyChecked,
    }),
    Image.configure({
      allowBase64: false,
      inline: true,
    }),
    TableKit.configure({
      table: {
        allowTableNodeSelection: true,
        resizable: false,
      },
    }),
    Placeholder.configure({
      placeholder: options.placeholder ?? '开始写 Markdown…',
      // Notes show the prompt in the read-only preview pane as well as the source.
      showOnlyWhenEditable: options.placeholder ? false : true,
    }),
    Markdown.configure({
      indentation: { style: 'space', size: 2 },
      markedOptions: {
        gfm: true,
        breaks: false,
        pedantic: false,
      },
    }),
    WorkMarkdown,
    WorkMarkdownShortcuts,
  ];
}

export function markdownTaskCheckboxLabel(node: {
  attrs: { checked?: boolean };
  textContent: string;
}): string {
  const label = node.textContent.trim() || '任务';
  return `${node.attrs.checked ? '已完成' : '未完成'}：${label}`;
}
