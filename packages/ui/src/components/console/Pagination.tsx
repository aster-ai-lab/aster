import { cn } from '../../lib/cn';

/**
 * Pagination — DESIGN.md `pagination*` / `cursor-pager`.
 *
 * Industry pattern (GitHub / Linear / Ant): icon steps + numbered window + ellipsis.
 * Secondary geometry only — never primary black CTA.
 */

export type PaginationProps = {
  page: number;
  pageSize: number;
  total?: number;
  hasNext?: boolean;
  hasPrev?: boolean;
  onPageChange: (page: number) => void;
  /** Pages on each side of the current page. Default 1. */
  siblingCount?: number;
  /** Show "a–b of n" / "Page n" summary. Default true. */
  showSummary?: boolean;
  className?: string;
  /** Disable navigation while a remote page is being loaded. */
  busy?: boolean;
  labels?: Partial<PaginationLabels>;
};

export type PaginationLabels = {
  navigation: string;
  pages: string;
  previous: string;
  next: string;
  page: (page: number) => string;
  range: (start: number, end: number, total: number) => string;
};

const defaultLabels: PaginationLabels = {
  navigation: 'Pagination',
  pages: 'Pages',
  previous: 'Previous page',
  next: 'Next page',
  page: (page) => `Page ${page}`,
  range: (start, end, total) =>
    total === 0 ? '0 results' : `${start}–${end} of ${total}`,
};

export type PageItem = number | 'ellipsis';

/** Build a compact page list: 1 … 4 5 6 … 20 */
export function getPaginationItems(
  page: number,
  pageCount: number,
  siblingCount = 1,
): PageItem[] {
  if (pageCount <= 0) return [];
  if (pageCount <= 7) {
    return Array.from({ length: pageCount }, (_, i) => i + 1);
  }

  const current = Math.min(Math.max(1, page), pageCount);
  const siblings = Math.max(0, siblingCount);
  const left = Math.max(2, current - siblings);
  const right = Math.min(pageCount - 1, current + siblings);

  const items: PageItem[] = [1];
  if (left > 2) items.push('ellipsis');
  for (let i = left; i <= right; i += 1) items.push(i);
  if (right < pageCount - 1) items.push('ellipsis');
  items.push(pageCount);
  return items;
}

function ChevronLeft() {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
      <path
        d="M7.5 2.5 4 6l3.5 3.5"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function ChevronRight() {
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
      <path
        d="M4.5 2.5 8 6l-3.5 3.5"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

const navigationClassName =
  'flex flex-wrap items-center justify-end gap-3 text-[13px] leading-5 text-[#3f3f3f] dark:text-[#d4d4d4]';
const controlsClassName =
  'inline-flex shrink-0 items-center gap-0 whitespace-nowrap sm:gap-1';
const pageButtonClassName =
  'inline-flex h-[30px] min-w-[30px] shrink-0 items-center justify-center rounded-[8px] px-2 text-[13px] tabular-nums outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-[#171717] focus-visible:ring-offset-2 dark:focus-visible:ring-[#d4d4d4] disabled:pointer-events-none disabled:opacity-40';

export function Pagination({
  page,
  pageSize,
  total,
  hasNext,
  hasPrev,
  onPageChange,
  siblingCount = 1,
  showSummary = true,
  className = '',
  busy = false,
  labels: labelOverrides,
}: PaginationProps) {
  const labels = { ...defaultLabels, ...labelOverrides };
  const pageCount =
    typeof total === 'number'
      ? Math.max(1, Math.ceil(total / Math.max(1, pageSize)) || 1)
      : null;

  const current =
    pageCount == null
      ? Math.max(1, page)
      : Math.min(Math.max(1, page), pageCount);
  const prevEnabled = !busy && current > 1 && (hasPrev ?? true);
  const nextEnabled =
    !busy &&
    (pageCount == null
      ? (hasNext ?? false)
      : current < pageCount && (hasNext ?? true));

  const items =
    pageCount != null
      ? getPaginationItems(current, pageCount, siblingCount)
      : [];

  return (
    <nav
      className={cn('ui-pagination', navigationClassName, className)}
      aria-label={labels.navigation}
      aria-busy={busy || undefined}
    >
      {showSummary ? (
        <p
          className="ui-pagination__summary m-0 whitespace-nowrap text-[12px] tabular-nums"
          role="status"
          aria-live="polite"
          aria-atomic="true"
        >
          {typeof total === 'number'
            ? labels.range(
                total === 0 ? 0 : (current - 1) * pageSize + 1,
                Math.min(total, current * pageSize),
                total,
              )
            : labels.page(current)}
        </p>
      ) : null}

      {pageCount === 1 ? null : (
        <div
          className={cn('ui-pagination__controls', controlsClassName)}
          role="group"
          aria-label={labels.pages}
        >
          <button
            type="button"
            className={cn('ui-pagination__step', pageButtonClassName)}
            aria-label={labels.previous}
            disabled={!prevEnabled}
            onClick={() => onPageChange(current - 1)}
          >
            <ChevronLeft />
          </button>

          {items.map((item, index) =>
            item === 'ellipsis' ? (
              <span
                key={`e-${index}`}
                className="ui-pagination__ellipsis inline-flex h-[30px] min-w-5 items-center justify-center"
                aria-hidden
              >
                …
              </span>
            ) : (
              <button
                key={item}
                type="button"
                className={cn(
                  'ui-pagination__page',
                  pageButtonClassName,
                  item === current && 'is-active',
                  item === current && 'bg-muted font-medium text-foreground',
                )}
                aria-label={labels.page(item)}
                aria-current={item === current ? 'page' : undefined}
                disabled={busy}
                onClick={() => {
                  if (item !== current) onPageChange(item);
                }}
              >
                {item}
              </button>
            ),
          )}

          <button
            type="button"
            className={cn('ui-pagination__step', pageButtonClassName)}
            aria-label={labels.next}
            disabled={!nextEnabled}
            onClick={() => onPageChange(current + 1)}
          >
            <ChevronRight />
          </button>
        </div>
      )}
    </nav>
  );
}

export type CursorPagerProps = {
  hasNext: boolean;
  hasPrev: boolean;
  onNext: () => void;
  onPrev: () => void;
  label?: string;
  className?: string;
  busy?: boolean;
  labels?: Partial<
    Pick<PaginationLabels, 'navigation' | 'pages' | 'previous' | 'next'>
  >;
};

export function CursorPager({
  hasNext,
  hasPrev,
  onNext,
  onPrev,
  label,
  className = '',
  busy = false,
  labels: labelOverrides,
}: CursorPagerProps) {
  const labels = { ...defaultLabels, ...labelOverrides };
  return (
    <nav
      className={cn(
        'ui-pagination ui-pagination--cursor',
        navigationClassName,
        'w-full justify-between',
        className,
      )}
      aria-label={labels.navigation}
      aria-busy={busy || undefined}
    >
      {label ? (
        <p
          className="ui-pagination__summary m-0 text-[12px] tabular-nums"
          role="status"
          aria-live="polite"
          aria-atomic="true"
        >
          {label}
        </p>
      ) : null}
      <div
        className={cn('ui-pagination__controls', controlsClassName)}
        role="group"
        aria-label={labels.pages}
      >
        <button
          type="button"
          className={cn('ui-pagination__step', pageButtonClassName)}
          aria-label={labels.previous}
          disabled={busy || !hasPrev}
          onClick={onPrev}
        >
          <ChevronLeft />
        </button>
        <button
          type="button"
          className={cn('ui-pagination__step', pageButtonClassName)}
          aria-label={labels.next}
          disabled={busy || !hasNext}
          onClick={onNext}
        >
          <ChevronRight />
        </button>
      </div>
    </nav>
  );
}
