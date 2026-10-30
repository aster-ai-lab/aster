import type { Mark, Node } from '@tiptap/pm/model';
import type { Selection } from '@tiptap/pm/state';
import { useCallback, useState } from 'react';
import type {
  MarkdownSourceEdit,
  MarkdownSourceSelection,
} from './markdown-source-commands';

const MARKDOWN_SOURCE_HISTORY_LIMIT = 100;
const MARKDOWN_TYPING_COALESCE_MS = 1_000;

export interface MarkdownVisualHistorySnapshot {
  doc: Node;
  selection: Selection;
  storedMarks: readonly Mark[] | null;
}

export interface MarkdownHistoryEdit extends MarkdownSourceEdit {
  visual?: MarkdownVisualHistorySnapshot;
}

interface MarkdownSourceTypingGroup {
  at: number;
  inputType?: string;
}

interface MarkdownSourceHistoryState {
  past: MarkdownHistoryEdit[];
  present: MarkdownHistoryEdit;
  future: MarkdownHistoryEdit[];
  typing: MarkdownSourceTypingGroup | null;
}

const documentHistories = new WeakMap<
  object,
  { current: MarkdownSourceHistoryState }
>();

export function useMarkdownSourceHistory(
  initialMarkdown: string,
  historyKey?: object,
) {
  const [historyRef] = useState(() => {
    const retained = historyKey && documentHistories.get(historyKey);
    const ref: { current: MarkdownSourceHistoryState } = {
      current:
        retained && retained.current.present.markdown === initialMarkdown
          ? retained.current
          : {
              past: [],
              present: {
                markdown: initialMarkdown,
                selection: collapsedSelection(0),
              },
              future: [],
              typing: null,
            },
    };
    ref.current.typing = null;
    if (historyKey) documentHistories.set(historyKey, ref);
    return ref;
  });
  const [, setVersion] = useState(0);
  const refresh = useCallback(() => setVersion((value) => value + 1), []);

  const record = useCallback(
    (
      edit: MarkdownHistoryEdit,
      options: { typing?: boolean; inputType?: string } = {},
    ): boolean => {
      const history = historyRef.current;
      const next = normalizeSnapshot(edit);
      if (next.markdown === history.present.markdown) {
        history.present = next;
        return false;
      }

      const now = Date.now();
      const coalesce =
        options.typing === true &&
        canCoalesceTyping(history, next, options.inputType, now);
      if (!coalesce) {
        history.past = [
          ...history.past.slice(-(MARKDOWN_SOURCE_HISTORY_LIMIT - 1)),
          history.present,
        ];
      }
      history.present = next;
      history.future = [];
      history.typing = options.typing
        ? { at: now, inputType: options.inputType }
        : null;
      refresh();
      return true;
    },
    [refresh],
  );

  const reset = useCallback(
    (markdown: string, selection: MarkdownSourceSelection): void => {
      const next = normalizeSnapshot({ markdown, selection });
      const history = historyRef.current;
      const changed =
        history.past.length > 0 ||
        history.future.length > 0 ||
        history.present.markdown !== next.markdown;
      historyRef.current = {
        past: [],
        present: next,
        future: [],
        typing: null,
      };
      if (changed) refresh();
    },
    [refresh],
  );

  const updateSelection = useCallback(
    (selection: MarkdownSourceSelection): void => {
      const history = historyRef.current;
      const normalized = normalizeSelection(
        selection,
        history.present.markdown.length,
      );
      if (sameSelection(normalized, history.present.selection)) return;
      history.present = { ...history.present, selection: normalized };
      history.typing = null;
      refresh();
    },
    [refresh],
  );

  const updateVisual = useCallback((visual: MarkdownVisualHistorySnapshot) => {
    const history = historyRef.current;
    const previous = history.present.visual;
    // Selection updates can arrive before the matching document update.
    if (previous && !previous.doc.eq(visual.doc)) return;
    if (previous && !previous.selection.eq(visual.selection)) {
      history.typing = null;
    }
    history.present = { ...history.present, visual };
  }, []);

  const undo = useCallback((): MarkdownHistoryEdit | null => {
    const history = historyRef.current;
    const previous = history.past.at(-1);
    if (!previous) return null;
    history.past = history.past.slice(0, -1);
    history.future = [
      ...history.future.slice(-(MARKDOWN_SOURCE_HISTORY_LIMIT - 1)),
      history.present,
    ];
    history.present = previous;
    history.typing = null;
    refresh();
    return previous;
  }, [refresh]);

  const redo = useCallback((): MarkdownHistoryEdit | null => {
    const history = historyRef.current;
    const next = history.future.at(-1);
    if (!next) return null;
    history.future = history.future.slice(0, -1);
    history.past = [
      ...history.past.slice(-(MARKDOWN_SOURCE_HISTORY_LIMIT - 1)),
      history.present,
    ];
    history.present = next;
    history.typing = null;
    refresh();
    return next;
  }, [refresh]);

  return {
    canUndo: historyRef.current.past.length > 0,
    canRedo: historyRef.current.future.length > 0,
    record,
    redo,
    reset,
    undo,
    updateSelection,
    updateVisual,
  };
}

function canCoalesceTyping(
  history: MarkdownSourceHistoryState,
  next: MarkdownHistoryEdit,
  inputType: string | undefined,
  now: number,
): boolean {
  const typing = history.typing;
  if (!typing || now - typing.at > MARKDOWN_TYPING_COALESCE_MS) return false;
  if (
    !isContinuousTypingInput(typing.inputType) ||
    !isContinuousTypingInput(inputType)
  ) {
    return false;
  }
  return (
    history.present.selection.start === history.present.selection.end &&
    next.selection.start === next.selection.end
  );
}

function isContinuousTypingInput(inputType: string | undefined): boolean {
  return (
    !inputType ||
    inputType === 'insertText' ||
    inputType === 'insertCompositionText' ||
    inputType === 'insertLineBreak' ||
    inputType === 'deleteContentBackward' ||
    inputType === 'deleteContentForward'
  );
}

function normalizeSnapshot(snapshot: MarkdownHistoryEdit): MarkdownHistoryEdit {
  return {
    markdown: snapshot.markdown,
    selection: normalizeSelection(snapshot.selection, snapshot.markdown.length),
    visual: snapshot.visual,
  };
}

function normalizeSelection(
  selection: MarkdownSourceSelection,
  length: number,
): MarkdownSourceSelection {
  const start = Math.max(0, Math.min(length, selection.start));
  const end = Math.max(start, Math.min(length, selection.end));
  return { start, end, direction: selection.direction };
}

function collapsedSelection(position: number): MarkdownSourceSelection {
  return { start: position, end: position, direction: 'none' };
}

function sameSelection(
  left: MarkdownSourceSelection,
  right: MarkdownSourceSelection,
): boolean {
  return (
    left.start === right.start &&
    left.end === right.end &&
    left.direction === right.direction
  );
}
