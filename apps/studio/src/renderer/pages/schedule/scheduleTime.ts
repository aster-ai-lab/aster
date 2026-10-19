export type ScheduleMode = 'day' | 'week' | 'month'

export const HOUR_PX = 48
export const DAY_MINUTES = 24 * 60

export type TimedSlice = {
  id: string
  start: Date
  end: Date
}

export type PlacedSlice = {
  id: string
  top: number
  height: number
  lane: number
  lanes: number
}

export function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate())
}

export function addDays(date: Date, days: number): Date {
  const next = new Date(date)
  next.setDate(next.getDate() + days)
  return next
}

export function startOfWeek(date: Date): Date {
  const day = startOfDay(date)
  return addDays(day, -day.getDay())
}

export function startOfMonth(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), 1)
}

export function addMonths(date: Date, months: number): Date {
  return new Date(date.getFullYear(), date.getMonth() + months, 1)
}

export function sameDay(left: Date, right: Date): boolean {
  return (
    left.getFullYear() === right.getFullYear() &&
    left.getMonth() === right.getMonth() &&
    left.getDate() === right.getDate()
  )
}

/** Inclusive start and exclusive end for the grid the user is looking at. */
export function visibleRange(mode: ScheduleMode, anchor: Date): { start: Date; end: Date } {
  if (mode === 'day') {
    const start = startOfDay(anchor)
    return { start, end: addDays(start, 1) }
  }
  if (mode === 'week') {
    const start = startOfWeek(anchor)
    return { start, end: addDays(start, 7) }
  }
  const month = startOfMonth(anchor)
  const start = startOfWeek(month)
  return { start, end: addDays(start, 42) }
}

export function daysInRange(start: Date, end: Date): Date[] {
  const days: Date[] = []
  for (let cursor = startOfDay(start); cursor < end; cursor = addDays(cursor, 1)) days.push(cursor)
  return days
}

export function shiftAnchor(mode: ScheduleMode, anchor: Date, step: -1 | 1): Date {
  if (mode === 'day') return addDays(anchor, step)
  if (mode === 'week') return addDays(anchor, step * 7)
  return new Date(anchor.getFullYear(), anchor.getMonth() + step, 1)
}

export function rangeLabel(start: Date, end: Date, language: string): string {
  const last = addDays(end, -1)
  if (
    language.toLowerCase().startsWith('zh') &&
    start.getMonth() === last.getMonth() &&
    start.getFullYear() === last.getFullYear()
  ) {
    return `${start.getFullYear()}年${start.getMonth() + 1}月${start.getDate()}日 - ${last.getDate()}日`
  }
  const full = new Intl.DateTimeFormat(language, { year: 'numeric', month: 'short', day: 'numeric' })
  return `${full.format(start)} – ${full.format(last)}`
}

export function gmtLabel(date: Date): string {
  const minutes = -date.getTimezoneOffset()
  const sign = minutes >= 0 ? '+' : '-'
  const abs = Math.abs(minutes)
  const hours = Math.floor(abs / 60)
  const rest = abs % 60
  return rest === 0 ? `GMT${sign}${hours}` : `GMT${sign}${hours}:${String(rest).padStart(2, '0')}`
}

export function dateInputValue(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${date.getFullYear()}-${month}-${day}`
}

export function timeInputValue(date: Date): string {
  return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`
}

export type LocalSpan = {
  startDate: string
  startTime: string
  endDate: string
  endTime: string
}

/** Moving the start onto or past the end keeps a one-hour span. An earlier end stays for the form to reject. */
export function moveSpan(current: LocalSpan, patch: Partial<LocalSpan>, edge: 'start' | 'end'): LocalSpan {
  const next = { ...current, ...patch }
  const start = composeLocal(next.startDate, next.startTime)
  const end = composeLocal(next.endDate, next.endTime)
  if (!start || !end || end > start || edge === 'end') return next
  const bumped = new Date(start.getTime() + 60 * 60 * 1000)
  return { ...next, endDate: dateInputValue(bumped), endTime: timeInputValue(bumped) }
}

/** Quarter hours, plus the current minute when an existing event is off the grid. */
export function timeChoices(current: string): string[] {
  const choices: string[] = []
  for (let hour = 0; hour < 24; hour += 1) {
    for (const minute of [0, 15, 30, 45]) {
      choices.push(`${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')}`)
    }
  }
  if (/^\d{2}:\d{2}$/.test(current) && !choices.includes(current)) {
    choices.push(current)
    choices.sort()
  }
  return choices
}

export function composeLocal(date: string, time: string): Date | null {
  const dateParts = date.split('-').map(Number)
  const timeParts = time.split(':').map(Number)
  if (dateParts.length !== 3 || timeParts.length < 2) return null
  const [year, month, day] = dateParts
  const [hour, minute] = timeParts
  if ([year, month, day, hour, minute].some((part) => !Number.isFinite(part))) return null
  const value = new Date(year, month - 1, day, hour, minute, 0, 0)
  if (value.getFullYear() !== year || value.getMonth() !== month - 1 || value.getDate() !== day) return null
  return value
}

/** Next whole hour on the same local day, or 09:00 when the anchor is another day. */
export function defaultSlot(anchor: Date, now: Date): { start: Date; end: Date } {
  const start = startOfDay(anchor)
  if (sameDay(anchor, now)) start.setHours(Math.min(now.getHours() + 1, 22))
  else start.setHours(9)
  return { start, end: new Date(start.getTime() + 60 * 60 * 1000) }
}

export function slotFromOffset(day: Date, offsetPx: number, hourPx = HOUR_PX): { start: Date; end: Date } {
  const minutes = Math.max(0, Math.min(DAY_MINUTES - 30, Math.floor(((offsetPx / hourPx) * 60) / 30) * 30))
  const start = startOfDay(day)
  start.setMinutes(minutes)
  return { start, end: new Date(start.getTime() + 60 * 60 * 1000) }
}

/** Paint the part of each event that falls on `day`. Overlaps share the column. */
export function placeOnDay(events: readonly TimedSlice[], day: Date, hourPx = HOUR_PX): PlacedSlice[] {
  const dayStart = startOfDay(day).getTime()
  const dayEnd = dayStart + DAY_MINUTES * 60 * 1000
  const slices = events
    .flatMap((event) => {
      const start = Math.max(event.start.getTime(), dayStart)
      const end = Math.min(event.end.getTime(), dayEnd)
      if (end <= start) return []
      const topMin = (start - dayStart) / 60000
      const heightMin = Math.max((end - start) / 60000, 20)
      return [{ id: event.id, topMin, heightMin, endMin: topMin + heightMin }]
    })
    .sort((left, right) => left.topMin - right.topMin || left.endMin - right.endMin)

  const laneEnds: number[] = []
  const laned = slices.map((slice) => {
    let lane = laneEnds.findIndex((end) => end <= slice.topMin + 0.01)
    if (lane < 0) {
      lane = laneEnds.length
      laneEnds.push(slice.endMin)
    } else {
      laneEnds[lane] = slice.endMin
    }
    return { ...slice, lane }
  })

  return laned.map((slice) => {
    const group = laned.filter((other) => slice.topMin < other.endMin && other.topMin < slice.endMin)
    const lanes = group.reduce((max, other) => Math.max(max, other.lane), 0) + 1
    return {
      id: slice.id,
      top: (slice.topMin / 60) * hourPx,
      height: Math.max((slice.heightMin / 60) * hourPx, 22),
      lane: slice.lane,
      lanes
    }
  })
}
