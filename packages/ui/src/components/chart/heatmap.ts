import { parseDayKey, toDayKey } from '../../lib/date'

export interface ActivityHeatmapBucket {
  date: string
  value: number | null
}

export interface ActivityHeatmapDay extends ActivityHeatmapBucket {
  date: string
  dateValue: Date
  isOutsideRange: boolean
}

export interface ActivityHeatmapRange {
  from?: number
  to?: number
}

const MIN_HEATMAP_DAYS = 365

function startOfLocalDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate())
}

function startOfLocalWeek(date: Date, firstDayOfWeek: number): Date {
  const day = startOfLocalDay(date)
  day.setDate(day.getDate() - ((day.getDay() - firstDayOfWeek + 7) % 7))
  return day
}

function endOfLocalWeek(date: Date, firstDayOfWeek: number): Date {
  const day = startOfLocalWeek(date, firstDayOfWeek)
  day.setDate(day.getDate() + 6)
  return day
}

/** Build a continuous one-year grid without fixed-millisecond DST arithmetic. */
export function buildActivityHeatmapDays(
  buckets: ActivityHeatmapBucket[],
  range: ActivityHeatmapRange | undefined,
  firstDayOfWeek: number,
  now = new Date()
): ActivityHeatmapDay[] {
  const today = startOfLocalDay(now)
  let rangeFirstDay: Date
  let rangeLastDay: Date

  if (range?.from !== undefined) {
    rangeFirstDay = startOfLocalDay(new Date(range.from))
    rangeLastDay = startOfLocalDay(new Date(range.to ?? now.getTime()))
  } else if (buckets.length > 0) {
    const times = buckets.map((bucket) => parseDayKey(bucket.date).getTime())
    rangeFirstDay = new Date(Math.min(...times))
    rangeLastDay = today
  } else {
    rangeLastDay = today
    rangeFirstDay = new Date(today)
    rangeFirstDay.setDate(rangeFirstDay.getDate() - 29)
  }

  const minimumFirstDay = new Date(rangeLastDay)
  minimumFirstDay.setDate(minimumFirstDay.getDate() - MIN_HEATMAP_DAYS + 1)
  const displayFirstDay = rangeFirstDay.getTime() < minimumFirstDay.getTime() ? rangeFirstDay : minimumFirstDay
  const firstWeekDay = startOfLocalWeek(displayFirstDay, firstDayOfWeek)
  const lastWeekDay = endOfLocalWeek(rangeLastDay, firstDayOfWeek)
  const bucketMap = new Map(buckets.map((bucket) => [bucket.date, bucket]))
  const days: ActivityHeatmapDay[] = []
  const cursor = new Date(firstWeekDay)

  while (cursor.getTime() <= lastWeekDay.getTime()) {
    const date = new Date(cursor)
    const key = toDayKey(date)
    days.push({
      date: key,
      dateValue: date,
      value: bucketMap.get(key)?.value ?? 0,
      isOutsideRange: date.getTime() < rangeFirstDay.getTime() || date.getTime() > rangeLastDay.getTime()
    })
    cursor.setDate(cursor.getDate() + 1)
  }
  return days
}

function quantile(sorted: number[], ratio: number): number {
  if (sorted.length === 0) return 0
  const index = Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * ratio))
  return sorted[index]
}

export function getActivityHeatmapThresholds(values: Array<number | null>): [number, number, number] {
  const positive = values.filter((value): value is number => value !== null && Number.isFinite(value) && value > 0).sort((a, b) => a - b)
  return [quantile(positive, 0.25), quantile(positive, 0.5), quantile(positive, 0.75)]
}

export function getActivityHeatmapIntensity(value: number | null, thresholds: [number, number, number]): 0 | 1 | 2 | 3 | 4 {
  if (value === null || value <= 0) return 0
  if (value <= thresholds[0]) return 1
  if (value <= thresholds[1]) return 2
  if (value <= thresholds[2]) return 3
  return 4
}
