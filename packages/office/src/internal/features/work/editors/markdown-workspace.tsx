import type { Editor } from '@tiptap/core';
import { officeMessage } from '../../../i18n/office-locale';
import { EditorContent } from '@tiptap/react';
import {
  type CSSProperties,
  type FormEvent,
  type KeyboardEvent,
  type PointerEvent,
  type RefObject,
  type UIEvent,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import {
  isWorkspaceContextMenuKeyboardEvent,
  type WorkspaceContextMenuEvent,
} from '../../workspace/components/workspace-context-menu';
import type {
  MarkdownSourceCommand,
  MarkdownSourceSelection,
} from './markdown-source-commands';
import { markdownFormattingShortcut } from './markdown-shortcuts';
import { useOfficeMessages } from './office-messages-context';
import {
  MarkdownSourcePresenceLayer,
  OfficeTiptapPresenceLayer,
} from './office-collaboration-presence-ui';

export type MarkdownViewMode = 'visual' | 'source' | 'split';
type MarkdownCompactPane = 'source' | 'preview';

const DEFAULT_MARKDOWN_SPLIT_PERCENT = 50;
const MIN_MARKDOWN_SPLIT_PERCENT = 30;
const MAX_MARKDOWN_SPLIT_PERCENT = 70;
const MARKDOWN_SPLIT_KEYBOARD_STEP = 5;
/** Readable source column. About forty characters at the note type size. */
const MIN_MARKDOWN_SOURCE_PX = 240;
/** Readable preview column. Narrower than this collapses the reading measure. */
const MIN_MARKDOWN_PREVIEW_PX = 280;
/** Grab target around the 1px border. The line stays 1px; the hit area is wider. */
const MARKDOWN_SPLITTER_PX = 8;

export function proportionalMarkdownScrollTop(
  sourceScrollTop: number,
  sourceScrollHeight: number,
  sourceClientHeight: number,
  targetScrollHeight: number,
  targetClientHeight: number,
): number {
  const sourceRange = Math.max(0, sourceScrollHeight - sourceClientHeight);
  const targetRange = Math.max(0, targetScrollHeight - targetClientHeight);
  if (!sourceRange || !targetRange) return 0;

  const progress = Math.min(1, Math.max(0, sourceScrollTop / sourceRange));
  return progress * targetRange;
}

/** Publish scroll-ownership markers for a3s-test / Playwright regression gates. */
export function publishMarkdownSourceScrollOwnership(
  source: HTMLTextAreaElement,
): void {
  const pane = source.closest('.work-markdown-pane.source');
  source.dataset.sourceScrolled = source.scrollTop > 0 ? 'true' : 'false';
  if (pane instanceof HTMLElement) {
    pane.dataset.paneScrolled = pane.scrollTop > 0 ? 'true' : 'false';
  }
}

export function MarkdownWorkspace({
  editor,
  markdown,
  mode,
  readOnly = false,
  visualReadOnly = readOnly,
  sourceRef,
  sourceSelectionRequest,
  onSourceChange,
  onSourceCommand,
  onSourceContextMenu,
  onSourceIntent,
  onSourceRedo,
  onSourceSelectionChange,
  onSourceUndo,
  onVisualContextMenu,
  onVisualIntent,
  sourcePlaceholder,
}: {
  editor: Editor;
  markdown: string;
  mode: MarkdownViewMode;
  readOnly?: boolean;
  visualReadOnly?: boolean;
  sourceRef: RefObject<HTMLTextAreaElement | null>;
  sourceSelectionRequest?: MarkdownSourceSelection & { revision: number };
  onSourceChange: (
    markdown: string,
    selection: MarkdownSourceSelection,
    inputType?: string,
  ) => void;
  onSourceCommand?: (command: MarkdownSourceCommand) => boolean;
  onSourceContextMenu?: (
    event: WorkspaceContextMenuEvent<HTMLTextAreaElement>,
  ) => boolean;
  onSourceIntent?: () => void;
  onSourceRedo?: () => boolean;
  onSourceSelectionChange?: (selection: MarkdownSourceSelection) => void;
  onSourceUndo?: () => boolean;
  onVisualContextMenu?: (
    event: WorkspaceContextMenuEvent<HTMLElement>,
  ) => boolean;
  onVisualIntent?: () => void;
  sourcePlaceholder?: string;
}) {
  const messages = useOfficeMessages();
  const workspaceRef = useRef<HTMLDivElement>(null);
  const visualRef = useRef<HTMLElement>(null);
  const visualCanvasRef = useRef<HTMLDivElement>(null);
  const synchronizedTargetRef = useRef<'source' | 'visual' | null>(null);
  const releaseFrameRef = useRef<number | null>(null);
  const sourceInputTypeRef = useRef<string | undefined>(undefined);
  const [sourcePanePercent, setSourcePanePercent] = useState(
    DEFAULT_MARKDOWN_SPLIT_PERCENT,
  );
  const [workspaceWidth, setWorkspaceWidth] = useState(0);
  const [compactPane, setCompactPane] = useState<MarkdownCompactPane>('source');
  const [resizing, setResizing] = useState(false);
  const showSource = !readOnly && mode !== 'visual';
  const showVisual = readOnly || mode !== 'source';
  const visualIsReadOnly = readOnly || visualReadOnly;

  const releaseScrollLock = useCallback(() => {
    if (releaseFrameRef.current !== null) {
      cancelAnimationFrame(releaseFrameRef.current);
    }
    releaseFrameRef.current = requestAnimationFrame(() => {
      synchronizedTargetRef.current = null;
      releaseFrameRef.current = null;
    });
  }, []);

  useEffect(
    () => () => {
      if (releaseFrameRef.current !== null) {
        cancelAnimationFrame(releaseFrameRef.current);
      }
    },
    [],
  );

  useEffect(() => {
    const workspace = workspaceRef.current;
    if (!workspace || typeof ResizeObserver === 'undefined') return;
    const measure = () => setWorkspaceWidth(workspace.getBoundingClientRect().width);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(workspace);
    return () => observer.disconnect();
  }, [mode]);

  useLayoutEffect(() => {
    if (!sourceSelectionRequest) return;
    const source = sourceRef.current;
    if (!source) return;
    source.focus({ preventScroll: true });
    source.setSelectionRange(
      sourceSelectionRequest.start,
      sourceSelectionRequest.end,
      sourceSelectionRequest.direction,
    );
    onSourceSelectionChange?.(sourceSelectionRequest);
  }, [onSourceSelectionChange, sourceRef, sourceSelectionRequest]);

  useLayoutEffect(() => {
    const source = sourceRef.current;
    if (!source) return;
    onSourceSelectionChange?.(sourceSelection(source));
  }, [mode, onSourceSelectionChange, sourceRef]);

  const handleSourceScroll = useCallback(
    (event: UIEvent<HTMLTextAreaElement>) => {
      const source = event.currentTarget;
      publishMarkdownSourceScrollOwnership(source);

      if (mode !== 'split') return;
      if (synchronizedTargetRef.current === 'source') {
        synchronizedTargetRef.current = null;
        return;
      }

      const target = visualRef.current;
      if (!target) return;
      synchronizedTargetRef.current = 'visual';
      target.scrollTop = proportionalMarkdownScrollTop(
        source.scrollTop,
        source.scrollHeight,
        source.clientHeight,
        target.scrollHeight,
        target.clientHeight,
      );
      target.dataset.previewScrollSynced =
        target.scrollTop > 0 ? 'true' : 'false';
      releaseScrollLock();
    },
    [mode, releaseScrollLock],
  );

  const handleVisualScroll = useCallback(
    (event: UIEvent<HTMLElement>) => {
      if (mode !== 'split') return;
      if (synchronizedTargetRef.current === 'visual') {
        synchronizedTargetRef.current = null;
        return;
      }

      const source = event.currentTarget;
      const target = sourceRef.current;
      if (!target) return;
      synchronizedTargetRef.current = 'source';
      target.scrollTop = proportionalMarkdownScrollTop(
        source.scrollTop,
        source.scrollHeight,
        source.clientHeight,
        target.scrollHeight,
        target.clientHeight,
      );
      releaseScrollLock();
    },
    [mode, releaseScrollLock],
  );

  const resizeFromPointer = useCallback((clientX: number) => {
    const workspace = workspaceRef.current;
    if (!workspace) return;
    const bounds = workspace.getBoundingClientRect();
    if (bounds.width <= 0) return;
    setSourcePanePercent(
      clampMarkdownSplitPercent(
        ((clientX - bounds.left) / bounds.width) * 100,
        bounds.width,
      ),
    );
  }, []);

  useEffect(() => {
    if (!resizing) return;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
    const move = (event: globalThis.PointerEvent) => {
      event.preventDefault();
      resizeFromPointer(event.clientX);
    };
    const end = () => setResizing(false);
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', end);
    window.addEventListener('pointercancel', end);
    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', end);
      window.removeEventListener('pointercancel', end);
    };
  }, [resizing, resizeFromPointer]);

  const handleResizeStart = (event: PointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    setResizing(true);
    resizeFromPointer(event.clientX);
  };

  const handleResizeKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    let next: number | null = null;
    if (event.key === 'ArrowLeft') {
      next = sourcePanePercent - MARKDOWN_SPLIT_KEYBOARD_STEP;
    } else if (event.key === 'ArrowRight') {
      next = sourcePanePercent + MARKDOWN_SPLIT_KEYBOARD_STEP;
    } else if (event.key === 'Home') {
      next = 0;
    } else if (event.key === 'End') {
      next = 100;
    } else if (event.key === 'Enter') {
      next = DEFAULT_MARKDOWN_SPLIT_PERCENT;
    }
    if (next === null) return;
    event.preventDefault();
    const width = workspaceRef.current?.getBoundingClientRect().width ?? 0;
    setSourcePanePercent(clampMarkdownSplitPercent(next, width));
  };

  const workspaceStyle = {
    '--work-markdown-source-pane': `${sourcePanePercent}%`,
  } as CSSProperties;

  return (
    <div
      ref={workspaceRef}
      className={`work-markdown-workspace ${mode}`}
      data-compact-pane={mode === 'split' ? compactPane : undefined}
      data-split-resizing={resizing || undefined}
      style={workspaceStyle}
    >
      {mode === 'split' && (
        <fieldset className="work-markdown-compact-switch">
          <legend className="sr-only">{officeMessage(messages, 'markdown.workspace.splitLegend')}</legend>
          <button
            type="button"
            aria-label={officeMessage(messages, 'markdown.workspace.showSourceAria')}
            aria-pressed={compactPane === 'source'}
            onClick={() => setCompactPane('source')}
          >
            {officeMessage(messages, 'markdown.workspace.source')}
          </button>
          <button
            type="button"
            aria-label={officeMessage(messages, 'markdown.workspace.showPreviewAria')}
            aria-pressed={compactPane === 'preview'}
            onClick={() => setCompactPane('preview')}
          >
            {officeMessage(messages, 'markdown.workspace.preview')}
          </button>
        </fieldset>
      )}
      {showSource && (
        <section
          aria-label={officeMessage(messages, 'markdown.workspace.sourcePaneAria')}
          className="work-markdown-pane source"
        >
          {mode === 'split' && (
            <header className="work-markdown-pane-label">{officeMessage(messages, 'markdown.workspace.source')}</header>
          )}
          <textarea
            ref={sourceRef}
            aria-label={officeMessage(messages, 'markdown.workspace.sourceAria')}
            placeholder={
              sourcePlaceholder ??
              officeMessage(messages, 'markdown.workspace.sourcePlaceholder')
            }
            value={markdown}
            spellCheck
            onBeforeInput={(event: FormEvent<HTMLTextAreaElement>) => {
              const inputType = (event.nativeEvent as InputEvent).inputType;
              if (inputType === 'historyUndo') {
                event.preventDefault();
                onSourceUndo?.();
                return;
              }
              if (inputType === 'historyRedo') {
                event.preventDefault();
                onSourceRedo?.();
                return;
              }
              sourceInputTypeRef.current = inputType || undefined;
            }}
            onChange={(event) => {
              onSourceIntent?.();
              const source = event.currentTarget;
              onSourceChange(
                source.value,
                sourceSelection(source),
                sourceInputTypeRef.current,
              );
              sourceInputTypeRef.current = undefined;
            }}
            onFocus={onSourceIntent}
            onContextMenu={onSourceContextMenu}
            onKeyDown={(event) => {
              if (
                isWorkspaceContextMenuKeyboardEvent(event) &&
                onSourceContextMenu?.(event)
              ) {
                return;
              }
              if (!(event.metaKey || event.ctrlKey) || event.nativeEvent.isComposing) return;
              const key = event.key.toLocaleLowerCase();
              if (!event.altKey && key === 'z') {
                event.preventDefault();
                if (event.shiftKey) onSourceRedo?.();
                else onSourceUndo?.();
                return;
              }
              if (!event.altKey && key === 'y' && !event.shiftKey) {
                event.preventDefault();
                onSourceRedo?.();
                return;
              }
              const formatting = markdownFormattingShortcut(event);
              if (formatting && onSourceCommand?.(formatting)) {
                event.preventDefault();
                return;
              }
              if (event.altKey || event.shiftKey) return;
              const command =
                key === 'b' ? 'bold' : key === 'i' ? 'italic' : null;
              if (!command || !onSourceCommand?.(command)) return;
              event.preventDefault();
            }}
            onKeyUp={(event) =>
              onSourceSelectionChange?.(sourceSelection(event.currentTarget))
            }
            onPointerDown={onSourceIntent}
            onPointerUp={(event) => {
              onSourceIntent?.();
              onSourceSelectionChange?.(sourceSelection(event.currentTarget));
            }}
            onSelect={(event) => {
              onSourceIntent?.();
              onSourceSelectionChange?.(sourceSelection(event.currentTarget));
            }}
            onScroll={handleSourceScroll}
          />
          <MarkdownSourcePresenceLayer
            markdown={markdown}
            sourceRef={sourceRef}
          />
        </section>
      )}
      {mode === 'split' && (
        <div
          className="work-markdown-splitter"
          role="separator"
          aria-label={officeMessage(messages, 'markdown.workspace.resizeAria')}
          aria-orientation="vertical"
          aria-valuemin={splitBounds(workspaceWidth).min}
          aria-valuemax={splitBounds(workspaceWidth).max}
          aria-valuenow={sourcePanePercent}
          aria-valuetext={officeMessage(messages, 'markdown.workspace.resizeValue', { percent: String(sourcePanePercent) })}
          tabIndex={0}
          title={officeMessage(messages, 'markdown.workspace.resizeTitle')}
          onDoubleClick={() =>
            setSourcePanePercent(DEFAULT_MARKDOWN_SPLIT_PERCENT)
          }
          onKeyDown={handleResizeKeyDown}
          onPointerDown={handleResizeStart}
        />
      )}
      {showVisual && (
        <section
          ref={visualRef}
          aria-label={
            visualIsReadOnly ? officeMessage(messages, 'markdown.workspace.previewPaneAria') : officeMessage(messages, 'markdown.workspace.editPaneAria')
          }
          className="work-markdown-pane visual"
          data-readonly={visualIsReadOnly ? 'true' : 'false'}
          onContextMenu={onVisualContextMenu}
          onKeyDownCapture={(event) => {
            if (!isWorkspaceContextMenuKeyboardEvent(event)) return;
            onVisualContextMenu?.(event);
          }}
          onFocusCapture={onVisualIntent}
          onPointerDownCapture={onVisualIntent}
          onScroll={handleVisualScroll}
        >
          {mode === 'split' && (
            <header className="work-markdown-pane-label">{officeMessage(messages, 'markdown.workspace.preview')}</header>
          )}
          <div ref={visualCanvasRef} className="work-markdown-canvas">
            <EditorContent editor={editor} />
            <OfficeTiptapPresenceLayer
              containerRef={visualCanvasRef}
              editor={editor}
              kind="markdown"
              markdownSurface="visual"
            />
          </div>
        </section>
      )}
    </div>
  );
}

function sourceSelection(source: HTMLTextAreaElement): MarkdownSourceSelection {
  return {
    start: source.selectionStart,
    end: source.selectionEnd,
    direction: source.selectionDirection,
  };
}

function splitBounds(width: number): { min: number; max: number } {
  const room = width - MARKDOWN_SPLITTER_PX;
  const floors =
    width > 0 && room >= MIN_MARKDOWN_SOURCE_PX + MIN_MARKDOWN_PREVIEW_PX;
  if (!floors) {
    return { min: MIN_MARKDOWN_SPLIT_PERCENT, max: MAX_MARKDOWN_SPLIT_PERCENT };
  }
  return {
    min: roundSplitPercent((MIN_MARKDOWN_SOURCE_PX / width) * 100),
    max: roundSplitPercent(
      ((width - MIN_MARKDOWN_PREVIEW_PX - MARKDOWN_SPLITTER_PX) / width) * 100,
    ),
  };
}

function clampMarkdownSplitPercent(value: number, width: number): number {
  const bounds = splitBounds(width);
  return roundSplitPercent(Math.min(bounds.max, Math.max(bounds.min, value)));
}

function roundSplitPercent(value: number): number {
  return Math.round(value * 10) / 10;
}
