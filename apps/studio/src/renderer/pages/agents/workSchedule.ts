import { Cron } from 'croner'

import type { JobScheduleSnapshot, Trigger } from '@shared/data/api/schemas/jobs'
import { isJobScheduleDue } from '@shared/job/scheduleDue'

export type WorkPeriod = 'daily' | 'weekly' | 'biweekly' | 'monthly' | 'yearly'
export type WorkScheduleMode = 'period' | 'interval' | 'once'

export type WorkFrequency = {
  mode: WorkScheduleMode
  period: WorkPeriod
  time: string
  weekdays: number[]
  weekday: number
  monthDay: number
  month: number
  hours: number
  at: number | null
  anchorMs: number | null
}

export type WorkScheduleDraft = {
  name: string
  prompt: string
  mode: WorkScheduleMode
  period: WorkPeriod
  time: string
  weekdays: number[]
  weekday: number
  monthDay: number
  month: number
  hours: number
  onceAt: string
}

export type WorkRunStatus = 'success' | 'failure'

export type WorkRun = {
  id: string
  taskId: string
  taskName: string
  status: WorkRunStatus
  at: number
}

export type WorkTask = {
  id: string
  name: string
  prompt: string
  frequency: WorkFrequency
  schedule: JobScheduleSnapshot
}

export type WorkScheduleState = {
  tasks: WorkTask[]
  runs: WorkRun[]
  /** Schedules removed from the list. Due checks still read these snapshots. */
  closed: JobScheduleSnapshot[]
}

export type WorkScheduleError = 'prompt' | 'once' | 'interval' | 'time' | 'weekdays' | 'month'

/** Keeps a paused or closed interval from falling back to createdAt + ms. */
const HELD_NEXT_RUN = '9999-12-31T23:59:00.000Z'

const WEEKDAY_ZH = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']
const WEEKDAY_EN = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']

let memory: WorkScheduleState = emptyWorkScheduleState()

export function emptyWorkScheduleState(): WorkScheduleState {
  return { tasks: [], runs: [], closed: [] }
}

export function loadWorkSchedule(): WorkScheduleState {
  return memory
}

export function commitWorkSchedule(state: WorkScheduleState): void {
  memory = state
}

export function resetWorkScheduleMemory(): void {
  memory = emptyWorkScheduleState()
}

export function defaultWorkScheduleDraft(): WorkScheduleDraft {
  return {
    name: '',
    prompt: '',
    mode: 'period',
    period: 'daily',
    time: '09:00',
    weekdays: [1],
    weekday: 1,
    monthDay: 1,
    month: 1,
    hours: 1,
    onceAt: ''
  }
}

export function draftFromTask(task: WorkTask): WorkScheduleDraft {
  return {
    name: task.name,
    prompt: task.prompt,
    mode: task.frequency.mode,
    period: task.frequency.period,
    time: task.frequency.time,
    weekdays: [...task.frequency.weekdays],
    weekday: task.frequency.weekday,
    monthDay: task.frequency.monthDay,
    month: task.frequency.month,
    hours: task.frequency.hours,
    onceAt: task.frequency.at === null ? '' : toDateTimeLocal(task.frequency.at)
  }
}

export function weekdayLabel(day: number, locale: string): string {
  const labels = locale.toLowerCase().startsWith('zh') ? WEEKDAY_ZH : WEEKDAY_EN
  return labels[day] ?? String(day)
}

export function formatWorkSchedule(frequency: WorkFrequency, locale: string): string {
  const zh = locale.toLowerCase().startsWith('zh')
  if (frequency.mode === 'interval') {
    return zh ? `每 ${frequency.hours} 小时` : `Every ${frequency.hours} hours`
  }
  if (frequency.mode === 'once') {
    const stamp = frequency.at === null ? '' : formatStamp(frequency.at)
    return zh ? `单次 · ${stamp}` : `Once · ${stamp}`
  }
  const time = frequency.time
  if (frequency.period === 'daily') return zh ? `每天 ${time}` : `Daily ${time}`
  if (frequency.period === 'weekly' || frequency.period === 'biweekly') {
    const days = orderedWeekdays(frequency.period === 'biweekly' ? [frequency.weekday] : frequency.weekdays)
      .map((day) => weekdayLabel(day, locale))
      .join(zh ? '、' : ', ')
    if (frequency.period === 'biweekly') return zh ? `双周 ${days} · ${time}` : `Every 2 weeks ${days} · ${time}`
    return zh ? `每周 ${days} · ${time}` : `Weekly ${days} · ${time}`
  }
  if (frequency.period === 'monthly') {
    return zh ? `每月 ${frequency.monthDay}日 · ${time}` : `Monthly day ${frequency.monthDay} · ${time}`
  }
  return zh
    ? `每年 ${frequency.month}月${frequency.monthDay}日 · ${time}`
    : `Yearly ${frequency.month}/${frequency.monthDay} · ${time}`
}

export function isWorkTaskDue(task: WorkTask | undefined, nowMs: number): boolean {
  if (!task || !task.schedule.enabled) return false
  if (task.schedule.trigger.kind === 'once') {
    if (task.schedule.lastRun) return false
    const nextRunMs = task.schedule.nextRun ? Date.parse(task.schedule.nextRun) : null
    return nextRunMs !== null && nextRunMs <= nowMs
  }
  return isJobScheduleDue(task.schedule, nowMs)
}

export function saveWorkTask(
  state: WorkScheduleState,
  draft: WorkScheduleDraft,
  nowMs: number,
  taskId?: string
): { state: WorkScheduleState; error: WorkScheduleError | null } {
  const built = buildFrequency(draft, nowMs, taskId ? findTask(state, taskId)?.frequency.anchorMs ?? null : null)
  if (built.error || !built.frequency || !built.trigger || built.nextRun === undefined) {
    return { state, error: built.error ?? 'time' }
  }
  const existing = taskId ? findTask(state, taskId) : undefined
  if (taskId && !existing) return { state, error: null }
  const id = existing?.id ?? nextTaskId(state.tasks)
  const createdAt = existing?.schedule.createdAt ?? new Date(nowMs).toISOString()
  const spentOnce =
    existing?.schedule.trigger.kind === 'once' &&
    existing.schedule.lastRun !== null &&
    built.frequency.mode === 'once' &&
    built.frequency.at === existing.frequency.at
  const nextRun = spentOnce ? null : built.nextRun
  const task: WorkTask = {
    id,
    name: draft.name.trim() || draft.prompt.trim().slice(0, 24) || '未命名任务',
    prompt: draft.prompt.trim(),
    frequency: built.frequency,
    schedule: toSnapshot({
      id,
      name: draft.name.trim() || null,
      trigger: built.trigger,
      nextRun,
      nowMs,
      enabled: existing ? existing.schedule.enabled : true,
      lastRun: existing?.schedule.lastRun ?? null,
      createdAt
    })
  }
  if (!task.schedule.enabled) task.schedule.nextRun = heldNextRun(task.schedule)
  const tasks = existing ? state.tasks.map((item) => (item.id === id ? task : item)) : [task, ...state.tasks]
  return { state: { ...state, tasks }, error: null }
}

export function pauseWorkTask(state: WorkScheduleState, taskId: string, nowMs: number): WorkScheduleState {
  return mapTask(state, taskId, (task) => ({
    ...task,
    schedule: {
      ...task.schedule,
      enabled: false,
      nextRun: heldNextRun(task.schedule),
      updatedAt: new Date(nowMs).toISOString()
    }
  }))
}

export function resumeWorkTask(state: WorkScheduleState, taskId: string, nowMs: number): WorkScheduleState {
  return mapTask(state, taskId, (task) => {
    const nextRun = task.schedule.trigger.kind === 'once' && task.schedule.lastRun ? null : nextRunFor(task.frequency, nowMs)
    return {
      ...task,
      schedule: {
        ...task.schedule,
        enabled: true,
        nextRun: nextRun === null ? null : new Date(nextRun).toISOString(),
        updatedAt: new Date(nowMs).toISOString()
      }
    }
  })
}

export function deleteWorkTask(state: WorkScheduleState, taskId: string): WorkScheduleState {
  const task = state.tasks.find((item) => item.id === taskId)
  if (!task) return state
  return {
    ...state,
    tasks: state.tasks.filter((item) => item.id !== taskId),
    closed: [...state.closed, closeSchedule(task.schedule)]
  }
}

function heldNextRun(schedule: JobScheduleSnapshot): string | null {
  return schedule.trigger.kind === 'interval' ? HELD_NEXT_RUN : null
}

function closeSchedule(schedule: JobScheduleSnapshot): JobScheduleSnapshot {
  return { ...schedule, enabled: false, nextRun: heldNextRun(schedule) ?? HELD_NEXT_RUN }
}

export function manualWorkTaskRunStatus(task: WorkTask): WorkRunStatus {
  return task.schedule.enabled ? 'success' : 'failure'
}

export function recordWorkTaskRun(
  state: WorkScheduleState,
  taskId: string,
  status: WorkRunStatus,
  nowMs: number
): WorkScheduleState {
  const task = findTask(state, taskId)
  if (!task) return state
  const run: WorkRun = {
    id: `run-${state.runs.length + 1}`,
    taskId: task.id,
    taskName: task.name,
    status,
    at: nowMs
  }
  const advanced = advanceAfterRun(task, nowMs)
  return {
    ...state,
    tasks: state.tasks.map((item) => (item.id === task.id ? advanced : item)),
    runs: [run, ...state.runs]
  }
}

function advanceAfterRun(task: WorkTask, firedAt: number): WorkTask {
  if (!task.schedule.enabled) {
    return {
      ...task,
      schedule: { ...task.schedule, lastRun: new Date(firedAt).toISOString(), updatedAt: new Date(firedAt).toISOString() }
    }
  }
  if (task.schedule.trigger.kind === 'once') {
    return {
      ...task,
      schedule: {
        ...task.schedule,
        lastRun: new Date(firedAt).toISOString(),
        nextRun: null,
        updatedAt: new Date(firedAt).toISOString()
      }
    }
  }
  const nextRun = nextRunFor(task.frequency, firedAt)
  return {
    ...task,
    schedule: {
      ...task.schedule,
      lastRun: new Date(firedAt).toISOString(),
      nextRun: nextRun === null ? null : new Date(nextRun).toISOString(),
      updatedAt: new Date(firedAt).toISOString()
    }
  }
}

function buildFrequency(
  draft: WorkScheduleDraft,
  nowMs: number,
  previousAnchor: number | null
): { frequency?: WorkFrequency; trigger?: Trigger; nextRun?: number | null; error: WorkScheduleError | null } {
  if (!draft.prompt.trim()) return { error: 'prompt' }
  const time = normalizeTime(draft.time)
  if (draft.mode !== 'interval' && draft.mode !== 'once' && !time) return { error: 'time' }

  if (draft.mode === 'interval') {
    if (!Number.isInteger(draft.hours) || draft.hours < 1) return { error: 'interval' }
    const frequency: WorkFrequency = {
      mode: 'interval',
      period: 'daily',
      time: time ?? '09:00',
      weekdays: [1],
      weekday: 1,
      monthDay: 1,
      month: 1,
      hours: draft.hours,
      at: null,
      anchorMs: null
    }
    return {
      frequency,
      trigger: { kind: 'interval', ms: draft.hours * 3_600_000, anchor: 'lastRun' },
      nextRun: nowMs + draft.hours * 3_600_000,
      error: null
    }
  }

  if (draft.mode === 'once') {
    const at = Date.parse(draft.onceAt)
    if (!Number.isFinite(at) || at <= nowMs) return { error: 'once' }
    const frequency: WorkFrequency = {
      mode: 'once',
      period: 'daily',
      time: time ?? '09:00',
      weekdays: [1],
      weekday: 1,
      monthDay: 1,
      month: 1,
      hours: 1,
      at,
      anchorMs: null
    }
    return { frequency, trigger: { kind: 'once', at }, nextRun: at, error: null }
  }

  if (!time) return { error: 'time' }
  const weekdays = orderedWeekdays(draft.weekdays)
  if (draft.period === 'weekly' && weekdays.length === 0) return { error: 'weekdays' }
  if (draft.period === 'monthly' && (draft.monthDay < 1 || draft.monthDay > 31)) return { error: 'month' }
  if (draft.period === 'yearly' && (draft.month < 1 || draft.month > 12 || draft.monthDay < 1 || draft.monthDay > 31)) {
    return { error: 'month' }
  }
  const weekday = clampDay(draft.weekday)
  const anchorMs = draft.period === 'biweekly' ? (previousAnchor ?? nextWeekdayAt(weekday, time.hour, time.minute, nowMs - 1)) : null
  const frequency: WorkFrequency = {
    mode: 'period',
    period: draft.period,
    time: formatClock(time.hour, time.minute),
    weekdays,
    weekday,
    monthDay: draft.monthDay,
    month: draft.month,
    hours: 1,
    at: null,
    anchorMs
  }
  const expr = cronExpr(frequency, time)
  if (!expr) return { error: 'time' }
  const nextRun = nextRunFor(frequency, nowMs - 1)
  if (nextRun === null) return { error: 'time' }
  return { frequency, trigger: { kind: 'cron', expr }, nextRun, error: null }
}

function nextRunFor(frequency: WorkFrequency, afterMs: number): number | null {
  if (frequency.mode === 'interval') return afterMs + frequency.hours * 3_600_000
  if (frequency.mode === 'once') return frequency.at !== null && frequency.at > afterMs ? frequency.at : null
  const time = parseTime(frequency.time)
  if (!time) return null
  if (frequency.mode === 'period' && frequency.period === 'biweekly') {
    const anchor = frequency.anchorMs ?? nextWeekdayAt(frequency.weekday, time.hour, time.minute, afterMs)
    return nextBiweekly(frequency.weekday, time.hour, time.minute, anchor, afterMs)
  }
  const expr = cronExpr(frequency, time)
  return expr ? nextCron(expr, afterMs) : null
}

function cronExpr(frequency: WorkFrequency, time: { hour: number; minute: number }): string | null {
  const head = `${time.minute} ${time.hour}`
  if (frequency.period === 'daily') return `${head} * * *`
  if (frequency.period === 'weekly') {
    const days = orderedWeekdays(frequency.weekdays)
    if (days.length === 0) return null
    return `${head} * * ${days.join(',')}`
  }
  if (frequency.period === 'biweekly') return `${head} * * ${clampDay(frequency.weekday)}`
  if (frequency.period === 'monthly') return `${head} ${frequency.monthDay} * *`
  if (frequency.period === 'yearly') return `${head} ${frequency.monthDay} ${frequency.month} *`
  return null
}

function nextCron(expr: string, afterMs: number): number | null {
  const cron = new Cron(expr, { paused: true })
  try {
    const next = cron.nextRun(new Date(afterMs))
    return next ? next.getTime() : null
  } catch {
    return null
  } finally {
    cron.stop()
  }
}

function nextBiweekly(weekday: number, hour: number, minute: number, anchorMs: number, afterMs: number): number {
  let candidate = nextWeekdayAt(weekday, hour, minute, afterMs)
  if (daysBetween(anchorMs, candidate) % 14 !== 0) candidate = addDays(candidate, 7)
  return candidate
}

function nextWeekdayAt(weekday: number, hour: number, minute: number, afterMs: number): number {
  const start = new Date(afterMs)
  for (let offset = 0; offset < 8; offset += 1) {
    const candidate = new Date(start.getFullYear(), start.getMonth(), start.getDate() + offset, hour, minute, 0, 0)
    if (candidate.getDay() === weekday && candidate.getTime() > afterMs) return candidate.getTime()
  }
  return addDays(new Date(start.getFullYear(), start.getMonth(), start.getDate(), hour, minute, 0, 0).getTime(), 7)
}

function addDays(ms: number, days: number): number {
  const date = new Date(ms)
  return new Date(date.getFullYear(), date.getMonth(), date.getDate() + days, date.getHours(), date.getMinutes(), 0, 0).getTime()
}

function daysBetween(startMs: number, endMs: number): number {
  const start = new Date(startMs)
  const end = new Date(endMs)
  const startUtc = Date.UTC(start.getFullYear(), start.getMonth(), start.getDate())
  const endUtc = Date.UTC(end.getFullYear(), end.getMonth(), end.getDate())
  return Math.round((endUtc - startUtc) / 86_400_000)
}

function toSnapshot(input: {
  id: string
  name: string | null
  trigger: Trigger
  nextRun: number | null
  nowMs: number
  enabled: boolean
  lastRun: string | null
  createdAt: string
}): JobScheduleSnapshot {
  return {
    id: input.id,
    type: 'workbench.scheduled',
    name: input.name,
    trigger: input.trigger,
    jobInputTemplate: null,
    enabled: input.enabled,
    nextRun: input.nextRun === null ? null : new Date(input.nextRun).toISOString(),
    lastRun: input.lastRun,
    catchUpPolicy: { kind: 'skip-missed' },
    metadata: {},
    createdAt: input.createdAt,
    updatedAt: new Date(input.nowMs).toISOString()
  }
}

function mapTask(state: WorkScheduleState, taskId: string, update: (task: WorkTask) => WorkTask): WorkScheduleState {
  if (!findTask(state, taskId)) return state
  return { ...state, tasks: state.tasks.map((task) => (task.id === taskId ? update(task) : task)) }
}

function findTask(state: WorkScheduleState, taskId: string): WorkTask | undefined {
  return state.tasks.find((task) => task.id === taskId)
}

function nextTaskId(tasks: readonly WorkTask[]): string {
  const max = tasks.reduce((highest, task) => {
    const match = /^task-(\d+)$/.exec(task.id)
    return Math.max(highest, match ? Number(match[1]) : 0)
  }, 0)
  return `task-${max + 1}`
}

function orderedWeekdays(days: readonly number[]): number[] {
  const unique = [...new Set(days.map(clampDay))]
  return unique.sort((left, right) => weekdayOrder(left) - weekdayOrder(right))
}

function weekdayOrder(day: number): number {
  return day === 0 ? 7 : day
}

function clampDay(day: number): number {
  if (!Number.isInteger(day) || day < 0 || day > 6) return 1
  return day
}

function normalizeTime(value: string): { hour: number; minute: number } | null {
  const parsed = parseTime(value.trim())
  return parsed
}

function parseTime(value: string): { hour: number; minute: number } | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(value.trim())
  if (!match) return null
  const hour = Number(match[1])
  const minute = Number(match[2])
  if (hour > 23 || minute > 59) return null
  return { hour, minute }
}

function formatClock(hour: number, minute: number): string {
  return `${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')}`
}

function formatStamp(ms: number): string {
  const date = new Date(ms)
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

function toDateTimeLocal(ms: number): string {
  const date = new Date(ms)
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`
}
