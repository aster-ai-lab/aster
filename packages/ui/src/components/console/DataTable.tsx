import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  Fragment,
  isValidElement,
  type KeyboardEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../../lib/cn';
import { type RowMotion, rowMotionClassName, rowMotionStyle } from '../../lib/rowMotion';
import { Button } from '../primitives/button';
import {
  filterRows,
  nextSortState,
  pageCountFor,
  paginateRows,
  sortRows,
  type ResourceTableColumnFilterState,
  type ResourceTablePaginationState,
  type ResourceTableSortState,
  type ResourceTableSortValue,
  type SortDirection,
} from '../../lib/resourceTableModel';
import { Pagination } from './Pagination';
import { SearchSelect } from './SearchSelect';
import { Skeleton } from '../primitives/skeleton';
import { Input } from '../primitives/input';

/**
 * DataTable — Console List resource table.
 *
 * Visual tokens follow repo root `DESIGN.md` (`data-table*`):
 * - Surface card + hairline-strong outline · `{rounded.lg}` 12px
 * - Headers caption-uppercase muted; cells body-sm ink
 * - Selected / interactive wash = ink 8% — never text-link blue
 * - Sort glyph = 12px chevrons (not emoji); pager = secondary buttons
 */

export type {
  ResourceTableColumnFilterState,
  ResourceTablePaginationState,
  ResourceTableSortState,
  ResourceTableSortValue,
  SortDirection,
};

export type DataTableColumn<T> = {
  id: string;
  header: ReactNode;
  cell: (row: T) => ReactNode;
  className?: string;
  width?: string;
  /** Opt-in column sorting. Needs getSortValue or sortFn. */
  sortable?: boolean;
  getSortValue?: (row: T) => ResourceTableSortValue;
  sortFn?: (a: T, b: T) => number;
  /** Show a compact text filter under the header. */
  filterable?: boolean;
  getFilterValue?: (row: T) => string | number | boolean | null | undefined;
};

export type DataTableProps<T> = {
  columns: DataTableColumn<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  onRowClick?: (row: T) => void;
  /** Marks the active row with DESIGN.md `data-table-row` selected wash. */
  selectedRowKey?: string | null;
  rowActions?: (row: T) => ReactNode;
  /** Optimistic state of a row (see `RowMotion`); every row also rises in when it mounts. */
  rowMotion?: (row: T) => RowMotion | undefined;
  empty?: ReactNode;
  loading?: boolean;
  loadingRows?: number;
  className?: string;
  'aria-label'?: string;

  sort?: ResourceTableSortState;
  defaultSort?: ResourceTableSortState;
  onSortChange?: (sort: ResourceTableSortState) => void;
  manualSorting?: boolean;

  globalFilter?: string;
  defaultGlobalFilter?: string;
  onGlobalFilterChange?: (value: string) => void;
  showGlobalFilter?: boolean;
  globalFilterPlaceholder?: string;
  getRowFilterText?: (row: T) => string;
  columnFilters?: ResourceTableColumnFilterState;
  defaultColumnFilters?: ResourceTableColumnFilterState;
  onColumnFiltersChange?: (filters: ResourceTableColumnFilterState) => void;
  manualFiltering?: boolean;

  pagination?: ResourceTablePaginationState;
  defaultPagination?: ResourceTablePaginationState;
  onPaginationChange?: (pagination: ResourceTablePaginationState) => void;
  pageSizeOptions?: number[];
  totalRows?: number;
  manualPagination?: boolean;
  /**
   * Show the pagination footer. Defaults to `true`.
   * Pass `false` for tiny embedded tables that should not page.
   */
  showPagination?: boolean;
  /**
   * Reload the rows. When omitted, refresh clears column filters and
   * returns to the first page — it does not invent a network reload.
   */
  onRefresh?: () => void | Promise<void>;
  /** Content above the search toolbar, inside the table. */
  header?: ReactNode;
  /** Controls placed immediately to the right of the search field. */
  afterSearch?: ReactNode;
  /** Extra controls on the toolbar row, immediately left of column settings and refresh. */
  extraTools?: ReactNode;
  /** Localized footer copy. Defaults stay English for package isolation. */
  labels?: Partial<DataTableLabels>;
  /** When set with renderExpanded, clicking a row toggles the expansion panel. */
  expandedRowKey?: string | null;
  onExpandedRowChange?: (rowKey: string | null) => void;
  renderExpanded?: (row: T) => ReactNode;
};

export type DataTableLabels = {
  zeroResults: string;
  range: string;
  rows: string;
  rowsPerPage: string;
  columnFilters: string;
  refresh: string;
  filterColumn: string;
};

const DEFAULT_LABELS: DataTableLabels = {
  zeroResults: '0 results',
  range: '{start}–{end} of {total}',
  rows: 'Rows',
  rowsPerPage: 'Rows per page',
  columnFilters: 'Column filters',
  refresh: 'Refresh',
  filterColumn: 'Filter {column}',
};

function nodeText(node: ReactNode): string {
  if (node == null || typeof node === 'boolean') return '';
  if (typeof node === 'string' || typeof node === 'number') return String(node);
  if (Array.isArray(node)) return node.map(nodeText).join(' ');
  if (isValidElement(node)) {
    const props = node.props as { children?: ReactNode };
    return nodeText(props.children);
  }
  return '';
}

function columnLabel(header: ReactNode, id: string): string {
  return typeof header === 'string' && header.trim() ? header : id;
}

function ColumnsIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden>
      <path
        d="M4 5h16l-6.2 7.2a1 1 0 0 0-.25.66V19l-3.1 1.55V12.86A1 1 0 0 0 10.2 12.2L4 5Z"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinejoin="round"
      />
    </svg>
  );
}

const LIST_TOOLS_ATTR = 'data-a3s-list-tools';
/** One turn of ui-btn-spin, so a fast reload still shows the icon rotating. */
const REFRESH_SPIN_MS = 700;

function findToolbarSlot(from: HTMLElement | null): HTMLElement | null {
  let node = from;
  while (node && node !== document.body) {
    let prev = node.previousElementSibling;
    while (prev) {
      if (prev instanceof HTMLElement) {
        if (prev.hasAttribute(LIST_TOOLS_ATTR)) return prev;
        const nested = prev.querySelector(`[${LIST_TOOLS_ATTR}]`);
        if (nested instanceof HTMLElement) return nested;
      }
      prev = prev.previousElementSibling;
    }
    if (node.getAttribute('role') === 'dialog') break;
    node = node.parentElement;
  }
  return null;
}

function RefreshIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden>
      <path
        d="M20 12a8 8 0 0 1-13.7 5.6M4 12a8 8 0 0 1 13.7-5.6"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
      />
      <path
        d="M20 5v4.5h-4.5M4 19v-4.5h4.5"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function fillLabel(template: string, vars: Record<string, string | number>) {
  return Object.entries(vars).reduce(
    (text, [key, value]) => text.split(`{${key}}`).join(String(value)),
    template,
  );
}

const DEFAULT_PAGE_SIZE = 20;
const DEFAULT_PAGE_SIZE_OPTIONS = [10, 20, 50, 100];

function useControllableState<T>({
  value,
  defaultValue,
  onChange,
}: {
  value: T | undefined;
  defaultValue: T;
  onChange?: (next: T) => void;
}): [T, (next: T) => void] {
  const [internal, setInternal] = useState(defaultValue);
  const isControlled = value !== undefined;
  const current = isControlled ? value : internal;

  const setValue = useCallback(
    (next: T) => {
      if (!isControlled) setInternal(next);
      onChange?.(next);
    },
    [isControlled, onChange],
  );

  return [current, setValue];
}

function SortGlyph({ direction }: { direction: SortDirection | null }) {
  return (
    <svg
      className={cn(
        'ui-data-table__sort-glyph',
        direction === 'asc' && 'is-asc',
        direction === 'desc' && 'is-desc',
      )}
      width="12"
      height="12"
      viewBox="0 0 12 12"
      fill="none"
      aria-hidden
    >
      <path
        className="ui-data-table__sort-up"
        d="M3.5 4.75 6 2.25l2.5 2.5"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        className="ui-data-table__sort-down"
        d="M3.5 7.25 6 9.75l2.5-2.5"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function TableTools({
  columns,
  hiddenIds,
  onToggleColumn,
  labels,
  refreshing,
  onRefresh,
}: {
  columns: { id: string; header: ReactNode }[];
  hiddenIds: string[];
  onToggleColumn: (id: string, visible: boolean) => void;
  labels: DataTableLabels;
  refreshing: boolean;
  onRefresh: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<{ top: number; left: number } | null>(
    null,
  );
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuId = useId();
  const visibleCount = columns.filter((col) => !hiddenIds.includes(col.id)).length;

  function updatePosition() {
    const trigger = rootRef.current;
    const menu = menuRef.current;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const menuWidth = menu?.offsetWidth ?? 180;
    const menuHeight = menu?.offsetHeight ?? 0;
    const gap = 4;
    let left = rect.right - menuWidth;
    left = Math.max(8, Math.min(left, window.innerWidth - menuWidth - 8));
    let top = rect.bottom + gap;
    if (top + menuHeight > window.innerHeight - 8 && rect.top > menuHeight) {
      top = rect.top - menuHeight - gap;
    }
    setCoords({ top, left });
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
  }, [open, columns.length]);

  useEffect(() => {
    if (!open) return;
    const onDoc = (event: MouseEvent) => {
      const target = event.target as Node;
      if (rootRef.current?.contains(target)) return;
      if (menuRef.current?.contains(target)) return;
      setOpen(false);
    };
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    const timer = window.setTimeout(() => {
      document.addEventListener('mousedown', onDoc);
    }, 0);
    document.addEventListener('keydown', onKey);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener('mousedown', onDoc);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  return (
    <div className="ui-data-table__chrome-actions">
      <div ref={rootRef} className="relative inline-flex">
        <Button
          type="button"
          size="sm"
          variant="ghost"
          className="ui-data-table__tool"
          aria-label={labels.columnFilters}
          title={labels.columnFilters}
          aria-haspopup="dialog"
          aria-expanded={open}
          aria-controls={menuId}
          onClick={() => setOpen((value) => !value)}
        >
          <ColumnsIcon />
        </Button>
        {open && typeof document !== 'undefined'
          ? createPortal(
              <div
                ref={menuRef}
                id={menuId}
                role="dialog"
                aria-label={labels.columnFilters}
                style={
                  coords
                    ? {
                        position: 'fixed',
                        top: coords.top,
                        left: coords.left,
                        zIndex: 90,
                      }
                    : { position: 'fixed', top: -9999, left: -9999, zIndex: 90 }
                }
                className="ui-data-table__columns"
              >
                {columns.map((col) => {
                  const shown = !hiddenIds.includes(col.id);
                  const label = columnLabel(col.header, col.id);
                  return (
                    <label key={col.id} className="ui-data-table__column-option">
                      <input
                        type="checkbox"
                        className="ui-data-table__column-check"
                        checked={shown}
                        disabled={shown && visibleCount <= 1}
                        aria-label={label}
                        onChange={(event) => onToggleColumn(col.id, event.target.checked)}
                      />
                      <span>{label}</span>
                    </label>
                  );
                })}
              </div>,
              document.body,
            )
          : null}
      </div>
      <Button
        type="button"
        size="sm"
        variant="ghost"
        className={cn('ui-data-table__tool', refreshing && 'is-refreshing')}
        aria-label={labels.refresh}
        title={labels.refresh}
        aria-busy={refreshing || undefined}
        onClick={(event) => {
          event.preventDefault();
          event.stopPropagation();
          onRefresh();
        }}
      >
        <RefreshIcon />
      </Button>
    </div>
  );
}

export function DataTable<T>({
  columns,
  rows,
  rowKey,
  onRowClick,
  selectedRowKey = null,
  rowActions,
  rowMotion,
  empty,
  loading = false,
  loadingRows = 5,
  className = '',
  'aria-label': ariaLabel = 'Data table',
  sort: sortProp,
  defaultSort = null,
  onSortChange,
  manualSorting = false,
  globalFilter: globalFilterProp,
  defaultGlobalFilter = '',
  onGlobalFilterChange,
  showGlobalFilter = false,
  globalFilterPlaceholder = 'Filter…',
  getRowFilterText,
  columnFilters: columnFiltersProp,
  defaultColumnFilters = {},
  onColumnFiltersChange,
  manualFiltering = false,
  pagination: paginationProp,
  defaultPagination,
  onPaginationChange,
  pageSizeOptions = DEFAULT_PAGE_SIZE_OPTIONS,
  totalRows,
  manualPagination = false,
  showPagination,
  onRefresh,
  header,
  afterSearch,
  extraTools,
  labels: labelsProp,
  expandedRowKey,
  onExpandedRowChange,
  renderExpanded,
}: DataTableProps<T>) {
  const labels = { ...DEFAULT_LABELS, ...labelsProp };
  // Pagination is on by default for list UIs; pass `showPagination={false}` to opt out
  // (e.g. tiny embedded tables that should never page).
  const paginationEnabled = showPagination !== false;

  const [sort, setSort] = useControllableState<ResourceTableSortState>({
    value: sortProp,
    defaultValue: defaultSort,
    onChange: onSortChange,
  });

  const [globalFilter, setGlobalFilter] = useControllableState<string>({
    value: globalFilterProp,
    defaultValue: defaultGlobalFilter,
    onChange: onGlobalFilterChange,
  });

  const [columnFilters, setColumnFilters] =
    useControllableState<ResourceTableColumnFilterState>({
      value: columnFiltersProp,
      defaultValue: defaultColumnFilters,
      onChange: onColumnFiltersChange,
    });

  const [pagination, setPagination] =
    useControllableState<ResourceTablePaginationState>({
      value: paginationProp,
      defaultValue: defaultPagination ?? {
        page: 1,
        pageSize: DEFAULT_PAGE_SIZE,
      },
      onChange: onPaginationChange,
    });

  const rootRef = useRef<HTMLDivElement>(null);
  const refreshLock = useRef(false);
  const ownerId = useId();
  const [focusKey, setFocusKey] = useState<string | null>(selectedRowKey);
  const [hiddenIds, setHiddenIds] = useState<string[]>([]);
  const [refreshing, setRefreshing] = useState(false);
  const [toolsSlot, setToolsSlot] = useState<HTMLElement | null>(null);

  useEffect(() => {
    if (selectedRowKey) setFocusKey(selectedRowKey);
  }, [selectedRowKey]);

  const filteredRows = useMemo(() => {
    if (manualFiltering) return rows;
    const model = columns.map((col) => ({
      id: col.id,
      sortable: col.sortable,
      getSortValue: col.getSortValue,
      sortFn: col.sortFn,
      getFilterValue: (row: T) => {
        if (col.getFilterValue) return col.getFilterValue(row);
        const text = nodeText(col.cell(row)).trim();
        if (text) return text;
        const sorted = col.getSortValue?.(row);
        return sorted instanceof Date ? sorted.toISOString() : sorted;
      },
    }));
    return filterRows(rows, model, {
      globalFilter,
      columnFilters,
      getRowFilterText,
    });
  }, [
    rows,
    columns,
    globalFilter,
    columnFilters,
    manualFiltering,
    getRowFilterText,
  ]);

  const sortedRows = useMemo(() => {
    if (manualSorting) return filteredRows;
    return sortRows(filteredRows, columns, sort);
  }, [filteredRows, columns, sort, manualSorting]);

  const filteredCount = manualPagination
    ? (totalRows ?? rows.length)
    : sortedRows.length;

  const pageCount = pageCountFor(filteredCount, pagination.pageSize);

  useEffect(() => {
    if (!paginationEnabled) return;
    if (pagination.page > pageCount) {
      setPagination({ page: pageCount, pageSize: pagination.pageSize });
    }
  }, [
    pagination.page,
    pagination.pageSize,
    pageCount,
    paginationEnabled,
    setPagination,
  ]);

  const visibleRows = useMemo(() => {
    if (!paginationEnabled || manualPagination) return sortedRows;
    return paginateRows(sortedRows, pagination);
  }, [sortedRows, pagination, paginationEnabled, manualPagination]);

  const shownColumns = useMemo(
    () => columns.filter((col) => !hiddenIds.includes(col.id)),
    [columns, hiddenIds],
  );

  useLayoutEffect(() => {
    const slot = findToolbarSlot(rootRef.current);
    if (!slot) {
      setToolsSlot(null);
      return;
    }
    const owner = slot.dataset['owner'];
    if (owner && owner !== ownerId) {
      setToolsSlot(null);
      return;
    }
    slot.dataset['owner'] = ownerId;
    setToolsSlot(slot);
    return () => {
      if (slot.dataset['owner'] === ownerId) delete slot.dataset['owner'];
    };
  }, [ownerId]);

  function toggleColumn(id: string, visible: boolean) {
    setHiddenIds((current) => {
      const hidden = new Set(current);
      if (visible) {
        hidden.delete(id);
      } else {
        const stillShown = columns.filter((col) => !hidden.has(col.id));
        if (stillShown.length <= 1) return current;
        hidden.add(id);
        if (sort?.id === id) setSort(null);
      }
      return [...hidden];
    });
  }

  async function refreshTable() {
    if (refreshLock.current) return;
    refreshLock.current = true;
    setRefreshing(true);
    const started = performance.now();
    // Yield so the spin class paints before a synchronous reload finishes.
    await new Promise<void>((resolve) => {
      window.setTimeout(resolve, 0);
    });
    setGlobalFilter('');
    setColumnFilters({});
    if (paginationEnabled) {
      setPagination({ page: 1, pageSize: pagination.pageSize });
    }
    try {
      await onRefresh?.();
    } finally {
      const remain = REFRESH_SPIN_MS - (performance.now() - started);
      if (remain > 0) {
        await new Promise<void>((resolve) => {
          window.setTimeout(resolve, remain);
        });
      }
      refreshLock.current = false;
      setRefreshing(false);
    }
  }

  const tools = (
    <>
      {extraTools}
      <TableTools
        columns={columns}
        hiddenIds={hiddenIds}
        onToggleColumn={toggleColumn}
        labels={labels}
        refreshing={refreshing}
        onRefresh={() => void refreshTable()}
      />
    </>
  );

  const chrome =
    showGlobalFilter || !toolsSlot ? (
      <div className="ui-data-table__chrome">
        {showGlobalFilter ? (
          <div className="ui-data-table__global-filter">
            <Input
              size="sm"
              value={globalFilter}
              onChange={(event) => onGlobalFilterInput(event.target.value)}
              placeholder={globalFilterPlaceholder}
              aria-label={globalFilterPlaceholder}
            />
          </div>
        ) : null}
        {afterSearch}
        {toolsSlot ? null : <div className="ui-data-table__chrome-end">{tools}</div>}
      </div>
    ) : null;

  const rangeStart =
    filteredCount === 0 ? 0 : (pagination.page - 1) * pagination.pageSize + 1;
  const rangeEnd = Math.min(
    filteredCount,
    pagination.page * pagination.pageSize,
  );

  const onHeaderSort = (column: DataTableColumn<T>) => {
    if (!column.sortable) return;
    setSort(nextSortState(sort, column.id));
  };

  const onColumnFilterInput = (columnId: string, value: string) => {
    const next = { ...columnFilters };
    if (!value.trim()) delete next[columnId];
    else next[columnId] = value;
    setColumnFilters(next);
    if (paginationEnabled) {
      setPagination({ page: 1, pageSize: pagination.pageSize });
    }
  };

  const onGlobalFilterInput = (value: string) => {
    setGlobalFilter(value);
    if (paginationEnabled) {
      setPagination({ page: 1, pageSize: pagination.pageSize });
    }
  };

  const moveFocus = (delta: number) => {
    if (!onRowClick || visibleRows.length === 0) return;
    const keys = visibleRows.map((row) => rowKey(row));
    const start = focusKey ? keys.indexOf(focusKey) : -1;
    const next =
      start < 0
        ? delta > 0
          ? 0
          : keys.length - 1
        : (start + delta + keys.length) % keys.length;
    setFocusKey(keys[next]);
  };

  const onTableKeyDown = (event: KeyboardEvent<HTMLTableElement>) => {
    if (!onRowClick) return;
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        moveFocus(1);
        break;
      case 'ArrowUp':
        event.preventDefault();
        moveFocus(-1);
        break;
      case 'Enter':
      case ' ': {
        event.preventDefault();
        const row = visibleRows.find((item) => rowKey(item) === focusKey);
        if (row) onRowClick(row);
        break;
      }
      default:
        break;
    }
  };

  if (!loading && filteredCount === 0 && empty) {
    return (
      <div ref={rootRef} className={cn('ui-data-table-root', className)}>
        {header}
        {toolsSlot ? createPortal(tools, toolsSlot) : null}
        {chrome}
        {empty}
      </div>
    );
  }

  return (
    <div ref={rootRef} className={cn('ui-data-table-root', className)}>
      {header}
      {toolsSlot ? createPortal(tools, toolsSlot) : null}
      {chrome}

      <div className="ui-card ui-data-table">
        <table
          className="ui-data-table__table"
          aria-label={ariaLabel}
          aria-rowcount={filteredCount}
          onKeyDown={onTableKeyDown}
        >
          <thead>
            <tr>
              {shownColumns.map((col) => {
                const ariaSort =
                  col.sortable && sort?.id === col.id
                    ? sort.direction === 'asc'
                      ? 'ascending'
                      : 'descending'
                    : col.sortable
                      ? 'none'
                      : undefined;

                return (
                  <th
                    key={col.id}
                    className={cn('ui-data-table__th', col.className)}
                    style={col.width ? { width: col.width } : undefined}
                    aria-sort={ariaSort}
                  >
                    {col.sortable ? (
                      <button
                        type="button"
                        className="ui-data-table__sort"
                        onClick={() => onHeaderSort(col)}
                      >
                        <span>{col.header}</span>
                        <SortGlyph
                          direction={
                            sort?.id === col.id ? sort.direction : null
                          }
                        />
                      </button>
                    ) : (
                      col.header
                    )}
                  </th>
                );
              })}
              {rowActions ? (
                <th className="ui-data-table__th ui-data-table__th--actions" />
              ) : null}
            </tr>
            {shownColumns.some((col) => col.filterable) ? (
              <tr className="ui-data-table__filter-row">
                {shownColumns.map((col) => (
                  <th key={`${col.id}-filter`} className="ui-data-table__th-filter">
                    {col.filterable ? (
                      <Input
                        size="sm"
                        value={columnFilters[col.id] ?? ''}
                        onChange={(event) =>
                          onColumnFilterInput(col.id, event.target.value)
                        }
                        placeholder={columnLabel(col.header, col.id)}
                        aria-label={fillLabel(labels.filterColumn, {
                          column: columnLabel(col.header, col.id),
                        })}
                        className="ui-data-table__col-filter"
                      />
                    ) : null}
                  </th>
                ))}
                {rowActions ? <th className="ui-data-table__th-filter" /> : null}
              </tr>
            ) : null}
          </thead>
          <tbody>
            {loading
              ? Array.from({ length: loadingRows }).map((_, i) => (
                  <tr key={`sk-${i}`} className="ui-data-table__row">
                    {shownColumns.map((col) => (
                      <td key={col.id} className="ui-data-table__td">
                        <Skeleton className="h-4 w-[75%]" />
                      </td>
                    ))}
                    {rowActions ? (
                      <td className="ui-data-table__td ui-data-table__td--actions">
                        <Skeleton className="ml-auto h-4 w-12" />
                      </td>
                    ) : null}
                  </tr>
                ))
              : visibleRows.length === 0 && empty
                ? (
                    <tr className="ui-data-table__row">
                      <td
                        className="ui-data-table__td"
                        colSpan={shownColumns.length + (rowActions ? 1 : 0)}
                      >
                        {empty}
                      </td>
                    </tr>
                  )
              : visibleRows.map((row, index) => {
                  const key = rowKey(row);
                  const motion = rowMotion?.(row);
                  const selected = selectedRowKey === key;
                  const focused = focusKey === key;
                  const expanded = expandedRowKey === key;
                  const interactive = Boolean(onRowClick || renderExpanded);
                  return (
                    <Fragment key={key}>
                    <tr
                      data-row-key={key}
                      data-motion={motion}
                      aria-busy={motion === 'pending' || undefined}
                      style={rowMotionStyle(index, motion)}
                      tabIndex={interactive ? (focused ? 0 : -1) : undefined}
                      className={cn(
                        'ui-data-table__row group',
                        interactive && 'is-interactive',
                        selected && 'is-selected',
                        focused && 'is-focused',
                        expanded && 'is-expanded',
                        rowMotionClassName(motion),
                      )}
                      onClick={
                        interactive
                          ? () => {
                              setFocusKey(key);
                              if (renderExpanded && onExpandedRowChange) {
                                onExpandedRowChange(expanded ? null : key);
                              }
                              onRowClick?.(row);
                            }
                          : undefined
                      }
                      onFocus={
                        interactive
                          ? () => {
                              setFocusKey(key);
                            }
                          : undefined
                      }
                    >
                      {shownColumns.map((col) => (
                        <td
                          key={col.id}
                          className={cn('ui-data-table__td', col.className)}
                        >
                          {col.cell(row)}
                        </td>
                      ))}
                      {rowActions ? (
                        <td
                          className="ui-data-table__td ui-data-table__td--actions"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <div className="ui-data-table__actions">
                            {rowActions(row)}
                          </div>
                        </td>
                      ) : null}
                    </tr>
                    {expanded && renderExpanded ? (
                      <tr className="ui-data-table__row ui-data-table__row--expanded">
                        <td
                          className="ui-data-table__td ui-data-table__td--expanded"
                          colSpan={shownColumns.length + (rowActions ? 1 : 0)}
                        >
                          {renderExpanded(row)}
                        </td>
                      </tr>
                    ) : null}
                    </Fragment>
                  );
                })}
          </tbody>
        </table>
      </div>

      {paginationEnabled ? (
        <div className="ui-data-table__footer">
          <div className="ui-data-table__meta">
            <span>
              {filteredCount === 0
                ? labels.zeroResults
                : fillLabel(labels.range, {
                    start: rangeStart,
                    end: rangeEnd,
                    total: filteredCount,
                  })}
            </span>
            <div className="ui-data-table__page-size">
              <span>{labels.rows}</span>
              <SearchSelect
                size="sm"
                searchable={false}
                allowClear={false}
                aria-label={labels.rowsPerPage}
                className="ui-data-table__page-size-select"
                value={String(pagination.pageSize)}
                options={pageSizeOptions.map((size) => ({
                  value: String(size),
                  label: String(size),
                }))}
                onChange={(next) => {
                  const pageSize = Number(next) || DEFAULT_PAGE_SIZE;
                  setPagination({ page: 1, pageSize });
                }}
              />
            </div>
          </div>
          <Pagination
            page={pagination.page}
            pageSize={pagination.pageSize}
            total={filteredCount}
            showSummary={false}
            onPageChange={(page) =>
              setPagination({ page, pageSize: pagination.pageSize })
            }
          />
        </div>
      ) : null}
    </div>
  );
}
