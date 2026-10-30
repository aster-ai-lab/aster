import { useEffect, useMemo, useRef, type ReactNode } from 'react'

import { cn } from '../../lib/utils'
import { NormalTooltip } from '../primitives/tooltip'
import { Skeleton } from '../primitives/skeleton'
import { getActivityHeatmapIntensity, type ActivityHeatmapDay } from './heatmap'
import './chart.css'

const CELL_SIZE = 12
const CELL_GAP = 3

const intensityClassNames: Record<0 | 1 | 2 | 3 | 4, string> = {
  0: 'bg-muted/70',
  1: 'bg-foreground/25',
  2: 'bg-foreground/45',
  3: 'bg-foreground/70',
  4: 'bg-neutral-900 dark:bg-neutral-100'
}

export interface ActivityHeatmapProps {
  days: ActivityHeatmapDay[]
  thresholds: [number, number, number]
  locale: string
  loading?: boolean
  className?: string
  getTooltip: (day: ActivityHeatmapDay) => ReactNode
  getAriaLabel: (day: ActivityHeatmapDay) => string
}

export function ActivityHeatmap({ days, thresholds, locale, loading = false, className, getTooltip, getAriaLabel }: ActivityHeatmapProps) {
  const heatmapRef = useRef<HTMLDivElement>(null)
  const animationRef = useRef<HTMLDivElement>(null)
  const weeks = useMemo(
    () => Array.from({ length: Math.ceil(days.length / 7) }, (_, index) => days.slice(index * 7, index * 7 + 7)),
    [days]
  )

  useEffect(() => {
    const element = heatmapRef.current
    if (!element) return
    element.scrollLeft = element.scrollWidth - element.clientWidth
  }, [days])

  useEffect(() => {
    if (loading) return
    const element = animationRef.current
    if (!element || typeof element.getAnimations !== 'function') return
    for (const animation of element.getAnimations({ subtree: true })) {
      animation.currentTime = 0
      animation.play()
    }
  }, [loading, days])

  const monthLabels = useMemo(() => {
    const formatter = new Intl.DateTimeFormat(locale, { month: 'short' })
    let previousVisibleIndex = -Infinity
    return weeks.map((week, weekIndex) => {
      const day = week[0]?.dateValue
      if (!day) return ''
      const previous = weekIndex > 0 ? weeks[weekIndex - 1][0]?.dateValue : undefined
      const label = !previous || previous.getMonth() !== day.getMonth() ? formatter.format(day) : ''
      if (!label || weekIndex - previousVisibleIndex < 3) return ''
      previousVisibleIndex = weekIndex
      return label
    })
  }, [locale, weeks])

  return (
    <div ref={heatmapRef} className={cn('mt-2 max-w-full min-w-0 overflow-x-auto', className)}>
      <div ref={animationRef} className="flex w-max min-w-full justify-end" style={{ gap: CELL_GAP }}>
        {weeks.map((week, weekIndex) => (
          <div
            key={week[0]?.date ?? weekIndex}
            className="grid shrink-0"
            style={{ width: CELL_SIZE, gap: CELL_GAP, gridTemplateRows: '16px auto' }}>
            <div className="h-4 overflow-visible pr-3 text-[10px] leading-4 whitespace-nowrap text-foreground-tertiary">
              {monthLabels[weekIndex]}
            </div>
            <div className="grid" style={{ gap: CELL_GAP, gridTemplateRows: `repeat(7, ${CELL_SIZE}px)` }}>
              {loading
                ? week.map((day) => <Skeleton key={day.date} className="rounded-[3px]" style={{ height: CELL_SIZE, width: CELL_SIZE }} />)
                : week.map((day, dayIndex) => {
                    const cellIndex = weekIndex * 7 + dayIndex
                    const cellStyle = {
                      height: CELL_SIZE,
                      width: CELL_SIZE,
                      animationDelay: `${Math.floor(cellIndex / 7) * 8 + (cellIndex % 7) * 12}ms`
                    }
                    if (day.isOutsideRange) {
                      return <div key={day.date} aria-hidden className="a3s-chart-heatmap-cell rounded-[3px] bg-muted/30" style={cellStyle} />
                    }
                    const intensity = getActivityHeatmapIntensity(day.value, thresholds)
                    return (
                      <NormalTooltip key={day.date} content={getTooltip(day)} side="top" sideOffset={4}>
                        <div
                          role="img"
                          aria-label={getAriaLabel(day)}
                          className={cn('a3s-chart-heatmap-cell cursor-help rounded-[3px]', intensityClassNames[intensity])}
                          style={cellStyle}
                        />
                      </NormalTooltip>
                    )
                  })}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
