/**
 * Pure DataTable pipeline — filter → sort → paginate.
 * Presentation stays in DataTable.tsx (DESIGN.md `data-table`).
 */

export type SortDirection = 'asc' | 'desc';

export type ResourceTableSortState = {
  id: string;
  direction: SortDirection;
} | null;

export type ResourceTablePaginationState = {
  /** 1-based page index */
  page: number;
  pageSize: number;
};

export type ResourceTableColumnFilterState = Record<string, string>;

export type ResourceTableSortValue =
  | string
  | number
  | boolean
  | Date
  | null
  | undefined;

export type ResourceTableColumnModel<T> = {
  id: string;
  sortable?: boolean;
  getSortValue?: (row: T) => ResourceTableSortValue;
  sortFn?: (a: T, b: T) => number;
  getFilterValue?: (row: T) => string | number | boolean | null | undefined;
};

export function compareSortValues(
  a: ResourceTableSortValue,
  b: ResourceTableSortValue,
): number {
  if (a == null && b == null) return 0;
  if (a == null) return 1;
  if (b == null) return -1;

  if (a instanceof Date || b instanceof Date) {
    const av = a instanceof Date ? a.getTime() : Number(a);
    const bv = b instanceof Date ? b.getTime() : Number(b);
    return av === bv ? 0 : av < bv ? -1 : 1;
  }

  if (typeof a === 'number' && typeof b === 'number') {
    return a === b ? 0 : a < b ? -1 : 1;
  }

  if (typeof a === 'boolean' && typeof b === 'boolean') {
    return Number(a) - Number(b);
  }

  return String(a).localeCompare(String(b), undefined, {
    numeric: true,
    sensitivity: 'base',
  });
}

export function defaultRowFilterText<T>(
  row: T,
  columns: ResourceTableColumnModel<T>[],
): string {
  return columns
    .map((col) => {
      if (col.getFilterValue) return String(col.getFilterValue(row) ?? '');
      if (col.getSortValue) {
        const value = col.getSortValue(row);
        if (value instanceof Date) return String(value.getTime());
        return String(value ?? '');
      }
      return '';
    })
    .join(' ')
    .toLowerCase();
}

export function nextSortState(
  current: ResourceTableSortState,
  columnId: string,
): ResourceTableSortState {
  if (!current || current.id !== columnId) {
    return { id: columnId, direction: 'asc' };
  }
  if (current.direction === 'asc') {
    return { id: columnId, direction: 'desc' };
  }
  return null;
}

export function filterRows<T>(
  rows: T[],
  columns: ResourceTableColumnModel<T>[],
  options: {
    globalFilter: string;
    columnFilters: ResourceTableColumnFilterState;
    getRowFilterText?: (row: T) => string;
  },
): T[] {
  const global = options.globalFilter.trim().toLowerCase();
  const activeColumnFilters = Object.entries(options.columnFilters).filter(
    ([, value]) => value.trim().length > 0,
  );

  if (!global && activeColumnFilters.length === 0) return rows;

  return rows.filter((row) => {
    for (const [columnId, raw] of activeColumnFilters) {
      const col = columns.find((item) => item.id === columnId);
      if (!col) continue;
      const needle = raw.trim().toLowerCase();
      const haystack = String(
        col.getFilterValue?.(row) ?? col.getSortValue?.(row) ?? '',
      ).toLowerCase();
      if (!haystack.includes(needle)) return false;
    }

    if (!global) return true;
    const text =
      options.getRowFilterText?.(row) ?? defaultRowFilterText(row, columns);
    return text.toLowerCase().includes(global);
  });
}

export function sortRows<T>(
  rows: T[],
  columns: ResourceTableColumnModel<T>[],
  sort: ResourceTableSortState,
): T[] {
  if (!sort) return rows;
  const column = columns.find((item) => item.id === sort.id);
  if (!column?.sortable) return rows;

  const copy = [...rows];
  copy.sort((a, b) => {
    const result = column.sortFn
      ? column.sortFn(a, b)
      : compareSortValues(column.getSortValue?.(a), column.getSortValue?.(b));
    return sort.direction === 'asc' ? result : -result;
  });
  return copy;
}

export function paginateRows<T>(
  rows: T[],
  pagination: ResourceTablePaginationState,
): T[] {
  const start = (pagination.page - 1) * pagination.pageSize;
  return rows.slice(start, start + pagination.pageSize);
}

export function pageCountFor(total: number, pageSize: number): number {
  return Math.max(1, Math.ceil(total / Math.max(1, pageSize)) || 1);
}
