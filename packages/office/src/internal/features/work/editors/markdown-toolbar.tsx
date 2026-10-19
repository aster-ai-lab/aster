import type { Editor } from '@tiptap/core';
import { officeMessage, resolveOfficeMessages } from '../../../i18n/office-locale';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import {
  AlignCenter,
  AlignJustify,
  AlignLeft,
  AlignRight,
  Bold,
  Code2,
  Image as ImageIcon,
  Italic,
  Link2,
  List,
  ListChecks,
  ListOrdered,
  Minus,
  Quote,
  Redo2,
  Strikethrough,
  Table2,
  Undo2,
  Unlink,
} from 'lucide-react';
import {
  type ButtonHTMLAttributes,
  type ReactNode,
  useEffect,
  useRef,
  useState,
} from 'react';
import {
  MarkdownInsertDialog,
  type MarkdownInsertDialogRequest,
  type MarkdownInsertDialogResult,
} from './markdown-insert-dialog';
import type {
  MarkdownSourceCommand,
  MarkdownSourceSelectionState,
} from './markdown-source-commands';
import {
  createMarkdownImageSourceInsert,
  createMarkdownLinkSourceInsert,
  createMarkdownSourceLinkRemoval,
  findMarkdownSourceLink,
  isMarkdownSourceCommandActive,
} from './markdown-source-commands';
import type { MarkdownViewMode } from './markdown-workspace';
import { synchronizeDocumentEditorSelectionFromDom } from './document-dom-selection';
import { OfficeSelect } from './office-controls';
import { useOfficeMessages } from './office-messages-context';
import { isOfficeShortcutBlocked } from './office-shortcuts';
import { moveOfficeToolbarFocus } from './office-toolbar-keyboard';
import {
  type WorkOfficeFileAction,
  WorkOfficeFileMenu,
  WorkOfficeRibbonButton,
} from './work-office-chrome';

type MarkdownInsertDialogState =
  | {
      request: MarkdownInsertDialogRequest;
      surface: 'source';
      target: MarkdownSourceSelectionState;
    }
  | {
      request: MarkdownInsertDialogRequest;
      surface: 'visual';
      target: {
        document: ProseMirrorNode;
        from: number;
        text: string;
        to: number;
      };
    };

interface MarkdownToolbarShortcut {
  ariaKeyShortcuts: string;
  label: string;
}

const markdownToolbarShortcuts = {
  blockquote: {
    ariaKeyShortcuts: 'Control+Shift+B Meta+Shift+B',
    label: 'Cmd/Ctrl+Shift+B',
  },
  bold: {
    ariaKeyShortcuts: 'Control+B Meta+B',
    label: 'Cmd/Ctrl+B',
  },
  bulletList: {
    ariaKeyShortcuts: 'Control+Shift+8 Meta+Shift+8',
    label: 'Cmd/Ctrl+Shift+8',
  },
  code: {
    ariaKeyShortcuts: 'Control+E Meta+E',
    label: 'Cmd/Ctrl+E',
  },
  codeBlock: {
    ariaKeyShortcuts: 'Control+Alt+C Meta+Alt+C',
    label: 'Cmd/Ctrl+Alt+C',
  },
  italic: {
    ariaKeyShortcuts: 'Control+I Meta+I',
    label: 'Cmd/Ctrl+I',
  },
  link: {
    ariaKeyShortcuts: 'Control+K Meta+K',
    label: 'Cmd/Ctrl+K',
  },
  orderedList: {
    ariaKeyShortcuts: 'Control+Shift+7 Meta+Shift+7',
    label: 'Cmd/Ctrl+Shift+7',
  },
  redo: {
    ariaKeyShortcuts: 'Control+Shift+Z Meta+Shift+Z Control+Y Meta+Y',
    label: '',
  },
  strike: {
    ariaKeyShortcuts: 'Control+Shift+S Meta+Shift+S Control+Shift+X Meta+Shift+X',
    label: 'Cmd/Ctrl+Shift+S',
  },
  taskList: {
    ariaKeyShortcuts: 'Control+Shift+9 Meta+Shift+9',
    label: 'Cmd/Ctrl+Shift+9',
  },
  undo: {
    ariaKeyShortcuts: 'Control+Z Meta+Z',
    label: 'Cmd/Ctrl+Z',
  },
} satisfies Record<string, MarkdownToolbarShortcut>;

export function MarkdownToolbar({
  editor,
  fileActions,
  collaborative,
  sourceEditing,
  canSourceRedo,
  canSourceUndo,
  viewMode,
  modes = ['visual', 'source', 'split'],
  getSourceFocusTarget,
  getSourceSelection,
  onSourceCommand,
  onSourceRedo,
  onSourceReplace,
  onSourceUndo,
  onViewModeChange,
}: {
  editor: Editor;
  fileActions?: readonly WorkOfficeFileAction[];
  collaborative: boolean;
  sourceEditing: boolean;
  canSourceRedo: boolean;
  canSourceUndo: boolean;
  viewMode: MarkdownViewMode;
  /** Note pages are split then source. Documents keep visual, source, then split. */
  modes?: readonly MarkdownViewMode[];
  getSourceFocusTarget: () => HTMLElement | null;
  getSourceSelection: () => MarkdownSourceSelectionState | null;
  onSourceCommand: (command: MarkdownSourceCommand) => boolean;
  onSourceRedo: () => boolean;
  onSourceReplace: (
    replacement: string,
    selectedRange?: { start: number; end: number },
    target?: MarkdownSourceSelectionState,
  ) => boolean;
  onSourceUndo: () => boolean;
  onViewModeChange: (mode: MarkdownViewMode) => void;
}) {
  const messages = useOfficeMessages();
  const resolvedShortcuts = {
    ...markdownToolbarShortcuts,
    redo: {
      ...markdownToolbarShortcuts.redo,
      label: officeMessage(messages, 'markdown.action.redoTitle'),
    },
  };
  const [insertDialog, setInsertDialog] =
    useState<MarkdownInsertDialogState | null>(null);
  const openLinkDialogRef = useRef<() => void>(() => undefined);
  const sourceSelection = sourceEditing ? getSourceSelection() : null;
  const sourceLink = sourceSelection
    ? findMarkdownSourceLink(
        sourceSelection.markdown,
        sourceSelection.selection,
      )
    : null;
  const commandIsActive = (command: MarkdownSourceCommand) => {
    if (!sourceSelection) return command === 'align-left';
    return isMarkdownSourceCommandActive(
      sourceSelection.markdown,
      sourceSelection.selection,
      command,
    );
  };
  const paragraphStyle = markdownParagraphStyleValue(
    sourceEditing,
    commandIsActive,
    editor,
  );
  const linkActive = sourceEditing
    ? Boolean(sourceLink)
    : editor.isActive('link');
  const usesSharedHistory = sourceEditing || collaborative;
  const canUndo = usesSharedHistory
    ? canSourceUndo
    : canRunVisualEditorCommand(editor, () =>
        editor.can().chain().focus().undo().run(),
      );
  const canRedo = usesSharedHistory
    ? canSourceRedo
    : canRunVisualEditorCommand(editor, () =>
        editor.can().chain().focus().redo().run(),
      );
  const runCommand = (
    sourceCommand: MarkdownSourceCommand,
    visualCommand: () => void,
  ) => {
    if (sourceEditing) {
      onSourceCommand(sourceCommand);
      return;
    }
    // A pointer selection can land in the browser before ProseMirror records it.
    synchronizeDocumentEditorSelectionFromDom(editor);
    visualCommand();
  };
  const openLinkDialog = () => {
    if (sourceEditing) {
      const current = getSourceSelection();
      if (!current) return;
      const link = findMarkdownSourceLink(current.markdown, current.selection);
      setInsertDialog({
        surface: 'source',
        target: link
          ? {
              markdown: current.markdown,
              selection: {
                start: link.range.start,
                end: link.range.end,
                direction: 'none',
              },
              text: current.markdown.slice(link.range.start, link.range.end),
            }
          : current,
        request: {
          kind: 'link',
          action: link ? 'edit' : 'insert',
          label: link?.label ?? current.text,
          source: link?.source ?? 'https://',
        },
      });
      return;
    }
    synchronizeDocumentEditorSelectionFromDom(editor);
    const editingLink = editor.isActive('link');
    if (editingLink) editor.commands.extendMarkRange('link');
    const { from, to } = editor.state.selection;
    setInsertDialog({
      surface: 'visual',
      target: {
        document: editor.state.doc,
        from,
        text: editor.state.doc.textBetween(from, to, ' '),
        to,
      },
      request: {
        kind: 'link',
        action: editingLink ? 'edit' : 'insert',
        label: editor.state.selection.empty
          ? ''
          : editor.state.doc.textBetween(from, to, ' '),
        source: editingLink
          ? String(editor.getAttributes('link').href ?? '')
          : 'https://',
      },
    });
  };

  useEffect(() => {
    openLinkDialogRef.current = openLinkDialog;
  });

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.repeat ||
        event.altKey ||
        event.shiftKey ||
        !(event.metaKey || event.ctrlKey) ||
        event.key.toLocaleLowerCase() !== 'k' ||
        isOfficeShortcutBlocked(event.target)
      ) {
        return;
      }
      const target = event.target;
      if (
        !(target instanceof Element) ||
        !target.closest('.work-markdown-editor')
      ) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      openLinkDialogRef.current();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, []);

  const removeLink = () => {
    if (sourceEditing) {
      const current = getSourceSelection();
      if (!current) return;
      const link = findMarkdownSourceLink(current.markdown, current.selection);
      if (!link) return;
      const removal = createMarkdownSourceLinkRemoval(link);
      onSourceReplace(removal.replacement, removal.selectedRange, {
        markdown: current.markdown,
        selection: {
          start: link.range.start,
          end: link.range.end,
          direction: 'none',
        },
        text: current.markdown.slice(link.range.start, link.range.end),
      });
      return;
    }
    synchronizeDocumentEditorSelectionFromDom(editor);
    if (!editor.isActive('link')) return;
    // Keep ribbon focus on remove-link. chain().focus() schedules into the
    // editor and breaks L2 loops.
    editor.chain().extendMarkRange('link').unsetLink().run();
  };
  const openImageDialog = () => {
    if (sourceEditing) {
      const current = getSourceSelection();
      if (!current) return;
      setInsertDialog({
        surface: 'source',
        target: current,
        request: {
          kind: 'image',
          altText: '',
          source: 'https://',
        },
      });
      return;
    }
    synchronizeDocumentEditorSelectionFromDom(editor);
    const { from, to } = editor.state.selection;
    setInsertDialog({
      surface: 'visual',
      target: {
        document: editor.state.doc,
        from,
        text: editor.state.doc.textBetween(from, to, ' '),
        to,
      },
      request: {
        kind: 'image',
        altText: '',
        source: 'https://',
      },
    });
  };
  const commitInsert = (result: MarkdownInsertDialogResult) => {
    const dialog = insertDialog;
    if (!dialog) return;
    if (dialog.surface === 'source') {
      const insert =
        result.kind === 'image'
          ? createMarkdownImageSourceInsert(result.altText, result.source)
          : createMarkdownLinkSourceInsert(result.label, result.source);
      onSourceReplace(insert.replacement, insert.selectedRange, dialog.target);
      setInsertDialog(null);
      return;
    }
    if (editor.isDestroyed || !editor.state.doc.eq(dialog.target.document)) {
      setInsertDialog(null);
      return;
    }
    const chain = editor
      .chain()
      .focus()
      .setTextSelection({ from: dialog.target.from, to: dialog.target.to });
    if (result.kind === 'image') {
      chain
        .setImage({
          src: result.source,
          alt: result.altText || undefined,
        })
        .run();
      setInsertDialog(null);
      return;
    }
    if (dialog.target.text && dialog.target.text === result.label) {
      chain.setLink({ href: result.source }).run();
      setInsertDialog(null);
      return;
    }
    chain
      .insertContent({
        type: 'text',
        text: result.label,
        marks: [{ type: 'link', attrs: { href: result.source } }],
      })
      .run();
    setInsertDialog(null);
  };

  return (
    <>
      <div className="work-markdown-ribbon">
        {fileActions?.length ? (
          <WorkOfficeFileMenu actions={fileActions} />
        ) : null}
        <div
          className="work-markdown-commands markdown-toolbar"
          role="toolbar"
          aria-label={officeMessage(messages, 'markdown.toolbar.aria')}
          onKeyDown={moveOfficeToolbarFocus}
        >
              <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.undo')}
                  shortcut={resolvedShortcuts.undo}
                  disabled={!canUndo}
                  onClick={() => {
                    if (usesSharedHistory) onSourceUndo();
                    // Keep ribbon focus — chain().focus() steals into the editor.
                    else editor.commands.undo();
                  }}
                >
                  <Undo2 size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.redo')}
                  shortcut={resolvedShortcuts.redo}
                  disabled={!canRedo}
                  onClick={() => {
                    if (usesSharedHistory) onSourceRedo();
                    // Keep ribbon focus — chain().focus() steals into the editor.
                    else editor.commands.redo();
                  }}
                >
                  <Redo2 size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
              <span className="work-markdown-toolbar-rule" aria-hidden="true" />
                <OfficeSelect
                  ariaLabel={officeMessage(messages, 'markdown.style.aria')}
                  className="markdown-paragraph-style-select"
                  menuClassName="markdown-paragraph-style-menu"
                  value={paragraphStyle}
                  options={[
                    { value: 'paragraph', label: officeMessage(messages, 'markdown.style.paragraph') },
                    { value: 'h1', label: officeMessage(messages, 'markdown.style.h1') },
                    { value: 'h2', label: officeMessage(messages, 'markdown.style.h2') },
                    { value: 'h3', label: officeMessage(messages, 'markdown.style.h3') },
                    { value: 'h4', label: officeMessage(messages, 'markdown.style.h4') },
                    { value: 'h5', label: officeMessage(messages, 'markdown.style.h5') },
                    { value: 'h6', label: officeMessage(messages, 'markdown.style.h6') },
                  ]}
                  onValueChange={(value) => {
                    if (value === 'paragraph') {
                      runCommand('paragraph', () => {
                        // Keep ribbon focus on the style combobox.
                        // chain().focus() schedules into the editor and breaks L2 loops.
                        editor.commands.setParagraph();
                      });
                    } else {
                      const level = Number(value.slice(1)) as
                        | 1
                        | 2
                        | 3
                        | 4
                        | 5
                        | 6;
                      runCommand(`heading-${level}`, () => {
                        // Keep ribbon focus on the style combobox.
                        // chain().focus() schedules into the editor and breaks L2 loops.
                        editor.commands.toggleHeading({ level });
                      });
                    }
                  }}
                />
              <span className="work-markdown-toolbar-rule" aria-hidden="true" />
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.bold')}
                  shortcut={resolvedShortcuts.bold}
                  active={
                    sourceEditing
                      ? commandIsActive('bold')
                      : editor.isActive('bold')
                  }
                  onClick={() =>
                    runCommand('bold', () => {
                      // Keep ribbon focus — chain().focus() steals into the editor.
                      editor.commands.toggleBold();
                    })
                  }
                >
                  <Bold size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.italic')}
                  shortcut={resolvedShortcuts.italic}
                  active={
                    sourceEditing
                      ? commandIsActive('italic')
                      : editor.isActive('italic')
                  }
                  onClick={() =>
                    runCommand('italic', () => {
                      // Keep ribbon focus — chain().focus() steals into the editor.
                      editor.commands.toggleItalic();
                    })
                  }
                >
                  <Italic size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.strike')}
                  shortcut={resolvedShortcuts.strike}
                  active={
                    sourceEditing
                      ? commandIsActive('strike')
                      : editor.isActive('strike')
                  }
                  onClick={() =>
                    runCommand('strike', () => {
                      // Keep ribbon focus — chain().focus() steals into the editor.
                      editor.commands.toggleStrike();
                    })
                  }
                >
                  <Strikethrough size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.inlineCode')}
                  shortcut={resolvedShortcuts.code}
                  active={
                    sourceEditing
                      ? commandIsActive('code')
                      : editor.isActive('code')
                  }
                  onClick={() =>
                    runCommand('code', () => {
                      // Keep ribbon focus — chain().focus() steals into the editor.
                      editor.commands.toggleCode();
                    })
                  }
                >
                  <Code2 size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
              <span className="work-markdown-toolbar-rule" aria-hidden="true" />
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.bulletList')}
                  shortcut={resolvedShortcuts.bulletList}
                  active={
                    sourceEditing
                      ? commandIsActive('bullet-list')
                      : editor.isActive('bulletList')
                  }
                  onClick={() =>
                    runCommand('bullet-list', () => {
                      // Keep ribbon focus — chain().focus() steals into the editor.
                      editor.commands.toggleBulletList();
                    })
                  }
                >
                  <List size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.orderedList')}
                  shortcut={resolvedShortcuts.orderedList}
                  active={
                    sourceEditing
                      ? commandIsActive('ordered-list')
                      : editor.isActive('orderedList')
                  }
                  onClick={() =>
                    runCommand('ordered-list', () => {
                      // Keep ribbon focus — chain().focus() steals into the editor.
                      editor.commands.toggleOrderedList();
                    })
                  }
                >
                  <ListOrdered size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.taskList')}
                  shortcut={resolvedShortcuts.taskList}
                  active={
                    sourceEditing
                      ? commandIsActive('task-list')
                      : editor.isActive('taskList')
                  }
                  onClick={() =>
                    runCommand('task-list', () => {
                      // Keep ribbon focus — chain().focus() steals into the editor.
                      editor.commands.toggleTaskList();
                    })
                  }
                >
                  <ListChecks size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.blockquote')}
                  shortcut={resolvedShortcuts.blockquote}
                  active={
                    sourceEditing
                      ? commandIsActive('blockquote')
                      : editor.isActive('blockquote')
                  }
                  onClick={() =>
                    runCommand('blockquote', () => {
                      // Keep ribbon focus — chain().focus() steals into the editor.
                      editor.commands.toggleBlockquote();
                    })
                  }
                >
                  <Quote size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
              <span className="work-markdown-toolbar-rule" aria-hidden="true" />
              <div
                className="work-markdown-align"
                role="radiogroup"
                aria-label={officeMessage(messages, 'markdown.group.align')}
              >
                {(
                  [
                    ['left', 'markdown.action.alignLeft', AlignLeft],
                    ['center', 'markdown.action.alignCenter', AlignCenter],
                    ['right', 'markdown.action.alignRight', AlignRight],
                    ['justify', 'markdown.action.alignJustify', AlignJustify],
                  ] as const
                ).map(([align, labelKey, Icon]) => (
                  <MarkdownToolbarButton
                    key={align}
                    label={officeMessage(messages, labelKey)}
                    active={
                      sourceEditing
                        ? commandIsActive(`align-${align}`)
                        : visualMarkdownAlign(editor) === align
                    }
                    role="radio"
                    aria-checked={
                      sourceEditing
                        ? commandIsActive(`align-${align}`)
                        : visualMarkdownAlign(editor) === align
                    }
                    aria-pressed={undefined}
                    onClick={() =>
                      runCommand(`align-${align}`, () => {
                        if (align === 'left') editor.commands.unsetTextAlign();
                        else editor.commands.setTextAlign(align);
                      })
                    }
                  >
                    <Icon size={16} strokeWidth={1.7} />
                  </MarkdownToolbarButton>
                ))}
              </div>
              <span className="work-markdown-toolbar-rule" aria-hidden="true" />
                <MarkdownToolbarButton
                  label={linkActive ? officeMessage(messages, 'markdown.action.editLink') : officeMessage(messages, 'markdown.action.addLink')}
                  shortcut={resolvedShortcuts.link}
                  active={linkActive}
                  onClick={openLinkDialog}
                >
                  <Link2 size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.removeLink')}
                  disabled={!linkActive}
                  onClick={removeLink}
                >
                  <Unlink size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.insertImage')}
                  onClick={openImageDialog}
                >
                  <ImageIcon size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.codeBlock')}
                  shortcut={resolvedShortcuts.codeBlock}
                  active={
                    sourceEditing
                      ? commandIsActive('code-block')
                      : editor.isActive('codeBlock')
                  }
                  onClick={() =>
                    runCommand('code-block', () => {
                      // Keep toolbar focus — chain().focus() steals into the editor.
                      editor.commands.toggleCodeBlock();
                    })
                  }
                >
                  <Code2 size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.horizontalRule')}
                  onClick={() =>
                    runCommand('horizontal-rule', () => {
                      // Keep toolbar focus — chain().focus() steals into the editor.
                      editor.commands.setHorizontalRule();
                    })
                  }
                >
                  <Minus size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
                <MarkdownToolbarButton
                  label={officeMessage(messages, 'markdown.action.table')}
                  onClick={() =>
                    runCommand('table', () => {
                      // Keep toolbar focus — chain().focus() steals into the editor.
                      editor.commands.insertTable({
                        rows: 3,
                        cols: 3,
                        withHeaderRow: true,
                      });
                    })
                  }
                >
                  <Table2 size={16} strokeWidth={1.7} />
                </MarkdownToolbarButton>
        </div>
        <MarkdownModeSwitch
          viewMode={viewMode}
          modes={modes}
          onViewModeChange={onViewModeChange}
        />
      </div>
      {insertDialog && (
        <MarkdownInsertDialog
          request={insertDialog.request}
          restoreFocusTarget={() =>
            insertDialog.surface === 'source'
              ? (getSourceFocusTarget() ?? editor.view.dom)
              : editor.view.dom
          }
          onClose={() => setInsertDialog(null)}
          onSubmit={commitInsert}
        />
      )}
    </>
  );
}

function visualMarkdownAlign(
  editor: Editor,
): 'left' | 'center' | 'right' | 'justify' {
  if (editor.isActive({ textAlign: 'center' })) return 'center';
  if (editor.isActive({ textAlign: 'right' })) return 'right';
  if (editor.isActive({ textAlign: 'justify' })) return 'justify';
  return 'left';
}

function markdownParagraphStyleValue(
  sourceEditing: boolean,
  commandIsActive: (command: MarkdownSourceCommand) => boolean,
  editor: Editor,
): 'paragraph' | 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6' {
  for (const level of [1, 2, 3, 4, 5, 6] as const) {
    const active = sourceEditing
      ? commandIsActive(`heading-${level}`)
      : editor.isActive('heading', { level });
    if (active) return `h${level}`;
  }
  return 'paragraph';
}

function canRunVisualEditorCommand(
  editor: Editor,
  command: () => boolean,
): boolean {
  if (editor.isDestroyed || !editor.view) return false;
  return command();
}

const MARKDOWN_MODE_LABELS = {
  visual: ['markdown.mode.visual', 'markdown.mode.visualShort'],
  source: ['markdown.mode.source', 'markdown.mode.sourceShort'],
  split: ['markdown.mode.split', 'markdown.mode.splitShort'],
} as const;

function MarkdownModeSwitch({
  viewMode,
  modes,
  onViewModeChange,
}: {
  viewMode: MarkdownViewMode;
  modes: readonly MarkdownViewMode[];
  onViewModeChange: (mode: MarkdownViewMode) => void;
}) {
  const messages = useOfficeMessages();
  return (
    <section
      className="work-markdown-mode-switch"
      aria-label={officeMessage(messages, 'markdown.group.editMode')}
    >
      {modes.map((mode) => {
        const [labelKey, shortKey] = MARKDOWN_MODE_LABELS[mode];
        return (
          <MarkdownToolbarButton
            key={mode}
            label={officeMessage(messages, labelKey)}
            visibleLabel={officeMessage(messages, shortKey)}
            displayLabel
            active={viewMode === mode}
            onClick={() => onViewModeChange(mode)}
          >
            {null}
          </MarkdownToolbarButton>
        );
      })}
    </section>
  );
}

function MarkdownToolbarButton({
  label,
  visibleLabel,
  shortcut,
  active = false,
  displayLabel = false,
  children,
  onMouseDown,
  ...props
}: {
  label: string;
  visibleLabel?: string;
  shortcut?: MarkdownToolbarShortcut;
  active?: boolean;
  displayLabel?: boolean;
  children: ReactNode;
} & Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  'aria-label' | 'children' | 'title' | 'type'
>) {
  return (
    <WorkOfficeRibbonButton
      {...props}
      label={label}
      visibleLabel={visibleLabel}
      title={shortcut ? officeMessage(resolveOfficeMessages(), 'markdown.action.shortcutTitle', { label, shortcut: shortcut.label }) : label}
      aria-keyshortcuts={shortcut?.ariaKeyShortcuts}
      active={active}
      displayLabel={displayLabel}
      onMouseDown={(event) => {
        // Keep focus on stay-mounted ribbon triggers for L2 loops.
        event.preventDefault();
        onMouseDown?.(event);
      }}
    >
      {children}
    </WorkOfficeRibbonButton>
  );
}
