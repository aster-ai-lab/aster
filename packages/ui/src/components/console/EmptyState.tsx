import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Button } from '../primitives/button';

export type EmptyStateVariant = 'no-data' | 'no-match';

export type EmptyStateProps = {
  title: string;
  description?: string;
  variant?: EmptyStateVariant;
  action?: ReactNode;
  onClearFilters?: () => void;
  /** Required when `onClearFilters` is set — never hardcode a locale here. */
  clearFiltersLabel?: string;
  className?: string;
  /** Optional custom illustration; defaults to a simple inbox mark. */
  icon?: ReactNode;
  /**
   * 放进已有容器时不要再套一张卡片。文件区这类画布用 plain，列表页保持默认卡片。
   */
  plain?: boolean;
};

function DefaultEmptyIcon() {
  return (
    <svg
      width="40"
      height="40"
      viewBox="0 0 40 40"
      fill="none"
      aria-hidden
      className="text-ink-faint"
    >
      <rect
        x="6"
        y="10"
        width="28"
        height="20"
        rx="3"
        stroke="currentColor"
        strokeWidth="1.5"
      />
      <path
        d="M6 14h28"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
      <path
        d="M14 22h12M16 26h8"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  );
}

export function EmptyState({
  title,
  description,
  variant = 'no-data',
  action,
  onClearFilters,
  clearFiltersLabel,
  className = '',
  icon,
  plain = false,
}: EmptyStateProps) {
  const clearFilters = variant === 'no-match' && onClearFilters && clearFiltersLabel ? (
    <Button variant="outline" size="sm" onClick={onClearFilters}>
      {clearFiltersLabel}
    </Button>
  ) : null;
  const body = (
    <>
      <div className={plain ? 'flex items-center justify-center' : 'flex size-12 items-center justify-center rounded-full bg-[color:var(--color-control-fill)]'}>
        {icon ?? <DefaultEmptyIcon />}
      </div>
      <div className="max-w-sm">
        <p className="text-sm font-medium text-ink">{title}</p>
        {description ? (
          <p className="mt-1 text-[13px] leading-5 text-ink-faint">{description}</p>
        ) : null}
      </div>
      {clearFilters || action ? (
        <div className="flex flex-wrap items-center justify-center gap-2">
          {clearFilters}
          {action}
        </div>
      ) : null}
    </>
  );
  const frame = cn(
    'flex flex-col items-center justify-center gap-3 px-6 text-center',
    plain ? 'bg-transparent py-2' : 'py-16',
    className,
  );
  if (plain) {
    return (
      <div className={frame} data-empty-variant={variant} role="status">
        {body}
      </div>
    );
  }
  return (
    <div className={frame} data-empty-variant={variant} role="status">
      {body}
    </div>
  );
}
