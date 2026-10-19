import { trainingEventState, type SessionTrace, type StoredTrainingEvent } from './extractExecutionTrace'

export const TRACE_PAGE_SIZE = 20

export interface TraceHit {
  session: SessionTrace
  event: StoredTrainingEvent
}

export function filterTraceHits(
  sessions: readonly SessionTrace[],
  query: string,
  haystack: (session: SessionTrace, event: StoredTrainingEvent) => string
): TraceHit[] {
  const needle = query.trim().toLowerCase()
  const hits: TraceHit[] = []
  for (const session of sessions) {
    for (const event of session.events) {
      if (!needle || haystack(session, event).toLowerCase().includes(needle)) hits.push({ session, event })
    }
  }
  return hits
}

export function pageTraceHits(
  hits: readonly TraceHit[],
  page: number,
  pageSize = TRACE_PAGE_SIZE
): { page: number; pages: number; total: number; from: number; to: number; hits: TraceHit[] } {
  const paged = pageItems(hits, page, pageSize)
  return { ...paged, hits: paged.items }
}

export interface TraceGroup {
  session: SessionTrace
  events: StoredTrainingEvent[]
  count: number
}

export function groupTraceHits(hits: readonly TraceHit[], totals: ReadonlyMap<string, number>): TraceGroup[] {
  const groups: TraceGroup[] = []
  for (const hit of hits) {
    const last = groups[groups.length - 1]
    if (last && last.session.sessionId === hit.session.sessionId) last.events.push(hit.event)
    else groups.push({ session: hit.session, events: [hit.event], count: totals.get(hit.session.sessionId) ?? 1 })
  }
  return groups
}

export type TraceOutcome = 'failed' | 'done' | 'running'

/** The latest execution end owns the session result. A run still in progress has no end. */
export function sessionTraceOutcome(session: SessionTrace): TraceOutcome {
  for (let index = session.events.length - 1; index >= 0; index -= 1) {
    const record = session.events[index]?.record
    if (record?.event_type !== 'execution_end') continue
    return trainingEventState(record) === 'failed' ? 'failed' : 'done'
  }
  return 'running'
}

export function pageTraceGroups(
  groups: readonly TraceGroup[],
  page: number,
  pageSize = TRACE_PAGE_SIZE
): { page: number; pages: number; total: number; from: number; to: number; groups: TraceGroup[] } {
  const paged = pageItems(groups, page, pageSize)
  return { ...paged, groups: paged.items }
}

function pageItems<T>(
  items: readonly T[],
  page: number,
  pageSize: number
): { page: number; pages: number; total: number; from: number; to: number; items: T[] } {
  const total = items.length
  const pages = total === 0 ? 0 : Math.ceil(total / pageSize)
  const safe = pages === 0 ? 0 : Math.min(Math.max(page, 0), pages - 1)
  const start = safe * pageSize
  const slice = items.slice(start, start + pageSize)
  return {
    page: safe,
    pages,
    total,
    from: total === 0 ? 0 : start + 1,
    to: start + slice.length,
    items: slice
  }
}

export function countTraceHits(hits: readonly TraceHit[]): Map<string, number> {
  const totals = new Map<string, number>()
  for (const hit of hits) totals.set(hit.session.sessionId, (totals.get(hit.session.sessionId) ?? 0) + 1)
  return totals
}
