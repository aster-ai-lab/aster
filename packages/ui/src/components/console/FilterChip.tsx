import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { cn } from '../../lib/cn';

export type FilterChipProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string;
  active?: boolean;
  icon?: ReactNode;
  /** Optional trailing count — keeps toolbar dense without a separate stats row. */
  count?: number | string;
  /**
   * Fill a padded segmented group (`h-8` + `p-0.5`) without overflowing.
   * Standalone toolbar chips keep fixed `h-8`.
   */
  inset?: boolean;
};

/**
 * DESIGN.md §6. Inside a segmented track (`inset`), the selected chip is the white
 * 22px pill and the rest are `#6f6f6f`. Standalone chips use the list-row selected
 * (`#f2f3f5`, medium, `#171717`) and rest (`#3f3f3f`) colors at the 30px toolbar height.
 */
export function FilterChip({
  label,
  active = false,
  icon,
  count,
  inset = false,
  className = '',
  ...props
}: FilterChipProps) {
  return (
    <button
      type="button"
      className={cn(
        'inline-flex items-center gap-1 whitespace-nowrap text-[13px] outline-none transition-colors focus-visible:shadow-[inset_0_0_0_2px_rgba(0,0,0,0.1)]',
        inset
          ? cn(
              'h-full min-h-0 self-stretch rounded-[6px] px-3',
              active
                ? 'bg-bg-elevated font-medium text-ink shadow-[var(--shadow-segment)]'
                : 'bg-transparent text-[color:var(--color-segment-rest)] hover:text-ink',
            )
          : cn(
              'h-[30px] rounded-lg px-2.5',
              active
                ? 'bg-surface font-medium text-ink'
                : 'bg-transparent text-ink-muted hover:bg-surface-2 hover:text-ink',
            ),
        className,
      )}
      aria-pressed={active}
      {...props}
    >
      {icon ? (
        <span className="inline-flex shrink-0 opacity-90" aria-hidden>
          {icon}
        </span>
      ) : null}
      <span>{label}</span>
      {count != null ? (
        <span
          className={cn(
            'tabular-nums',
            active ? 'text-ink' : 'text-ink-faint',
          )}
        >
          {count}
        </span>
      ) : null}
    </button>
  );
}
