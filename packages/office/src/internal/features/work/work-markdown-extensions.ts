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

/** Chords TipTap does not bind: paragraph style, and Google Docs strikethrough. */
const WorkMarkdownShortcuts = Extension.create({
  name: 'workMarkdownShortcuts',

  addKeyboardShortcuts() {
    return {
      'Mod-Alt-0': () => this.editor.commands.setParagraph(),
      'Mod-Shift-x': () => this.editor.commands.toggleStrike(),
    };
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
