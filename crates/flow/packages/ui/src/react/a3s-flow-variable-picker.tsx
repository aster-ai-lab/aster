import {
  createContext,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type InputHTMLAttributes,
  type KeyboardEvent,
  type ReactNode,
  type RefObject,
  type TextareaHTMLAttributes,
} from 'react';
import { createPortal } from 'react-dom';
import { DesignerIcon } from './designer-icons';
import { useFloatingPanelPosition, type FloatingPanelPositionOptions } from './floating-panel';

export type A3sFlowExpressionVariableGroup = 'input' | 'global' | 'upstream' | 'scope';

export interface A3sFlowExpressionVariable {
  dataType?: string;
  description?: string;
  group: A3sFlowExpressionVariableGroup;
  label: string;
  nodeId?: string;
  path: string;
}

export const A3S_FLOW_DEFAULT_EXPRESSION_VARIABLES: readonly A3sFlowExpressionVariable[] = [
  {
    group: 'input',
    label: 'Workflow input',
    path: 'input',
    dataType: 'object',
  },
  {
    group: 'global',
    label: 'Workflow run ID',
    path: 'global.run_id',
    dataType: 'string',
  },
  {
    group: 'global',
    label: 'Workflow name',
    path: 'global.workflow_name',
    dataType: 'string',
  },
];

const A3sFlowExpressionVariablesContext = createContext<
  readonly A3sFlowExpressionVariable[] | undefined
>(undefined);

export function A3sFlowExpressionVariablesProvider({
  children,
  variables,
}: {
  children?: ReactNode;
  variables?: readonly A3sFlowExpressionVariable[];
}) {
  return (
    <A3sFlowExpressionVariablesContext.Provider value={variables}>
      {children}
    </A3sFlowExpressionVariablesContext.Provider>
  );
}

export function useA3sFlowExpressionVariables(
  variables?: readonly A3sFlowExpressionVariable[],
): readonly A3sFlowExpressionVariable[] {
  const providedVariables = useContext(A3sFlowExpressionVariablesContext);
  return variables ?? providedVariables ?? A3S_FLOW_DEFAULT_EXPRESSION_VARIABLES;
}

function isChinese(locale: string): boolean {
  return locale.toLocaleLowerCase().startsWith('zh');
}

function normalizedSearch(value: string): string {
  return value.trim().replace(/^\$/u, '').toLocaleLowerCase();
}

export function filterA3sFlowExpressionVariables(
  variables: readonly A3sFlowExpressionVariable[],
  query: string,
): A3sFlowExpressionVariable[] {
  const normalized = normalizedSearch(query);
  if (!normalized) return [...variables];
  const terms = normalized.split(/\s+/u).filter(Boolean);
  return variables.filter((variable) => {
    const haystack = [
      variable.path,
      variable.label,
      variable.description,
      variable.dataType,
      variable.nodeId,
    ]
      .filter(Boolean)
      .join(' ')
      .toLocaleLowerCase();
    return terms.every((term) => haystack.includes(term));
  });
}

const SUGGESTION_PANEL: FloatingPanelPositionOptions = {
  align: 'end',
  gap: 8,
  maxHeight: 360,
  minWidth: 240,
  viewportMargin: 8,
  width: (anchorRect) => Math.min(360, Math.max(anchorRect.width, 240)),
};

/**
 * The expression control draws an icon beside the text field. Anchoring to
 * that whole control keeps the menu on the same column as the field below it.
 */
export function suggestionAnchor(root: HTMLElement | null): HTMLElement | null {
  if (!root) return null;
  return (root.closest('.a3s-form-flow-expression-source') as HTMLElement | null) ?? root;
}

const FORM_TOKEN_NAMES = [
  '--a3s-form-panel',
  '--a3s-form-panel-soft',
  '--a3s-form-ink',
  '--a3s-form-muted',
  '--a3s-form-subtle',
  '--a3s-form-faint',
  '--a3s-form-line',
  '--a3s-form-line-strong',
  '--a3s-form-blue',
  '--font-mono',
] as const;

const useIsoLayoutEffect = typeof window === 'undefined' ? useEffect : useLayoutEffect;

export function applySuggestionFormTokens(
  source: HTMLElement | null,
  menu: HTMLElement | null,
): void {
  const panel = source?.closest('.a3s-form-workflow-node-panel');
  if (!panel || !menu) return;
  const styles = getComputedStyle(panel);
  for (const name of FORM_TOKEN_NAMES) {
    const value = styles.getPropertyValue(name).trim();
    if (value) menu.style.setProperty(name, value);
  }
}

function useVariableSuggestionPosition(
  rootRef: RefObject<HTMLElement | null>,
  panelRef: RefObject<HTMLElement | null>,
  open: boolean,
) {
  const anchorRef = useRef<HTMLElement | null>(null);
  // The field node already exists from the previous commit when the menu opens.
  // Assigning it here makes the floating layout effect measure the column,
  // not the icon-inset text field.
  anchorRef.current = suggestionAnchor(rootRef.current);
  useIsoLayoutEffect(() => {
    if (!open) return;
    applySuggestionFormTokens(rootRef.current, panelRef.current);
  }, [open, panelRef, rootRef]);
  useFloatingPanelPosition(anchorRef, panelRef, open, SUGGESTION_PANEL);
}

function groupLabel(group: A3sFlowExpressionVariableGroup, chinese: boolean): string {
  const labels = chinese
    ? {
        global: '全局变量',
        input: '工作流输入',
        scope: '当前子流程',
        upstream: '上游节点输出',
      }
    : {
        global: 'Global variables',
        input: 'Workflow input',
        scope: 'Current scope',
        upstream: 'Upstream outputs',
      };
  return labels[group];
}

export function VariableSuggestionList({
  activeIndex,
  id,
  locale,
  onActiveIndexChange,
  onSelect,
  panelRef,
  query,
  variables,
}: {
  activeIndex: number;
  id: string;
  locale: string;
  onActiveIndexChange: (index: number) => void;
  onSelect: (variable: A3sFlowExpressionVariable) => void;
  panelRef?: RefObject<HTMLDivElement | null>;
  query: string;
  variables: readonly A3sFlowExpressionVariable[];
}) {
  const chinese = isChinese(locale);
  const filtered = useMemo(
    () => filterA3sFlowExpressionVariables(variables, query),
    [query, variables],
  );
  const grouped = useMemo(() => {
    const groups = new Map<
      A3sFlowExpressionVariableGroup,
      { index: number; variable: A3sFlowExpressionVariable }[]
    >();
    filtered.forEach((variable, index) => {
      const entries = groups.get(variable.group) ?? [];
      entries.push({ index, variable });
      groups.set(variable.group, entries);
    });
    return groups;
  }, [filtered]);

  const list = (
    <div
      aria-label={chinese ? '变量智能感知' : 'Variable suggestions'}
      className="a3s-flow-variable-suggestions"
      id={id}
      ref={panelRef}
      role="listbox"
    >
      <header>
        <span aria-hidden="true">$</span>
        <strong>{chinese ? '引用变量' : 'Reference a variable'}</strong>
        <kbd>↑↓</kbd>
        <kbd>Enter</kbd>
      </header>
      {filtered.length === 0 ? (
        <p>{chinese ? `没有匹配“${query}”的变量` : `No variables match “${query}”`}</p>
      ) : (
        <div className="a3s-flow-variable-suggestions__groups">
          {[...grouped.entries()].map(([group, entries]) => (
            <section aria-label={groupLabel(group, chinese)} key={group}>
              <h4>{groupLabel(group, chinese)}</h4>
              {entries.map(({ index, variable }) => (
                <button
                  aria-selected={index === activeIndex}
                  className={index === activeIndex ? 'is-active' : undefined}
                  data-variable-option=""
                  key={`${variable.group}-${variable.path}`}
                  onMouseDown={(event) => event.preventDefault()}
                  onMouseEnter={() => onActiveIndexChange(index)}
                  onClick={() => onSelect(variable)}
                  role="option"
                  type="button"
                >
                  <span>
                    <code>${variable.path}</code>
                    <small>{variable.label}</small>
                  </span>
                  {variable.dataType && <em>{variable.dataType}</em>}
                </button>
              ))}
            </section>
          ))}
        </div>
      )}
    </div>
  );
  if (typeof document === 'undefined') return list;
  return createPortal(list, document.body);
}

export function useVariableSuggestionState(variables: readonly A3sFlowExpressionVariable[]) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const filtered = useMemo(
    () => filterA3sFlowExpressionVariables(variables, query),
    [query, variables],
  );

  useEffect(() => setActiveIndex(0), [query]);

  const close = () => {
    setOpen(false);
    setQuery('');
    setActiveIndex(0);
  };

  const handleKeyDown = (
    event: KeyboardEvent<HTMLElement>,
    onSelect: (variable: A3sFlowExpressionVariable) => void,
  ): boolean => {
    if (!open) return false;
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close();
      return true;
    }
    if (filtered.length === 0) return false;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const direction = event.key === 'ArrowDown' ? 1 : -1;
      setActiveIndex((current) => (current + direction + filtered.length) % filtered.length);
      return true;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      onSelect(filtered[Math.min(activeIndex, filtered.length - 1)]);
      close();
      return true;
    }
    return false;
  };

  return {
    activeIndex,
    close,
    filtered,
    handleKeyDown,
    open,
    query,
    setActiveIndex,
    setOpen,
    setQuery,
  };
}

function useOutsideDismiss(
  ref: RefObject<HTMLElement | null>,
  panelRef: RefObject<HTMLElement | null>,
  open: boolean,
  onClose: () => void,
) {
  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (ref.current?.contains(target) || panelRef.current?.contains(target)) return;
      onClose();
    };
    document.addEventListener('pointerdown', handlePointerDown, true);
    return () => document.removeEventListener('pointerdown', handlePointerDown, true);
  }, [onClose, open, panelRef, ref]);
}

export interface VariableReferenceInputProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, 'onChange' | 'value'> {
  locale: string;
  onPathChange: (path: string) => void;
  path: string;
  variables?: readonly A3sFlowExpressionVariable[];
}

export function VariableReferenceInput({
  locale,
  onPathChange,
  path,
  variables = A3S_FLOW_DEFAULT_EXPRESSION_VARIABLES,
  ...props
}: VariableReferenceInputProps) {
  const generatedId = useId();
  const suggestionsId = `${props.id ?? generatedId}-variables`;
  const rootRef = useRef<HTMLDivElement>(null);
  const suggestionsRef = useRef<HTMLDivElement>(null);
  const state = useVariableSuggestionState(variables);
  useOutsideDismiss(rootRef, suggestionsRef, state.open, state.close);
  useVariableSuggestionPosition(rootRef, suggestionsRef, state.open);

  const choose = (variable: A3sFlowExpressionVariable) => {
    onPathChange(variable.path);
    state.close();
  };

  return (
    <div className="a3s-flow-variable-input" data-open={state.open || undefined} ref={rootRef}>
      <span aria-hidden="true" className="a3s-flow-variable-input__sigil">
        $
      </span>
      <input
        {...props}
        aria-controls={state.open ? suggestionsId : undefined}
        aria-expanded={state.open}
        aria-haspopup="listbox"
        autoComplete="off"
        className={['input', props.className].filter(Boolean).join(' ')}
        value={path}
        onChange={(event) => {
          const raw = event.target.value;
          const marker = raw.lastIndexOf('$');
          const next = marker < 0 ? raw : `${raw.slice(0, marker)}${raw.slice(marker + 1)}`;
          onPathChange(next);
          if (marker >= 0) {
            state.setQuery(raw.slice(marker + 1));
            state.setOpen(true);
          } else if (state.open) {
            state.setQuery(next);
          }
        }}
        onKeyDown={(event) => {
          if (state.handleKeyDown(event, choose)) return;
          props.onKeyDown?.(event);
        }}
      />
      <button
        aria-label={isChinese(locale) ? '选择变量' : 'Choose variable'}
        className="a3s-flow-variable-input__trigger"
        disabled={props.disabled}
        onClick={() => {
          state.setQuery(path);
          state.setOpen((current) => !current);
        }}
        onKeyDown={(event) => {
          state.handleKeyDown(event, choose);
        }}
        title={isChinese(locale) ? '输入 $ 或点击选择变量' : 'Type $ or choose a variable'}
        type="button"
      >
        <DesignerIcon name="list" size={13} />
      </button>
      {state.open && (
        <VariableSuggestionList
          activeIndex={state.activeIndex}
          id={suggestionsId}
          locale={locale}
          onActiveIndexChange={state.setActiveIndex}
          onSelect={choose}
          panelRef={suggestionsRef}
          query={state.query}
          variables={variables}
        />
      )}
    </div>
  );
}

export interface VariableTemplateTextareaProps
  extends Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, 'onChange' | 'value'> {
  locale: string;
  onValueChange: (value: string) => void;
  value: string;
  variables?: readonly A3sFlowExpressionVariable[];
}

export function VariableTemplateTextarea({
  locale,
  onValueChange,
  value,
  variables = A3S_FLOW_DEFAULT_EXPRESSION_VARIABLES,
  ...props
}: VariableTemplateTextareaProps) {
  const generatedId = useId();
  const suggestionsId = `${props.id ?? generatedId}-variables`;
  const rootRef = useRef<HTMLDivElement>(null);
  const suggestionsRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const markerRef = useRef<number | undefined>(undefined);
  const selectionRef = useRef<{ start: number; end: number } | null>(null);
  const state = useVariableSuggestionState(variables);
  useOutsideDismiss(rootRef, suggestionsRef, state.open, state.close);
  useVariableSuggestionPosition(rootRef, suggestionsRef, state.open);

  const rememberSelection = (textarea = textareaRef.current) => {
    if (!textarea) return;
    selectionRef.current = {
      start: textarea.selectionStart,
      end: textarea.selectionEnd,
    };
  };

  const choose = (variable: A3sFlowExpressionVariable) => {
    const textarea = textareaRef.current;
    const activeSelection =
      textarea && document.activeElement === textarea
        ? {
            start: textarea.selectionStart,
            end: textarea.selectionEnd,
          }
        : undefined;
    const selection = selectionRef.current ?? activeSelection;
    const caret = selection?.start ?? value.length;
    const end = selection?.end ?? caret;
    const start = markerRef.current ?? caret;
    const boundedStart = Math.max(0, Math.min(value.length, start));
    const boundedEnd = Math.max(boundedStart, Math.min(value.length, Math.max(end, boundedStart)));
    const replacement = `{{${variable.path}}}`;
    onValueChange(`${value.slice(0, boundedStart)}${replacement}${value.slice(boundedEnd)}`);
    markerRef.current = undefined;
    state.close();
    requestAnimationFrame(() => {
      const nextCaret = boundedStart + replacement.length;
      selectionRef.current = { start: nextCaret, end: nextCaret };
      textareaRef.current?.focus();
      textareaRef.current?.setSelectionRange(nextCaret, nextCaret);
    });
  };

  return (
    <div className="a3s-flow-variable-textarea" data-open={state.open || undefined} ref={rootRef}>
      <textarea
        {...props}
        aria-controls={state.open ? suggestionsId : undefined}
        aria-expanded={state.open}
        aria-haspopup="listbox"
        className={['textarea', props.className].filter(Boolean).join(' ')}
        onChange={(event) => {
          const next = event.target.value;
          onValueChange(next);
          const caret = event.target.selectionStart ?? next.length;
          selectionRef.current = {
            start: caret,
            end: event.target.selectionEnd ?? caret,
          };
          const marker = next.lastIndexOf('$', Math.max(0, caret - 1));
          const query = marker >= 0 ? next.slice(marker + 1, caret) : '';
          if (marker >= 0 && /^[\w.-]*$/u.test(query)) {
            markerRef.current = marker;
            state.setQuery(query);
            state.setOpen(true);
          } else {
            markerRef.current = undefined;
            state.close();
          }
        }}
        onBlur={(event) => {
          rememberSelection(event.currentTarget);
          props.onBlur?.(event);
        }}
        onClick={(event) => {
          rememberSelection(event.currentTarget);
          props.onClick?.(event);
        }}
        onFocus={(event) => {
          rememberSelection(event.currentTarget);
          props.onFocus?.(event);
        }}
        onKeyDown={(event) => {
          if (state.handleKeyDown(event, choose)) return;
          props.onKeyDown?.(event);
        }}
        onKeyUp={(event) => {
          rememberSelection(event.currentTarget);
          props.onKeyUp?.(event);
        }}
        onSelect={(event) => {
          rememberSelection(event.currentTarget);
          props.onSelect?.(event);
        }}
        ref={textareaRef}
        value={value}
      />
      <button
        aria-label={isChinese(locale) ? '插入变量' : 'Insert variable'}
        className="a3s-flow-variable-textarea__trigger"
        disabled={props.disabled}
        onMouseDown={() => {
          // Capture the active caret before the trigger moves focus. The
          // textarea's blur handler also records it for keyboard users.
          if (document.activeElement === textareaRef.current) {
            rememberSelection();
          }
        }}
        onClick={() => {
          markerRef.current = undefined;
          state.setQuery('');
          state.setOpen((current) => !current);
        }}
        onKeyDown={(event) => {
          state.handleKeyDown(event, choose);
        }}
        title={isChinese(locale) ? '输入 $ 或点击插入变量' : 'Type $ or insert a variable'}
        type="button"
      >
        <span aria-hidden="true">$</span>
        {isChinese(locale) ? '变量' : 'Variable'}
      </button>
      {state.open && (
        <VariableSuggestionList
          activeIndex={state.activeIndex}
          id={suggestionsId}
          locale={locale}
          onActiveIndexChange={state.setActiveIndex}
          onSelect={choose}
          panelRef={suggestionsRef}
          query={state.query}
          variables={variables}
        />
      )}
    </div>
  );
}
