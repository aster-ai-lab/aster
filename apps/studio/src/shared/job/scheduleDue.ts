import type { JobScheduleSnapshot } from '@shared/data/api/schemas/jobs'

/**
 * Whether a persisted job schedule is due at `nowMs`.
 *
 * This is the catch-up overdue rule JobManager already applies:
 * - cron: the persisted `nextRun` is at or before now
 * - interval: the persisted `nextRun`, or `(lastRun ?? createdAt) + ms` when
 *   an older row never stored `nextRun`
 * - once: not due here. SchedulerService arms a one-shot timer, and a fired
 *   once row has `nextRun === null`
 *
 * Paused rows are not due: JobManager clears `nextRun` while a schedule is held.
 */
export function isJobScheduleDue(schedule: JobScheduleSnapshot, nowMs: number): boolean {
  const trigger = schedule.trigger
  if (trigger.kind === 'cron') {
    const nextRunMs = schedule.nextRun ? Date.parse(schedule.nextRun) : null
    return nextRunMs !== null && nextRunMs <= nowMs
  }
  if (trigger.kind === 'interval') {
    const nextRunMs = schedule.nextRun ? Date.parse(schedule.nextRun) : null
    if (nextRunMs !== null) return nextRunMs <= nowMs
    const lastRunMs = schedule.lastRun ? Date.parse(schedule.lastRun) : null
    const anchorMs = lastRunMs ?? Date.parse(schedule.createdAt)
    return anchorMs + trigger.ms <= nowMs
  }
  return false
}
