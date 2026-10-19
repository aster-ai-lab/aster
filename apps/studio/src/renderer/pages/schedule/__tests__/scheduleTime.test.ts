import { describe, expect, it } from 'vitest'

import { defaultSlot, moveSpan, placeOnDay, rangeLabel, startOfWeek, timeChoices, visibleRange } from '../scheduleTime'

describe('schedule time grid', () => {
  it('opens the week on Sunday, matching a 4日–10日 range when today is the 6th', () => {
    const tuesday = new Date(2026, 9, 6, 13, 0)
    expect(startOfWeek(tuesday).getDate()).toBe(4)
    const range = visibleRange('week', tuesday)
    expect(range.start.getDate()).toBe(4)
    expect(range.end.getDate()).toBe(11)
    expect(rangeLabel(range.start, range.end, 'zh-CN')).toBe('2026年10月4日 - 10日')
  })

  it('places a 13:00–15:00 block on that day and splits an overlap into two lanes', () => {
    const day = new Date(2026, 9, 6)
    const alone = placeOnDay(
      [{ id: 'a', start: new Date(2026, 9, 6, 13, 0), end: new Date(2026, 9, 6, 15, 0) }],
      day,
      48
    )
    expect(alone).toEqual([{ id: 'a', top: 13 * 48, height: 2 * 48, lane: 0, lanes: 1 }])

    const overlap = placeOnDay(
      [
        { id: 'a', start: new Date(2026, 9, 6, 13, 0), end: new Date(2026, 9, 6, 15, 0) },
        { id: 'b', start: new Date(2026, 9, 6, 14, 0), end: new Date(2026, 9, 6, 16, 0) }
      ],
      day,
      48
    )
    expect(overlap.map((item) => item.lanes)).toEqual([2, 2])
    expect(new Set(overlap.map((item) => item.lane)).size).toBe(2)
  })

  it('keeps a timed block off a day it does not touch', () => {
    const placed = placeOnDay(
      [{ id: 'a', start: new Date(2026, 9, 6, 13, 0), end: new Date(2026, 9, 6, 15, 0) }],
      new Date(2026, 9, 7)
    )
    expect(placed).toEqual([])
  })

  it('uses 09:00 on another day and the next hour on today', () => {
    const now = new Date(2026, 9, 6, 13, 20)
    const other = defaultSlot(new Date(2026, 9, 8), now)
    expect(other.start.getHours()).toBe(9)
    expect(other.end.getTime() - other.start.getTime()).toBe(60 * 60 * 1000)
    const today = defaultSlot(new Date(2026, 9, 6, 1), now)
    expect(today.start.getHours()).toBe(14)
  })

  it('keeps a one-hour span when the start moves onto the end, and keeps an earlier end', () => {
    const span = { startDate: '2026-10-06', startTime: '22:00', endDate: '2026-10-06', endTime: '23:00' }
    expect(moveSpan(span, { startTime: '23:00' }, 'start')).toEqual({
      startDate: '2026-10-06',
      startTime: '23:00',
      endDate: '2026-10-07',
      endTime: '00:00'
    })
    expect(moveSpan(span, { endTime: '21:00' }, 'end')).toEqual({ ...span, endTime: '21:00' })
    expect(timeChoices('22:10')).toContain('22:10')
    expect(timeChoices('22:00')).toHaveLength(96)
  })
})
