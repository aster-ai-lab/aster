import type { ButtonHTMLAttributes, ReactNode } from 'react'
import { cn } from '../../lib/cn'
import { SelectionHighlight } from '../primitives/selection-highlight'

export type FilterChipProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  label: string
  active?: boolean
  icon?: ReactNode
  /** Optional trailing count — keeps toolbar dense without a separate stats row. */
  count?: number | string
  /**
   * Fill a padded segmented group (`h-8` + `p-0.5`) without overflowing.
   * Standalone toolbar chips keep fixed `h-8`.
   */
  inset?: boolean
  /**
   * Shared selection plate id for an inset chip strip. Parent should wrap the
   * strip in `LayoutGroup` with the same id (Arc shared-highlight recipe).
   */
  highlightId?: string
}

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
  highlightId,
  className = '',
  ...props
}: FilterChipProps) {
  const plate = Boolean(highlightId && active)
  return (
    <button
      type="button"
      className={cn(
        'relative isolate inline-flex items-center gap-1 whitespace-nowrap text-[13px] outline-none motion-safe:transition-colors motion-safe:duration-150 focus-visible:shadow-[inset_0_0_0_2px_rgba(0,0,0,0.1)]',
        inset
          ? cn(
              'h-full min-h-0 self-stretch rounded-[6px] px-3',
              active
                ? cn('font-medium text-ink', !plate && 'bg-bg-elevated shadow-[var(--shadow-segment)]')
                : 'bg-transparent text-[color:var(--color-segment-rest)] hover:text-ink'
            )
          : cn(
              'h-[30px] rounded-lg px-2.5',
              active
                ? cn('font-medium text-ink', !plate && 'bg-surface')
                : 'bg-transparent text-ink-muted hover:bg-surface-2 hover:text-ink'
            ),
        className
      )}
      aria-pressed={active}
      {...props}>
      {plate ? (
        <SelectionHighlight
          layoutId={highlightId!}
          layoutDependency={label}
          className={inset ? 'bg-bg-elevated shadow-[var(--shadow-segment)]' : 'bg-surface'}
        />
      ) : null}
      {icon ? (
        <span className="relative z-10 inline-flex shrink-0 opacity-90" aria-hidden>
          {icon}
        </span>
      ) : null}
      <span className="relative z-10">{label}</span>
      {count != null ? (
        <span
          className={cn(
            'relative z-10 tabular-nums',
            active ? 'text-ink' : 'text-ink-faint'
          )}>
          {count}
        </span>
      ) : null}
    </button>
  )
}
