import {
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../../lib/cn';
import type { SearchSelectSize } from './SearchSelect';

export type CascaderOption = {
  value: string;
  label: string;
  keywords?: string;
  children?: CascaderOption[];
};

export type CascaderProps = {
  value: string[];
  options: CascaderOption[];
  onChange: (value: string[]) => void;
  placeholder?: string;
  searchPlaceholder?: string;
  emptyText?: string;
  /** Joins the selected labels in the trigger. */
  separator?: string;
  className?: string;
  disabled?: boolean;
  size?: SearchSelectSize;
  searchable?: boolean;
  allowClear?: boolean;
  /**
   * When a chosen level has exactly one child, append that child and keep
   * going. Municipalities use this so 北京市 opens straight onto its counties.
   */
  expandSingleChild?: boolean;
  id?: string;
  'aria-label'?: string;
  'aria-invalid'?: boolean | 'true' | 'false';
  'aria-describedby'?: string;
};

type SearchHit = {
  path: string[];
  label: string;
  hay: string;
};

function childrenAfter(options: CascaderOption[], path: string[]): CascaderOption[] {
  let level = options;
  for (const value of path) {
    const current = level.find((item) => item.value === value);
    if (!current) return [];
    level = current.children ?? [];
  }
  return level;
}

function nodeAt(options: CascaderOption[], path: string[]): CascaderOption | undefined {
  let level = options;
  let current: CascaderOption | undefined;
  for (const value of path) {
    current = level.find((item) => item.value === value);
    if (!current) return undefined;
    level = current.children ?? [];
  }
  return current;
}

function columnsOf(options: CascaderOption[], path: string[]): CascaderOption[][] {
  const columns: CascaderOption[][] = [];
  if (options.length === 0) return columns;
  columns.push(options);
  let level = options;
  for (const value of path) {
    const current = level.find((item) => item.value === value);
    if (!current?.children?.length) break;
    columns.push(current.children);
    level = current.children;
  }
  return columns;
}

function expandSingles(options: CascaderOption[], path: string[]): string[] {
  const next = [...path];
  let children = childrenAfter(options, next);
  while (children.length === 1 && children[0]) {
    next.push(children[0].value);
    children = children[0].children ?? [];
  }
  return next;
}

function pathLabels(options: CascaderOption[], path: string[]): string[] {
  const labels: string[] = [];
  let level = options;
  for (const value of path) {
    const current = level.find((item) => item.value === value);
    if (!current) break;
    labels.push(current.label);
    level = current.children ?? [];
  }
  return labels;
}

function flatten(options: CascaderOption[], prefix: string[], labelPrefix: string, out: SearchHit[]) {
  for (const option of options) {
    const path = [...prefix, option.value];
    const label = labelPrefix ? `${labelPrefix} / ${option.label}` : option.label;
    out.push({
      path,
      label,
      hay: `${label} ${option.keywords ?? ''}`.toLowerCase(),
    });
    if (option.children?.length) flatten(option.children, path, label, out);
  }
}

function Chevron() {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden className="ui-cascader__chevron">
      <path
        d="M4.5 2.5L8 6L4.5 9.5"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/**
 * Column cascader. Choosing a parent opens the next column; choosing a leaf
 * commits the path and closes. The trigger shows the path as one value.
 */
export function Cascader({
  value,
  options,
  onChange,
  placeholder = 'Select…',
  searchPlaceholder = 'Search…',
  emptyText = 'No results',
  separator = ' / ',
  className = '',
  disabled = false,
  size = 'md',
  searchable = true,
  allowClear = true,
  expandSingleChild = false,
  id,
  'aria-label': ariaLabel,
  'aria-invalid': ariaInvalid,
  'aria-describedby': ariaDescribedBy,
}: CascaderProps) {
  const listId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const valueRef = useRef(value);
  valueRef.current = value;
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [activePath, setActivePath] = useState<string[]>(value);
  const [highlight, setHighlight] = useState({ col: 0, index: 0 });
  const [hitIndex, setHitIndex] = useState(0);
  const [coords, setCoords] = useState<{ top: number; left: number; width: number } | null>(null);

  const invalid = ariaInvalid === true || ariaInvalid === 'true';
  const labels = pathLabels(options, value);
  const display = labels.join(separator);
  const searching = query.trim().length > 0;

  const hits = useMemo(() => {
    if (!searching) return [];
    const all: SearchHit[] = [];
    flatten(options, [], '', all);
    const needle = query.trim().toLowerCase();
    return all.filter((hit) => hit.hay.includes(needle)).slice(0, 50);
  }, [options, query, searching]);

  const columns = useMemo(
    () => columnsOf(options, activePath),
    [options, activePath],
  );

  function updatePosition() {
    const el = rootRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const columnCount = searching ? 1 : Math.max(columns.length, 1);
    const columnWidth = size === 'sm' ? 148 : 168;
    const desired = searching
      ? Math.max(rect.width, 320)
      : Math.max(rect.width, columnCount * columnWidth + 12);
    const width = Math.min(desired, window.innerWidth - 16);
    const left = Math.max(8, Math.min(rect.left, window.innerWidth - width - 8));
    let top = rect.bottom + 6;
    const estimatedHeight = 320;
    if (top + estimatedHeight > window.innerHeight - 8 && rect.top > estimatedHeight) {
      top = Math.max(8, rect.top - estimatedHeight - 6);
    }
    setCoords({ top, left, width });
  }

  useLayoutEffect(() => {
    if (!open) {
      setCoords(null);
      return;
    }
    updatePosition();
    const onScroll = () => updatePosition();
    window.addEventListener('resize', onScroll);
    window.addEventListener('scroll', onScroll, true);
    return () => {
      window.removeEventListener('resize', onScroll);
      window.removeEventListener('scroll', onScroll, true);
    };
  }, [open, size, columns.length, searching]);

  useEffect(() => {
    if (!open) return;
    const current = valueRef.current;
    setQuery('');
    setHitIndex(0);
    setActivePath(current);
    const cols = columnsOf(options, current);
    const col = current.length > 0 ? current.length - 1 : 0;
    const index = Math.max(
      0,
      (cols[col] ?? []).findIndex((item) => item.value === current[col]),
    );
    setHighlight({ col, index });
    if (!searchable) return;
    const timer = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => window.clearTimeout(timer);
  }, [open, options, searchable]);

  useEffect(() => {
    if (!open) return;
    const current = panelRef.current?.querySelector('[data-current="true"]');
    current?.scrollIntoView({ block: 'nearest' });
  }, [open, activePath, highlight, hitIndex, searching]);

  useEffect(() => {
    if (!open) return;
    const onDoc = (event: MouseEvent) => {
      const target = event.target as Node;
      if (rootRef.current?.contains(target)) return;
      if (panelRef.current?.contains(target)) return;
      setOpen(false);
    };
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDoc);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDoc);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  function commit(path: string[]) {
    const next = expandSingleChild ? expandSingles(options, path) : path;
    setActivePath(next);
    onChange(next);
    const cols = columnsOf(options, next);
    const col = next.length > 0 ? next.length - 1 : 0;
    const index = Math.max(0, (cols[col] ?? []).findIndex((item) => item.value === next[col]));
    setHighlight({ col, index });
    const tip = nodeAt(options, next);
    if (!tip?.children?.length) setOpen(false);
  }

  function onKeyDown(event: KeyboardEvent) {
    if (!open) {
      if (event.key === 'ArrowDown' || event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        setOpen(true);
      }
      return;
    }
    if (event.key === 'Escape') {
      event.preventDefault();
      setOpen(false);
      return;
    }
    if (searching) {
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        setHitIndex((index) => Math.min(index + 1, Math.max(hits.length - 1, 0)));
      } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        setHitIndex((index) => Math.max(index - 1, 0));
      } else if (event.key === 'Enter') {
        event.preventDefault();
        const hit = hits[hitIndex];
        if (hit) commit(hit.path);
      }
      return;
    }
    const column = columns[highlight.col] ?? [];
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setHighlight((current) => ({
        ...current,
        index: Math.min(current.index + 1, Math.max(column.length - 1, 0)),
      }));
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setHighlight((current) => ({ ...current, index: Math.max(current.index - 1, 0) }));
    } else if (event.key === 'ArrowRight') {
      const option = column[highlight.index];
      if (!option?.children?.length) return;
      event.preventDefault();
      setActivePath(activePath.slice(0, highlight.col).concat(option.value));
      setHighlight({ col: highlight.col + 1, index: 0 });
    } else if (event.key === 'ArrowLeft') {
      if (highlight.col === 0) return;
      event.preventDefault();
      setHighlight((current) => ({ col: current.col - 1, index: 0 }));
    } else if (event.key === 'Enter') {
      event.preventDefault();
      const option = column[highlight.index];
      if (option) commit(activePath.slice(0, highlight.col).concat(option.value));
    }
  }

  return (
    <div
      ref={rootRef}
      className={cn(
        'ui-search-select',
        size === 'sm' && 'ui-search-select--sm',
        open && 'is-open',
        invalid && 'is-invalid',
        className,
      )}
    >
      <button
        type="button"
        id={id}
        className="ui-search-select__trigger"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-label={ariaLabel}
        aria-invalid={invalid || undefined}
        aria-describedby={ariaDescribedBy}
        disabled={disabled}
        onClick={() => !disabled && setOpen((current) => !current)}
        onKeyDown={onKeyDown}
      >
        <span className="ui-search-select__value">
          <span className={cn(!display && 'is-placeholder')}>{display || placeholder}</span>
        </span>
        <span className="inline-flex shrink-0 items-center gap-0.5">
          {allowClear && display && !disabled ? (
            <span
              role="button"
              tabIndex={-1}
              className="inline-flex size-5 items-center justify-center rounded text-ink-faint hover:bg-surface hover:text-ink"
              aria-label="Clear"
              onMouseDown={(event) => {
                event.preventDefault();
                event.stopPropagation();
              }}
              onClick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                setActivePath([]);
                onChange([]);
              }}
            >
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
                <path d="M3 3l6 6M9 3L3 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </span>
          ) : null}
          <svg
            className="ui-search-select__chevron"
            width={size === 'sm' ? 12 : 14}
            height={size === 'sm' ? 12 : 14}
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.75"
            aria-hidden
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </span>
      </button>
      {open && typeof document !== 'undefined'
        ? createPortal(
            <div
              ref={panelRef}
              id={listId}
              className={cn('ui-search-select__panel', size === 'sm' && 'ui-search-select__panel--sm')}
              style={
                coords
                  ? {
                      position: 'fixed',
                      top: coords.top,
                      left: coords.left,
                      width: coords.width,
                      zIndex: 100,
                    }
                  : { position: 'fixed', top: -9999, left: -9999, zIndex: 100 }
              }
            >
              {searchable ? (
                <input
                  ref={inputRef}
                  className="ui-search-select__search"
                  value={query}
                  placeholder={searchPlaceholder}
                  aria-label={searchPlaceholder}
                  onChange={(event) => {
                    setQuery(event.target.value);
                    setHitIndex(0);
                  }}
                  onKeyDown={onKeyDown}
                />
              ) : null}
              {searching ? (
                <ul className="ui-search-select__list ui-cascader__results" role="listbox">
                  {hits.length === 0 ? (
                    <li className="ui-search-select__empty">{emptyText}</li>
                  ) : (
                    hits.map((hit, index) => (
                      <li key={hit.path.join('/')} role="option" aria-selected={index === hitIndex}>
                        <button
                          type="button"
                          data-current={index === hitIndex ? 'true' : undefined}
                          className={cn('ui-search-select__option', index === hitIndex && 'is-active')}
                          onMouseEnter={() => setHitIndex(index)}
                          onClick={() => commit(hit.path)}
                        >
                          <span className="ui-search-select__option-label">{hit.label}</span>
                        </button>
                      </li>
                    ))
                  )}
                </ul>
              ) : columns.length === 0 ? (
                <p className="ui-search-select__empty">{emptyText}</p>
              ) : (
                <div className="ui-cascader__columns">
                  {columns.map((column, colIndex) => (
                    <ul key={column[0]?.value ?? colIndex} className="ui-cascader__column" role="listbox">
                      {column.map((option, index) => {
                        const selected = activePath[colIndex] === option.value;
                        const current = highlight.col === colIndex && highlight.index === index;
                        return (
                          <li key={option.value} role="option" aria-selected={selected}>
                            <button
                              type="button"
                              data-current={current ? 'true' : undefined}
                              className={cn(
                                'ui-search-select__option',
                                selected && 'is-selected',
                                current && 'is-active',
                              )}
                              onMouseEnter={() => setHighlight({ col: colIndex, index })}
                              onClick={() => commit(activePath.slice(0, colIndex).concat(option.value))}
                            >
                              <span className="ui-search-select__option-label">{option.label}</span>
                              {option.children?.length ? <Chevron /> : null}
                            </button>
                          </li>
                        );
                      })}
                    </ul>
                  ))}
                </div>
              )}
            </div>,
            document.body,
          )
        : null}
    </div>
  );
}

function pathKey(path: string[]): string {
  return path.join('\0');
}

function samePath(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((part, index) => part === right[index]);
}

function isAncestorPath(ancestor: string[], path: string[]): boolean {
  return ancestor.length < path.length && ancestor.every((part, index) => part === path[index]);
}

function coversPath(selected: string[][], path: string[]): boolean {
  return selected.some((item) => samePath(item, path) || isAncestorPath(item, path));
}

function hasSelectedDescendant(selected: string[][], path: string[]): boolean {
  return selected.some((item) => isAncestorPath(path, item));
}

function dedupePaths(paths: string[][]): string[][] {
  const seen = new Set<string>();
  const next: string[][] = [];
  for (const path of paths) {
    const key = pathKey(path);
    if (seen.has(key) || path.length === 0) continue;
    seen.add(key);
    next.push(path);
  }
  return next;
}

function rollupPaths(selected: string[][], options: CascaderOption[]): string[][] {
  let current = dedupePaths(selected);
  let changed = true;
  while (changed) {
    changed = false;
    const groups = new Map<string, string[][]>();
    for (const path of current) {
      const parent = path.slice(0, -1);
      const key = pathKey(parent);
      const group = groups.get(key) ?? [];
      group.push(path);
      groups.set(key, group);
    }
    const consumed = new Set<string>();
    const rolled: string[][] = [];
    for (const [key, group] of groups) {
      const parent = key ? key.split('\0') : [];
      if (parent.length === 0) continue;
      const children = childrenAfter(options, parent);
      const complete =
        children.length > 0 &&
        children.every((child) =>
          group.some((path) => path.length === parent.length + 1 && path[path.length - 1] === child.value),
        );
      if (!complete) continue;
      for (const path of group) consumed.add(pathKey(path));
      rolled.push(parent);
      changed = true;
    }
    if (changed) {
      current = dedupePaths([...current.filter((path) => !consumed.has(pathKey(path))), ...rolled]);
    }
  }
  return current;
}

function checkPath(selected: string[][], path: string[], options: CascaderOption[]): string[][] {
  if (path.length === 0 || coversPath(selected, path)) return selected;
  const next = selected.filter((item) => !isAncestorPath(path, item));
  next.push(path);
  return rollupPaths(next, options);
}

function uncheckPath(selected: string[][], path: string[], options: CascaderOption[]): string[][] {
  if (selected.some((item) => samePath(item, path))) {
    return selected.filter((item) => !samePath(item, path));
  }
  const ancestor = selected.find((item) => isAncestorPath(item, path));
  if (!ancestor) return selected;
  const rest = selected.filter((item) => !samePath(item, ancestor));
  const children = childrenAfter(options, ancestor).map((child) => [...ancestor, child.value]);
  return uncheckPath([...rest, ...children], path, options);
}

export type MultiCascaderProps = {
  value: string[][];
  onChange: (value: string[][]) => void;
  options: CascaderOption[];
  rootLabel?: string;
  rootSelected?: boolean;
  onRootChange?: (selected: boolean) => void;
  placeholder?: string;
  searchPlaceholder?: string;
  emptyText?: string;
  separator?: string;
  className?: string;
  disabled?: boolean;
  size?: SearchSelectSize;
  searchable?: boolean;
  allowClear?: boolean;
  id?: string;
  'aria-label'?: string;
  'aria-invalid'?: boolean | 'true' | 'false';
  'aria-describedby'?: string;
};

function CheckBox({
  checked,
  indeterminate,
  label,
  onChange,
}: {
  checked: boolean;
  indeterminate: boolean;
  label: string;
  onChange: () => void;
}) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = indeterminate && !checked;
  }, [checked, indeterminate]);
  return (
    <input
      ref={ref}
      type="checkbox"
      className="ui-cascader__check"
      checked={checked}
      aria-label={label}
      onClick={(event) => event.stopPropagation()}
      onChange={onChange}
    />
  );
}

/**
 * Multiple column cascader. A checked parent covers its children. Checking
 * every child rolls back up to the parent. An optional root choice (全国)
 * replaces the path selection.
 */
export function MultiCascader({
  value,
  onChange,
  options,
  rootLabel,
  rootSelected = false,
  onRootChange,
  placeholder = 'Select…',
  searchPlaceholder = 'Search…',
  emptyText = 'No results',
  separator = ' / ',
  className = '',
  disabled = false,
  size = 'md',
  searchable = true,
  allowClear = true,
  id,
  'aria-label': ariaLabel,
  'aria-invalid': ariaInvalid,
  'aria-describedby': ariaDescribedBy,
}: MultiCascaderProps) {
  const listId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const valueRef = useRef(value);
  valueRef.current = value;
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [activePath, setActivePath] = useState<string[]>([]);
  const [highlight, setHighlight] = useState({ col: 0, index: 0 });
  const [hitIndex, setHitIndex] = useState(0);
  const [coords, setCoords] = useState<{ top: number; left: number; width: number } | null>(null);

  const invalid = ariaInvalid === true || ariaInvalid === 'true';
  const searching = query.trim().length > 0;
  const labels = rootSelected && rootLabel
    ? [rootLabel]
    : value.map((path) => pathLabels(options, path).join(separator)).filter(Boolean);
  const display = labels.join('、');

  const hits = useMemo(() => {
    if (!searching) return [];
    const all: SearchHit[] = [];
    flatten(options, [], '', all);
    const needle = query.trim().toLowerCase();
    return all.filter((hit) => hit.hay.includes(needle)).slice(0, 50);
  }, [options, query, searching]);

  const columns = useMemo(() => columnsOf(options, activePath), [options, activePath]);

  function updatePosition() {
    const el = rootRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const width = Math.min(rect.width, window.innerWidth - 16);
    const left = Math.max(8, Math.min(rect.left, window.innerWidth - width - 8));
    let top = rect.bottom + 6;
    const estimatedHeight = 320;
    if (top + estimatedHeight > window.innerHeight - 8 && rect.top > estimatedHeight) {
      top = Math.max(8, rect.top - estimatedHeight - 6);
    }
    setCoords({ top, left, width });
  }

  useLayoutEffect(() => {
    if (!open) {
      setCoords(null);
      return;
    }
    updatePosition();
    const onScroll = () => updatePosition();
    window.addEventListener('resize', onScroll);
    window.addEventListener('scroll', onScroll, true);
    return () => {
      window.removeEventListener('resize', onScroll);
      window.removeEventListener('scroll', onScroll, true);
    };
  }, [open, size, columns.length, searching]);

  useEffect(() => {
    if (!open) return;
    const current = valueRef.current[0] ?? [];
    setQuery('');
    setHitIndex(0);
    setActivePath(current);
    const cols = columnsOf(options, current);
    const col = current.length > 0 ? current.length - 1 : 0;
    const index = Math.max(0, (cols[col] ?? []).findIndex((item) => item.value === current[col]));
    setHighlight({ col, index });
    if (!searchable) return;
    const timer = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => window.clearTimeout(timer);
  }, [open, options, searchable]);

  useEffect(() => {
    if (!open) return;
    const onDoc = (event: MouseEvent) => {
      const target = event.target as Node;
      if (rootRef.current?.contains(target)) return;
      if (panelRef.current?.contains(target)) return;
      setOpen(false);
    };
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDoc);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDoc);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  function toggle(path: string[]) {
    onRootChange?.(false);
    const next = coversPath(value, path) ? uncheckPath(value, path, options) : checkPath(value, path, options);
    onChange(next);
  }

  function chooseRoot() {
    onRootChange?.(!rootSelected);
  }

  return (
    <div
      ref={rootRef}
      className={cn(
        'ui-search-select',
        'ui-cascader',
        size === 'sm' && 'ui-search-select--sm',
        open && 'is-open',
        invalid && 'is-invalid',
        className,
      )}
    >
      <button
        type="button"
        id={id}
        className="ui-search-select__trigger"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-label={ariaLabel}
        aria-invalid={invalid || undefined}
        aria-describedby={ariaDescribedBy}
        disabled={disabled}
        title={labels.length > 1 ? display : undefined}
        onClick={() => !disabled && setOpen((current) => !current)}
      >
        <span className="ui-search-select__value">
          {labels.length === 0 ? (
            <span className="is-placeholder">{placeholder}</span>
          ) : (
            <span className="ui-cascader__tags">
              <span className="ui-cascader__tag" title={labels[0]}>
                <span className="ui-cascader__tag-label">{labels[0]}</span>
              </span>
              {labels.length > 1 ? (
                <span className="ui-cascader__more" title={labels.slice(1).join('、')}>
                  +{labels.length - 1}
                </span>
              ) : null}
            </span>
          )}
        </span>
        <span className="inline-flex shrink-0 items-center gap-0.5">
          {allowClear && display && !disabled ? (
            <span
              role="button"
              tabIndex={-1}
              className="inline-flex size-5 items-center justify-center rounded text-ink-faint hover:bg-surface hover:text-ink"
              aria-label="Clear"
              onMouseDown={(event) => {
                event.preventDefault();
                event.stopPropagation();
              }}
              onClick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                onRootChange?.(false);
                onChange([]);
              }}
            >
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
                <path d="M3 3l6 6M9 3L3 9" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </span>
          ) : null}
          <svg
            className="ui-search-select__chevron"
            width={size === 'sm' ? 12 : 14}
            height={size === 'sm' ? 12 : 14}
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.75"
            aria-hidden
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </span>
      </button>
      {open && typeof document !== 'undefined'
        ? createPortal(
            <div
              ref={panelRef}
              id={listId}
              className={cn('ui-search-select__panel', size === 'sm' && 'ui-search-select__panel--sm')}
              style={
                coords
                  ? { position: 'fixed', top: coords.top, left: coords.left, width: coords.width, zIndex: 100 }
                  : { position: 'fixed', top: -9999, left: -9999, zIndex: 100 }
              }
            >
              {searchable ? (
                <input
                  ref={inputRef}
                  className="ui-search-select__search"
                  value={query}
                  placeholder={searchPlaceholder}
                  aria-label={searchPlaceholder}
                  onChange={(event) => {
                    setQuery(event.target.value);
                    setHitIndex(0);
                  }}
                  onKeyDown={(event) => {
                    if (event.key === 'ArrowDown') {
                      event.preventDefault();
                      setHitIndex((index) => Math.min(index + 1, Math.max(hits.length - 1, 0)));
                    } else if (event.key === 'ArrowUp') {
                      event.preventDefault();
                      setHitIndex((index) => Math.max(index - 1, 0));
                    } else if (event.key === 'Enter' && searching) {
                      event.preventDefault();
                      const hit = hits[hitIndex];
                      if (hit) toggle(hit.path);
                    } else if (event.key === 'Escape') {
                      event.preventDefault();
                      setOpen(false);
                    }
                  }}
                />
              ) : null}
              {rootLabel ? (
                <label className="ui-cascader__root">
                  <CheckBox checked={rootSelected} indeterminate={false} label={rootLabel} onChange={chooseRoot} />
                  <span>{rootLabel}</span>
                </label>
              ) : null}
              {searching ? (
                <ul className="ui-search-select__list ui-cascader__results" role="listbox">
                  {hits.length === 0 ? (
                    <li className="ui-search-select__empty">{emptyText}</li>
                  ) : (
                    hits.map((hit, index) => {
                      const checked = coversPath(value, hit.path);
                      return (
                        <li key={hit.path.join('/')} role="option" aria-selected={checked}>
                          <div className={cn('ui-cascader__row', index === hitIndex && 'is-active')}>
                            <CheckBox
                              checked={checked}
                              indeterminate={!checked && hasSelectedDescendant(value, hit.path)}
                              label={hit.label}
                              onChange={() => toggle(hit.path)}
                            />
                            <button
                              type="button"
                              className="ui-search-select__option"
                              onMouseEnter={() => setHitIndex(index)}
                              onClick={() => toggle(hit.path)}
                            >
                              <span className="ui-search-select__option-label">{hit.label}</span>
                            </button>
                          </div>
                        </li>
                      );
                    })
                  )}
                </ul>
              ) : columns.length === 0 ? (
                <p className="ui-search-select__empty">{emptyText}</p>
              ) : (
                <div className="ui-cascader__columns ui-cascader__columns--fit">
                  {columns.map((column, colIndex) => (
                    <ul key={colIndex} className="ui-cascader__column" role="listbox">
                      {column.map((option, index) => {
                        const path = activePath.slice(0, colIndex).concat(option.value);
                        const checked = coversPath(value, path);
                        const current = highlight.col === colIndex && highlight.index === index;
                        return (
                          <li key={option.value} role="option" aria-selected={checked}>
                            <div className={cn('ui-cascader__row', current && 'is-active')}>
                              <CheckBox
                                checked={checked}
                                indeterminate={!checked && hasSelectedDescendant(value, path)}
                                label={option.label}
                                onChange={() => toggle(path)}
                              />
                              <button
                                type="button"
                                data-current={current ? 'true' : undefined}
                                className={cn('ui-search-select__option', checked && 'is-selected')}
                                onMouseEnter={() => setHighlight({ col: colIndex, index })}
                                onClick={() => {
                                  const next = expandSingles(options, path);
                                  setActivePath(next);
                                  setHighlight({ col: colIndex, index });
                                }}
                              >
                                <span className="ui-search-select__option-label">{option.label}</span>
                                {option.children?.length ? <Chevron /> : null}
                              </button>
                            </div>
                          </li>
                        );
                      })}
                    </ul>
                  ))}
                </div>
              )}
            </div>,
            document.body,
          )
        : null}
    </div>
  );
}
