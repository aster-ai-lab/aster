/** Repeat grammar shared with the schedule service. Presets have no suffix. */

export const REPEAT_PRESETS = ['none', 'daily', 'weekly', 'monthly_nth', 'monthly_day', 'yearly', 'weekdays'] as const

export type RepeatPreset = (typeof REPEAT_PRESETS)[number]

export type CustomRepeat = {
  unit: 'day' | 'week' | 'month' | 'year'
  interval: number
  /** 0 = Sunday … 6 = Saturday. Stored sorted and unique. */
  weekdays: number[]
  monthMode: 'day' | 'nth'
  end: 'never' | 'until' | 'count'
  until: string
  count: number
}

export const REMINDER_PRESETS = [0, 5, 15, 30, 60, 120, 1440, 2880] as const

export const TIME_ZONES = [
  'Asia/Shanghai',
  'Asia/Tokyo',
  'Asia/Singapore',
  'Europe/London',
  'America/New_York',
  'America/Los_Angeles',
  'UTC'
] as const

const DATE = /^(\d{4})-(\d{2})-(\d{2})$/
const TIME = /^(\d{2}):(\d{2})$/

export function localTimeZone(): string {
  const zone = Intl.DateTimeFormat().resolvedOptions().timeZone
  return zone && zone.length <= 64 ? zone : 'UTC'
}

export function weekdayIndex(date: string): number {
  const parts = dateParts(date)
  if (!parts) return 0
  return new Date(Date.UTC(parts.year, parts.month - 1, parts.day)).getUTCDay()
}

export function monthDay(date: string): number {
  return dateParts(date)?.day ?? 1
}

export function monthNumber(date: string): number {
  return dateParts(date)?.month ?? 1
}

/** 1-based "Nth weekday" of the month. Day 7 is the first, day 8 is the second. */
export function nthOfMonth(day: number): number {
  if (day < 1) return 1
  return Math.floor((day - 1) / 7) + 1
}

export function shiftDate(date: string, days: number): string {
  const parts = dateParts(date)
  if (!parts) return date
  const next = new Date(Date.UTC(parts.year, parts.month - 1, parts.day + days))
  const month = String(next.getUTCMonth() + 1).padStart(2, '0')
  const day = String(next.getUTCDate()).padStart(2, '0')
  return `${next.getUTCFullYear()}-${month}-${day}`
}

export function defaultCustom(date: string): CustomRepeat {
  const weekday = weekdayIndex(date)
  return {
    unit: 'week',
    interval: 1,
    weekdays: [weekday],
    monthMode: 'nth',
    end: 'never',
    until: shiftDate(date || '2026-01-01', 30),
    count: 10
  }
}

export function encodeCustom(custom: CustomRepeat): string {
  const interval = clamp(Math.trunc(custom.interval), 1, 30)
  let body: string
  if (custom.unit === 'day') body = `custom:day:${interval}`
  else if (custom.unit === 'year') body = `custom:year:${interval}`
  else if (custom.unit === 'month') body = `custom:month:${interval}:${custom.monthMode === 'nth' ? 'nth' : 'day'}`
  else {
    const days = [...new Set(custom.weekdays.filter((day) => day >= 0 && day <= 6))].sort((a, b) => a - b)
    body = `custom:week:${interval}:${(days.length > 0 ? days : [0]).join(',')}`
  }
  if (custom.end === 'count') return `${body}|count:${clamp(Math.trunc(custom.count), 1, 366)}`
  if (custom.end === 'until' && DATE.test(custom.until)) return `${body}|until:${custom.until}`
  return body
}

export function decodeRule(rule: string): { preset: RepeatPreset | 'custom'; custom: CustomRepeat } | null {
  const raw = rule.trim()
  if (!raw || raw.length > 160) return null
  const split = raw.indexOf('|')
  const body = split < 0 ? raw : raw.slice(0, split)
  const bound = split < 0 ? '' : raw.slice(split + 1)
  const custom = defaultCustom('2026-01-01')
  if (bound.startsWith('count:')) {
    const count = Number(bound.slice('count:'.length))
    if (!Number.isInteger(count) || count < 1 || count > 366) return null
    custom.end = 'count'
    custom.count = count
  } else if (bound.startsWith('until:')) {
    const until = bound.slice('until:'.length)
    if (!DATE.test(until)) return null
    custom.end = 'until'
    custom.until = until
  } else if (bound) {
    return null
  }
  if ((REPEAT_PRESETS as readonly string[]).includes(body)) {
    if (body === 'none' && bound) return null
    return { preset: body as RepeatPreset, custom }
  }
  const parts = body.split(':')
  if (parts[0] !== 'custom' || parts.length < 3) return null
  const interval = Number(parts[2])
  if (!Number.isInteger(interval) || interval < 1 || interval > 30) return null
  custom.interval = interval
  if (parts[1] === 'day' && parts.length === 3) custom.unit = 'day'
  else if (parts[1] === 'year' && parts.length === 3) custom.unit = 'year'
  else if (parts[1] === 'month' && parts.length === 4 && (parts[3] === 'day' || parts[3] === 'nth')) {
    custom.unit = 'month'
    custom.monthMode = parts[3]
  } else if (parts[1] === 'week' && parts.length === 4) {
    const days = parts[3].split(',').map(Number)
    if (days.length === 0 || days.some((day) => !Number.isInteger(day) || day < 0 || day > 6)) return null
    const sorted = [...days].sort((a, b) => a - b)
    if (sorted.join(',') !== days.join(',') || new Set(sorted).size !== sorted.length) return null
    custom.unit = 'week'
    custom.weekdays = sorted
  } else {
    return null
  }
  return { preset: 'custom', custom }
}

export function zoneOffsetMinutes(instant: Date, timeZone: string): number | null {
  if (!Number.isFinite(instant.getTime())) return null
  const parts = zoneParts(instant, timeZone)
  if (!parts) return null
  const asUtc = Date.UTC(parts.year, parts.month - 1, parts.day, parts.hour, parts.minute, parts.second)
  return Math.round((asUtc - instant.getTime()) / 60000)
}

/** Wall clock in `timeZone` as a UTC instant. Two passes cover an offset that depends on the instant. */
export function zonedInstant(date: string, time: string, timeZone: string): Date | null {
  const day = dateParts(date)
  const clock = timeParts(time)
  if (!day || !clock) return null
  const utcGuess = Date.UTC(day.year, day.month - 1, day.day, clock.hour, clock.minute, 0)
  const first = zoneOffsetMinutes(new Date(utcGuess), timeZone)
  if (first === null || first < -840 || first > 840) return null
  let utc = utcGuess - first * 60000
  const second = zoneOffsetMinutes(new Date(utc), timeZone)
  if (second === null) return null
  if (second !== first) utc = utcGuess - second * 60000
  const result = new Date(utc)
  const check = wallParts(result.toISOString(), timeZone)
  if (!check || check.date !== date || check.time !== time) return null
  return result
}

export function wallParts(iso: string, timeZone: string): { date: string; time: string } | null {
  const instant = new Date(iso)
  const parts = zoneParts(instant, timeZone)
  if (!parts) return null
  const month = String(parts.month).padStart(2, '0')
  const day = String(parts.day).padStart(2, '0')
  const hour = String(parts.hour).padStart(2, '0')
  const minute = String(parts.minute).padStart(2, '0')
  return { date: `${parts.year}-${month}-${day}`, time: `${hour}:${minute}` }
}

export function instantsFromDraft(draft: {
  startDate: string
  startTime: string
  endDate: string
  endTime: string
  allDay: boolean
  timeZone: string
}): { starts_at: string; ends_at: string; utc_offset_minutes: number } | null {
  const zone = draft.timeZone || 'UTC'
  const start = zonedInstant(draft.startDate, draft.allDay ? '00:00' : draft.startTime, zone)
  const endDate = draft.allDay ? shiftDate(draft.endDate, 1) : draft.endDate
  const end = zonedInstant(endDate, draft.allDay ? '00:00' : draft.endTime, zone)
  if (!start || !end || end.getTime() <= start.getTime()) return null
  if (end.getTime() - start.getTime() > 14 * 24 * 60 * 60 * 1000) return null
  const offset = zoneOffsetMinutes(start, zone)
  if (offset === null || offset < -840 || offset > 840) return null
  return { starts_at: start.toISOString(), ends_at: end.toISOString(), utc_offset_minutes: offset }
}

/** Wall clock in the event zone. An all-day end is the inclusive last day. */
export function spanFromEvent(event: {
  starts_at: string
  ends_at: string
  all_day: boolean
  time_zone: string
}): { startDate: string; startTime: string; endDate: string; endTime: string } | null {
  const zone = event.time_zone || 'UTC'
  const start = wallParts(event.starts_at, zone)
  const end = wallParts(event.ends_at, zone)
  if (!start || !end) return null
  if (event.all_day && end.time === '00:00') {
    const inclusive = shiftDate(end.date, -1)
    return {
      startDate: start.date,
      startTime: '00:00',
      endDate: inclusive >= start.date ? inclusive : start.date,
      endTime: '00:00'
    }
  }
  return { startDate: start.date, startTime: start.time, endDate: end.date, endTime: end.time }
}

export function emailOk(email: string): boolean {
  const value = email.trim().toLowerCase()
  const at = value.indexOf('@')
  if (at <= 0 || at !== value.lastIndexOf('@')) return false
  const name = value.slice(0, at)
  const domain = value.slice(at + 1)
  if (!name || name.includes(' ') || domain.includes(' ')) return false
  if (name.length + domain.length >= 120) return false
  if (!domain.includes('.') || domain.startsWith('.') || domain.endsWith('.')) return false
  return [...value].every((ch) => ch.charCodeAt(0) < 128 && ch.charCodeAt(0) >= 32)
}

function dateParts(date: string): { year: number; month: number; day: number } | null {
  const match = DATE.exec(date)
  if (!match) return null
  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  if (month < 1 || month > 12 || day < 1 || day > 31) return null
  const check = new Date(Date.UTC(year, month - 1, day))
  if (check.getUTCFullYear() !== year || check.getUTCMonth() !== month - 1 || check.getUTCDate() !== day) return null
  return { year, month, day }
}

function timeParts(time: string): { hour: number; minute: number } | null {
  const match = TIME.exec(time)
  if (!match) return null
  const hour = Number(match[1])
  const minute = Number(match[2])
  if (hour > 23 || minute > 59) return null
  return { hour, minute }
}

function zoneParts(
  instant: Date,
  timeZone: string
): { year: number; month: number; day: number; hour: number; minute: number; second: number } | null {
  if (!Number.isFinite(instant.getTime()) || !timeZone) return null
  try {
    const parts = new Intl.DateTimeFormat('en-US', {
      timeZone,
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hourCycle: 'h23'
    }).formatToParts(instant)
    const read = (type: Intl.DateTimeFormatPartTypes) => Number(parts.find((part) => part.type === type)?.value)
    const year = read('year')
    const month = read('month')
    const day = read('day')
    let hour = read('hour')
    const minute = read('minute')
    const second = read('second')
    if ([year, month, day, hour, minute, second].some((part) => !Number.isFinite(part))) return null
    if (hour === 24) hour = 0
    return { year, month, day, hour, minute, second }
  } catch {
    return null
  }
}

function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min
  return Math.min(max, Math.max(min, value))
}
