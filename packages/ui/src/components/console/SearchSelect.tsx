import {
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactElement,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../../lib/cn';
import { menuCardClass, menuRowClass, menuRowSelectedClass } from '../../lib/menu-surface';
import { usePortalContainer } from '../primitives/portal-container';

function wheelDeltaY(event: WheelEvent, el: HTMLElement) {
  if (event.deltaMode === 1) return event.deltaY * 16;
  if (event.deltaMode === 2) return event.deltaY * el.clientHeight;
  return event.deltaY;
}

function revealOption(list: HTMLElement, option: HTMLElement) {
  const delta = option.getBoundingClientRect().top - list.getBoundingClientRect().top;
  if (delta < 0) {
    list.scrollTop += delta;
    return;
  }
  const overflow = delta + option.offsetHeight - list.clientHeight;
  if (overflow > 0) list.scrollTop += overflow;
}

export type SearchSelectOption = {
  value: string;
  label: string;
  keywords?: string;
  /** Optional leading mark (logo / avatar) shown in trigger and list. */
  icon?: ReactNode;
};

export type SearchSelectSize = 'sm' | 'md';

type SearchSelectBaseProps = {
  options: SearchSelectOption[];
  placeholder?: string;
  searchPlaceholder?: string;
  emptyText?: string;
  className?: string;
  disabled?: boolean;
  /** sm = compact toolbar / card header filters */
  size?: SearchSelectSize;
  /** When false, omit the in-panel search field (short option lists). */
  searchable?: boolean;
  /**
   * Trailing debounce for the in-panel search filtering, in milliseconds.
   * 0 (default) filters on every keystroke.
   */
  searchDebounceMs?: number;
  /** Show clear control when a value is selected. Default true. */
  allowClear?: boolean;
  id?: string;
  /** Stable hook for tests and end-to-end runs, set on the root. */
  'data-ui'?: string;
  'aria-label'?: string;
  'aria-invalid'?: boolean | 'true' | 'false';
  'aria-describedby'?: string;
};

export type SearchSelectProps = SearchSelectBaseProps & {
  multiple?: false;
  value: string;
  onChange: (value: string) => void;
  /** Value written on clear. Default ''. */
  clearValue?: string;
};

export type MultiSearchSelectProps = SearchSelectBaseProps & {
  multiple: true;
  value: readonly string[];
  onChange: (value: string[]) => void;
};

/**
 * Searchable select — panel portaled to body so dialog overflow does not clip the list.
 * `multiple` keeps the panel open and toggles each option.
 */
export function SearchSelect(props: SearchSelectProps): ReactElement;
export function SearchSelect(props: MultiSearchSelectProps): ReactElement;
export function SearchSelect(props: SearchSelectProps | MultiSearchSelectProps) {
  const portalContainer = usePortalContainer();
  const {
    options,
    placeholder = 'Select…',
    searchPlaceholder = 'Search…',
    emptyText = 'No results',
    className = '',
    disabled = false,
    size = 'md',
    searchable = true,
    searchDebounceMs = 0,
    allowClear = true,
    id,
    'data-ui': dataUi,
    'aria-label': ariaLabel,
    'aria-invalid': ariaInvalid,
    'aria-describedby': ariaDescribedBy,
  } = props;
  const multiple = props.multiple === true;
  const clearValue = props.multiple ? '' : (props.clearValue ?? '');
  const selectedOptions = multiple
    ? options.filter((option) => props.value.includes(option.value))
    : options.filter((option) => option.value === props.value);
  const listId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [instantMotion, setInstantMotion] = useState(false);
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [active, setActive] = useState(0);
  const [coords, setCoords] = useState<{
    top: number;
    left: number;
    width: number;
  } | null>(null);

  const invalid = ariaInvalid === true || ariaInvalid === 'true';
  const summary = selectedOptions.map((option) => option.label).join('、');
  const isPlaceholder = selectedOptions.length === 0;
  const debounceMs = Math.max(0, searchDebounceMs);

  useLayoutEffect(() => {
    if (!open) {
      setInstantMotion(false);
      return;
    }
    // Inactive webviews can leave an entrance animation frozen at opacity zero.
    const settle = () => {
      if (document.hidden || !document.hasFocus()) setInstantMotion(true);
    };
    settle();
    window.addEventListener('blur', settle);
    document.addEventListener('visibilitychange', settle);
    return () => {
      window.removeEventListener('blur', settle);
      document.removeEventListener('visibilitychange', settle);
    };
  }, [open]);

  useEffect(() => {
    if (debounceMs <= 0) return;
    const timer = window.setTimeout(() => setDebouncedQuery(query), debounceMs);
    return () => window.clearTimeout(timer);
  }, [query, debounceMs]);

  const effectiveQuery = debounceMs > 0 ? debouncedQuery : query;
  const filtered = useMemo(() => {
    if (!searchable) return options;
    const q = effectiveQuery.trim().toLowerCase();
    if (!q) return options;
    return options.filter((o) => {
      const hay = `${o.label} ${o.keywords ?? ''} ${o.value}`.toLowerCase();
      return hay.includes(q);
    });
  }, [options, effectiveQuery, searchable]);

  function updatePosition() {
    const el = rootRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const panelWidth = Math.min(Math.max(rect.width, size === 'sm' ? 10 * 16 : 12 * 16), window.innerWidth - 16);
    let left = rect.left;
    left = Math.max(8, Math.min(left, window.innerWidth - panelWidth - 8));
    let top = rect.bottom + 6;
    const estimatedHeight = 240;
    if (
      top + estimatedHeight > window.innerHeight - 8 &&
      rect.top > estimatedHeight
    ) {
      top = Math.max(8, rect.top - estimatedHeight - 6);
    }
    // A dialog's transformed content is the containing block for its nested panel.
    const parent = portalContainer?.getBoundingClientRect();
    setCoords({
      top: top - (parent?.top ?? 0) + (portalContainer?.scrollTop ?? 0),
      left: left - (parent?.left ?? 0) + (portalContainer?.scrollLeft ?? 0),
      width: panelWidth,
    });
  }

  useLayoutEffect(() => {
    if (!open) {
      setCoords(null);
      return;
    }
    updatePosition();
    const onScroll = (event: Event) => {
      const target = event.target;
      if (target instanceof Node && panelRef.current?.contains(target)) return;
      updatePosition();
    };
    window.addEventListener('resize', onScroll);
    window.addEventListener('scroll', onScroll, true);
    return () => {
      window.removeEventListener('resize', onScroll);
      window.removeEventListener('scroll', onScroll, true);
    };
  }, [open, size, filtered.length, portalContainer]);

  useLayoutEffect(() => {
    if (!open) return;
    const list = listRef.current;
    const option = list?.querySelector<HTMLElement>('.ui-search-select__option.is-active');
    if (!list || !option) return;
    revealOption(list, option);
  }, [open, active, filtered.length]);

  useEffect(() => {
    const panel = panelRef.current;
    const list = listRef.current;
    if (!open || !panel || !list) return;
    // A modal dialog's scroll lock cancels wheel events on this body portal, so the dialog scroller moves instead.
    const onWheel = (event: WheelEvent) => {
      if (list.scrollHeight <= list.clientHeight) return;
      event.preventDefault();
      event.stopPropagation();
      list.scrollTop += wheelDeltaY(event, list);
    };
    panel.addEventListener('wheel', onWheel, { capture: true, passive: false });
    return () => panel.removeEventListener('wheel', onWheel, { capture: true });
  }, [open, filtered.length]);

  useEffect(() => {
    if (!open) return;
    setQuery('');
    setDebouncedQuery('');
    setActive(Math.max(0, options.findIndex((option) => isChosen(option.value))));
    if (!searchable) return;
    const t = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => window.clearTimeout(t);
  }, [open, searchable]);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      const target = e.target as Node;
      if (rootRef.current?.contains(target)) return;
      if (panelRef.current?.contains(target)) return;
      setOpen(false);
    };
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDoc);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDoc);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  function isChosen(value: string) {
    return multiple ? props.value.includes(value) : props.value === value;
  }

  function pick(next: string) {
    if (props.multiple) {
      const current = props.value;
      props.onChange(
        current.includes(next) ? current.filter((id) => id !== next) : [...current, next],
      );
      return;
    }
    props.onChange(next);
    setOpen(false);
    triggerRef.current?.focus();
  }

  function clearSelection() {
    if (props.multiple) props.onChange([]);
    else props.onChange(clearValue);
  }

  function onKeyDown(e: KeyboardEvent) {
    if (!open) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        setOpen(true);
      }
      return;
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      setOpen(false);
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActive((i) => Math.min(i + 1, Math.max(filtered.length - 1, 0)));
      return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActive((i) => Math.max(i - 1, 0));
      return;
    }
    if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      setActive(e.key === 'Home' ? 0 : Math.max(filtered.length - 1, 0));
      return;
    }
    if (e.key === 'Tab') {
      setOpen(false);
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      const opt = filtered[active];
      if (opt) pick(opt.value);
    }
  }

  return (
    <div
      ref={rootRef}
      className={cn(
        'ui-search-select relative max-w-full',
        size === 'sm' ? 'ui-search-select--sm w-auto min-w-[7.5rem]' : 'w-full',
        open && 'is-open',
        invalid && 'is-invalid',
        className,
      )}
      data-ui={dataUi}
      data-size={size}
      data-disabled={disabled ? 'true' : undefined}
      data-invalid={invalid ? 'true' : undefined}
    >
      <button
        ref={triggerRef}
        type="button"
        id={id}
        className={cn(
          'ui-search-select__trigger',
          'flex w-full cursor-pointer items-center justify-between gap-1.5 border border-transparent bg-[var(--color-control-fill)] text-left text-[13px] font-normal leading-none text-[var(--color-ink)] outline-none transition-colors',
          'hover:bg-[var(--color-control-fill-hover)] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10',
          'disabled:cursor-not-allowed disabled:opacity-40',
          size === 'sm' ? 'h-[30px] min-w-[7.5rem] rounded-[8px] ps-2.5 pe-2' : 'h-9 rounded-[8px] px-3',
          open && 'bg-[var(--color-control-fill-active)] ring-2 ring-inset ring-black/10',
          invalid && 'border-[color:var(--color-error)]',
        )}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-label={ariaLabel}
        aria-invalid={invalid || undefined}
        aria-describedby={ariaDescribedBy}
        disabled={disabled}
        onClick={() => !disabled && setOpen((v) => !v)}
        onKeyDown={onKeyDown}
      >
        <span className="ui-search-select__value inline-flex min-w-0 items-center gap-1.5 overflow-hidden">
          {!multiple && selectedOptions[0]?.icon ? (
            <span className="ui-search-select__icon grid size-4 shrink-0 place-items-center overflow-hidden" aria-hidden>
              {selectedOptions[0].icon}
            </span>
          ) : null}
          <span
            className={cn('truncate', isPlaceholder && 'is-placeholder text-[var(--color-ink-faint)]')}
            title={isPlaceholder ? undefined : summary}
          >
            {isPlaceholder ? placeholder : summary}
          </span>
        </span>
        <span className="inline-flex shrink-0 items-center gap-0.5">
          {allowClear && !isPlaceholder && (multiple || props.value !== clearValue) && !disabled ? (
            <span
              role="button"
              tabIndex={-1}
              className="inline-flex size-5 items-center justify-center rounded text-ink-faint hover:bg-surface hover:text-ink"
              aria-label="Clear"
              onMouseDown={(e) => {
                e.preventDefault();
                e.stopPropagation();
              }}
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                clearSelection();
              }}
            >
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
                <path
                  d="M3 3l6 6M9 3L3 9"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                />
              </svg>
            </span>
          ) : null}
          <svg
            className={cn(
              'ui-search-select__chevron shrink-0 text-[var(--color-ink-faint)] transition-transform',
              open && 'rotate-180',
            )}
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
              className={cn(
                'ui-search-select__panel ui-search-select__panel--portal',
                menuCardClass,
                'grid gap-1 p-1.5',
                coords && 'ui-search-select__panel--shown',
                size === 'sm' && 'ui-search-select__panel--sm',
              )}
              role="presentation"
              style={
                coords
                  ? {
                      position: portalContainer ? 'absolute' : 'fixed',
                      top: coords.top,
                      left: coords.left,
                      width: coords.width,
                      minWidth: coords.width,
                      maxWidth: Math.min(coords.width, window.innerWidth - 16),
                      zIndex: 240,
                      animation: instantMotion ? 'none' : undefined,
                    }
                  : { position: 'fixed', top: -9999, left: -9999, visibility: 'hidden', zIndex: 240 }
              }
            >
              {searchable ? (
                <input
                  ref={inputRef}
                  className={cn(
                    'ui-search-select__search h-8 w-full rounded-[8px] border border-transparent bg-[var(--color-control-fill)] text-[13px] leading-none text-[var(--color-ink)] outline-none placeholder:text-[var(--color-ink-faint)]',
                    size === 'sm' ? 'px-2.5' : 'px-3',
                  )}
                  value={query}
                  placeholder={searchPlaceholder}
                  role="combobox"
                  aria-autocomplete="list"
                  aria-expanded
                  aria-controls={listId}
                  aria-label={ariaLabel}
                  aria-activedescendant={filtered[active] ? `${listId}-${active}` : undefined}
                  onChange={(e) => {
                    setQuery(e.target.value);
                    setActive(0);
                  }}
                  onKeyDown={onKeyDown}
                />
              ) : null}
              <ul
                ref={listRef}
                id={listId}
                className={cn(
                  'ui-search-select__list m-0 min-h-0 list-none overflow-auto overscroll-contain p-0',
                  size === 'sm' ? 'max-h-40' : 'max-h-48',
                )}
                role="listbox"
                aria-multiselectable={multiple || undefined}
              >
                {filtered.length === 0 ? (
                  <li className="ui-search-select__empty px-2 py-2 text-[13px] text-[var(--color-ink-faint)]">
                    {emptyText}
                  </li>
                ) : (
                  filtered.map((opt, i) => {
                    const chosen = isChosen(opt.value);
                    return (
                    // The option owns the click: its children are presentational, so
                    // assistive tech activates the <li>, never the inner button.
                    <li
                      key={opt.value}
                      id={`${listId}-${i}`}
                      role="option"
                      aria-selected={chosen}
                      onClick={() => pick(opt.value)}
                    >
                      <button
                        type="button"
                        tabIndex={-1}
                        className={cn(
                          'ui-search-select__option',
                          'flex w-full cursor-pointer items-center border-0 bg-transparent text-left',
                          menuRowClass,
                          size === 'sm' ? 'min-h-8 gap-2 px-2' : 'min-h-9 gap-2.5 px-2.5',
                          chosen && 'is-selected',
                          i === active && 'is-active',
                          i === active && !chosen && 'bg-[var(--color-surface-2)] text-[var(--color-ink)]',
                          chosen && menuRowSelectedClass,
                        )}
                        onMouseEnter={() => setActive(i)}
                      >
                        {multiple ? (
                          <span
                            className={cn(
                              'ui-search-select__check inline-flex size-4 shrink-0 items-center justify-center rounded-[4px] border border-[color:var(--color-border)] bg-[var(--color-surface)] text-transparent',
                              chosen && 'border-[color:var(--color-ink)] bg-[var(--color-ink)] text-[var(--color-surface)]',
                            )}
                            aria-hidden
                          >
                            {chosen ? (
                              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                                <path
                                  d="M3.2 8.3 6.3 11.4 12.8 4.7"
                                  stroke="currentColor"
                                  strokeWidth="2"
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                />
                              </svg>
                            ) : null}
                          </span>
                        ) : opt.icon ? (
                          <span
                            className="ui-search-select__icon grid size-5 shrink-0 place-items-center overflow-hidden"
                            aria-hidden
                          >
                            {opt.icon}
                          </span>
                        ) : null}
                        <span className="ui-search-select__option-label min-w-0 truncate">
                          {opt.label}
                        </span>
                      </button>
                    </li>
                    );
                  })
                )}
              </ul>
            </div>,
            portalContainer ?? document.body,
          )
        : null}
    </div>
  );
}
