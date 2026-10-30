import { cn } from '../../lib/utils'

export interface SparklineProps {
  values: number[]
  className?: string
}

export function buildSparklinePoints(values: number[], width = 48, height = 32): string {
  const recentValues = values.slice(-64)
  const maxValue = Math.max(...recentValues, 0)
  const minValue = Math.min(...recentValues, maxValue)
  const xStep = recentValues.length > 1 ? width / (recentValues.length - 1) : width

  return recentValues
    .map((value, index) => {
      const ratio = maxValue === minValue ? 0.5 : (value - minValue) / (maxValue - minValue)
      const x = index * xStep
      const y = height - ratio * (height - 4) - 2
      return `${x},${y}`
    })
    .join(' ')
}

/** Decorative trend only; the adjacent metric owns the exact accessible value. */
export function Sparkline({ values, className }: SparklineProps) {
  const width = 48
  const height = 32
  return (
    <div className={cn('flex h-8 w-12 shrink-0 items-center', className)} aria-hidden>
      <svg viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" className="h-8 w-full text-foreground">
        <polyline
          points={buildSparklinePoints(values, width, height)}
          fill="none"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="2"
        />
      </svg>
    </div>
  )
}
