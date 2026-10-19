import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Input } from '../primitives/input';

export type ListToolbarProps = {
  search?: string;
  onSearchChange?: (value: string) => void;
  searchPlaceholder?: string;
  /** Extra classes on the search field. */
  searchClassName?: string;
  filters?: ReactNode;
  filtersClassName?: string;
  /** Keep search and filters on one row; the search field shrinks before the filters wrap. */
  keepFiltersInline?: boolean;
  /** Exactly one primary CTA preferred (PRINCIPLES UX §6) */
  primaryAction?: ReactNode;
  secondaryActions?: ReactNode;
  className?: string;
};

function SearchGlyph() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <circle cx="11" cy="11" r="6.5" />
      <path d="M16.5 16.5 21 21" />
    </svg>
  );
}

/** Single-row list chrome: search + filters | actions. */
export function ListToolbar({
  search,
  onSearchChange,
  searchPlaceholder = 'Search…',
  searchClassName = '',
  filters,
  filtersClassName = '',
  keepFiltersInline = false,
  primaryAction,
  secondaryActions,
  className = '',
}: ListToolbarProps) {
  const searchField = onSearchChange ? (
    <div
      className={cn(
        'relative w-[220px] shrink-0',
        keepFiltersInline && 'min-w-[8.75rem] w-auto max-w-[220px] flex-1 shrink',
        searchClassName,
      )}
    >
      <span className="pointer-events-none absolute inset-y-0 left-2.5 z-[1] inline-flex items-center text-ink-faint">
        <SearchGlyph />
      </span>
      <Input
        size="sm"
        value={search ?? ''}
        onChange={(e) => onSearchChange(e.target.value)}
        placeholder={searchPlaceholder}
        className="w-full !pl-8 !pr-7"
        aria-label={searchPlaceholder}
        allowClear
      />
    </div>
  ) : null;
  const filterField = filters ? (
    <div
      className={cn(
        'flex min-w-0 flex-wrap items-center gap-1.5',
        keepFiltersInline && 'shrink-0 flex-nowrap',
        filtersClassName,
      )}
    >
      {filters}
    </div>
  ) : null;

  return (
    <div
      className={cn(
        'flex flex-wrap items-center gap-x-2 gap-y-1.5',
        className,
      )}
    >
      {keepFiltersInline ? (
        <div className="flex min-w-[17.5rem] max-w-full flex-1 flex-nowrap items-center gap-2">
          {searchField}
          {filterField}
        </div>
      ) : (
        <>
          {searchField}
          {filterField}
        </>
      )}
      <div
        className="ml-auto flex shrink-0 items-center gap-1"
        data-a3s-list-tools=""
      />
      {secondaryActions || primaryAction ? (
        <div className="flex shrink-0 items-center gap-2">
          {secondaryActions}
          {primaryAction}
        </div>
      ) : null}
    </div>
  );
}
