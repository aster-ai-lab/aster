import CircularProgress from '../primitives/circular-progress'
import { cn } from '../../lib/cn'

export type RingProgressProps = {
  value: number // 0–100
  size?: number
  strokeWidth?: number
  label?: string
  className?: string
}

/**
 * Console meter API on shared Arc CircularProgress.
 * Prefer `CircularProgress` for new product surfaces.
 */
export function RingProgress({
  value,
  size = 72,
  strokeWidth = 6,
  label,
  className = '',
}: RingProgressProps) {
  const pct = Math.max(0, Math.min(100, value))
  return (
    <div
      className={cn('ui-ring', className)}
      style={{ width: size, height: size }}
      role="meter"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(pct)}
      aria-label={label}
    >
      <CircularProgress
        value={pct}
        size={size}
        strokeWidth={strokeWidth}
        showLabel
        renderLabel={() => `${Math.round(pct)}%`}
        className="block"
        labelClassName="ui-ring__label text-[13px] tabular-nums"
      />
    </div>
  )
}
