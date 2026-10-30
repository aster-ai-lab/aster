import { afterEach, describe, expect, it, vi } from 'vitest'

import { buildActivityHeatmapDays, getActivityHeatmapIntensity, getActivityHeatmapThresholds } from '../heatmap'

afterEach(() => {
  vi.useRealTimers()
})

describe('activity heatmap', () => {
  it('keeps a one-year grid while marking only the selected range active', () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date(2026, 6, 27, 12))
    const range = {
      from: new Date(2026, 5, 28).getTime(),
      to: new Date(2026, 6, 27, 23, 59, 59, 999)
    }
    const days = buildActivityHeatmapDays([], range, 0)

    expect(days).toHaveLength(53 * 7)
    expect(days.filter((day) => !day.isOutsideRange)).toHaveLength(30)
    expect(days[0].dateValue.getDay()).toBe(0)
    expect(days.at(-1)?.dateValue.getDay()).toBe(6)

    const mondayFirstDays = buildActivityHeatmapDays([], range, 1)
    expect(mondayFirstDays[0].dateValue.getDay()).toBe(1)
    expect(mondayFirstDays.at(-1)?.dateValue.getDay()).toBe(0)
  })

  it('derives thresholds from positive values and keeps null/zero at intensity zero', () => {
    const values = [null, 0, 10, 20, 30, 40]
    const thresholds = getActivityHeatmapThresholds(values)
    expect(thresholds).toEqual([10, 20, 30])
    expect(getActivityHeatmapIntensity(null, thresholds)).toBe(0)
    expect(getActivityHeatmapIntensity(0, thresholds)).toBe(0)
    expect(getActivityHeatmapIntensity(10, thresholds)).toBe(1)
    expect(getActivityHeatmapIntensity(20, thresholds)).toBe(2)
    expect(getActivityHeatmapIntensity(30, thresholds)).toBe(3)
    expect(getActivityHeatmapIntensity(40, thresholds)).toBe(4)
  })
})
