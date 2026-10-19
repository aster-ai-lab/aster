import { describe, expect, it } from 'vitest'

import {
  decodeRule,
  emailOk,
  encodeCustom,
  instantsFromDraft,
  nthOfMonth,
  spanFromEvent,
  weekdayIndex,
  zonedInstant
} from '../scheduleRule'

describe('schedule repeat grammar', () => {
  it('round-trips presets and a counted custom week', () => {
    for (const preset of ['none', 'daily', 'weekly', 'monthly_nth', 'monthly_day', 'yearly', 'weekdays'] as const) {
      expect(decodeRule(preset)?.preset).toBe(preset)
    }
    const custom = encodeCustom({
      unit: 'week',
      interval: 2,
      weekdays: [3, 1],
      monthMode: 'day',
      end: 'count',
      until: '2026-12-31',
      count: 4
    })
    expect(custom).toBe('custom:week:2:1,3|count:4')
    expect(decodeRule(custom)?.custom.weekdays).toEqual([1, 3])
    expect(decodeRule('custom:week:2:3,1')).toBeNull()
    expect(decodeRule('every day')).toBeNull()
    expect(decodeRule('none|count:2')).toBeNull()
  })

  it('names the first Wednesday of the month from the civil date', () => {
    expect(weekdayIndex('2026-10-07')).toBe(3)
    expect(nthOfMonth(7)).toBe(1)
    expect(nthOfMonth(8)).toBe(2)
  })

  it('keeps 09:00 in Shanghai on both sides of the instant', () => {
    const instant = zonedInstant('2026-10-07', '09:00', 'Asia/Shanghai')
    expect(instant?.toISOString()).toBe('2026-10-07T01:00:00.000Z')
    const span = instantsFromDraft({
      startDate: '2026-10-07',
      startTime: '09:00',
      endDate: '2026-10-07',
      endTime: '09:30',
      allDay: false,
      timeZone: 'Asia/Shanghai'
    })
    expect(span).toEqual({
      starts_at: '2026-10-07T01:00:00.000Z',
      ends_at: '2026-10-07T01:30:00.000Z',
      utc_offset_minutes: 480
    })
    expect(
      spanFromEvent({
        starts_at: '2026-10-07T01:00:00.000Z',
        ends_at: '2026-10-07T16:00:00.000Z',
        all_day: true,
        time_zone: 'Asia/Shanghai'
      })
    ).toEqual({ startDate: '2026-10-07', startTime: '00:00', endDate: '2026-10-07', endTime: '00:00' })
  })

  it('accepts one email and rejects a space or a missing dot', () => {
    expect(emailOk('a@b.co')).toBe(true)
    expect(emailOk('a@b')).toBe(false)
    expect(emailOk('a b@b.co')).toBe(false)
    expect(emailOk('not-an-email')).toBe(false)
  })
})
