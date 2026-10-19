import { memberRequest, MemberOrgError, PERSONAL_CENTER } from '@renderer/data/memberOrgs'

const DAY = /^\d{4}-\d{2}-\d{2}$/
const MAX_QUERY_DAYS = 93

export class OrgUsageError extends Error {
  readonly code: 'org' | 'invalid' | 'load'

  constructor(code: 'org' | 'invalid' | 'load', message = code) {
    super(message)
    this.name = 'OrgUsageError'
    this.code = code
  }
}

export interface OrgUsageTotals {
  tokens: number
  requests: number
  succeeded: number
  failed: number
}

export interface OrgUsageLine {
  day: string
  modelId: string
  endpoint: string
  tokens: number
  requests: number
  succeeded: number
  failed: number
}

export interface OrgUsageSummary {
  fromDay: string
  toDay: string
  totals: OrgUsageTotals
  byDay: OrgUsageLine[]
  byModel: OrgUsageLine[]
  lines: OrgUsageLine[]
}

/** 团队用量的筛选。两个字段都空是全团队。同时填写时会员优先。 */
export interface OrgUsageScope {
  departmentId?: string
  userId?: string
}

export interface UsageSpan {
  days: number
  currentDays: number
}

/** 窗口天数。上一期放进同一次请求，且不超过接口的 93 天。 */
export function usageSpan(windowKey: string): UsageSpan {
  const currentDays = windowKey === '7d' ? 7 : windowKey === '90d' ? 90 : 30
  const doubled = currentDays * 2
  return { currentDays, days: doubled <= MAX_QUERY_DAYS ? doubled : currentDays }
}

export function shiftUtcDay(day: string, delta: number): string {
  const [year, month, date] = day.split('-').map(Number)
  const next = new Date(Date.UTC(year, (month || 1) - 1, date || 1))
  next.setUTCDate(next.getUTCDate() + delta)
  return next.toISOString().slice(0, 10)
}

export function parseOrgUsage(payload: unknown): OrgUsageSummary {
  const row = record(payload)
  const totals = record(row.totals)
  const fromDay = requireDay(row.from_day)
  const toDay = requireDay(row.to_day)
  return {
    fromDay,
    toDay,
    totals: {
      tokens: count(totals.tokens),
      requests: count(totals.requests),
      succeeded: count(totals.succeeded),
      failed: count(totals.failed)
    },
    byDay: list(row.by_day).map((item) => parseDay(item)),
    byModel: list(row.by_model).map((item) => parseModel(item, toDay)),
    lines: list(row.lines).map((item) => parseLine(item))
  }
}

export function sumLines(lines: readonly OrgUsageLine[]): OrgUsageTotals {
  return lines.reduce<OrgUsageTotals>(
    (totals, line) => ({
      tokens: totals.tokens + line.tokens,
      requests: totals.requests + line.requests,
      succeeded: totals.succeeded + line.succeeded,
      failed: totals.failed + line.failed
    }),
    { tokens: 0, requests: 0, succeeded: 0, failed: 0 }
  )
}

export interface UsageWindowSlice {
  current: OrgUsageLine[]
  previous: OrgUsageLine[] | null
  /** 成功和失败能落在当前窗口时才有值。 */
  outcomes: { succeeded: number; failed: number } | null
  previousOutcomes: { succeeded: number; failed: number } | null
  modelLines: OrgUsageLine[]
}

/** 用响应里的日期切出当前窗口。有明细行时按行切，否则按日合计切。 */
export function sliceUsage(summary: OrgUsageSummary, currentDays: number): UsageWindowSlice {
  const start = shiftUtcDay(summary.toDay, -(currentDays - 1))
  const previousStart = shiftUtcDay(start, -currentDays)
  const hasPrevious = summary.fromDay < start
  const inCurrent = (day: string) => day >= start && day <= summary.toDay
  const inPrevious = (day: string) => day >= previousStart && day < start

  if (summary.lines.length > 0) {
    const current = summary.lines.filter((line) => inCurrent(line.day))
    const previous = hasPrevious ? summary.lines.filter((line) => inPrevious(line.day)) : null
    return {
      current,
      previous,
      outcomes: sumLines(current),
      previousOutcomes: previous ? sumLines(previous) : null,
      modelLines: current
    }
  }

  const current = summary.byDay.filter((line) => inCurrent(line.day))
  const previous = hasPrevious ? summary.byDay.filter((line) => inPrevious(line.day)) : null
  const currentTotals = sumLines(current)
  const quiet = currentTotals.tokens === 0 && currentTotals.requests === 0
  return {
    current,
    previous,
    outcomes: hasPrevious
      ? quiet
        ? { succeeded: 0, failed: 0 }
        : null
      : { succeeded: summary.totals.succeeded, failed: summary.totals.failed },
    previousOutcomes: null,
    modelLines: hasPrevious ? [] : summary.byModel
  }
}

export function usagePath(orgId: string, days: number, scope?: OrgUsageScope): string {
  const base = `/api/member/orgs/${encodeURIComponent(orgId)}/usage?days=${days}`
  if (scope?.userId) return `${base}&user_id=${encodeURIComponent(scope.userId)}`
  if (scope?.departmentId) return `${base}&department_id=${encodeURIComponent(scope.departmentId)}`
  return base
}

export async function fetchOrgUsage(
  orgId: string,
  days: number,
  fetchImpl: typeof fetch,
  scope?: OrgUsageScope
): Promise<OrgUsageSummary> {
  if (!orgId || orgId === PERSONAL_CENTER) throw new OrgUsageError('org')
  if (!Number.isSafeInteger(days) || days < 1 || days > MAX_QUERY_DAYS) throw new OrgUsageError('invalid')
  try {
    const data = await memberRequest(usagePath(orgId, days, scope), { method: 'GET' }, fetchImpl)
    return parseOrgUsage(data)
  } catch (error) {
    if (error instanceof OrgUsageError) throw error
    const message = error instanceof MemberOrgError || error instanceof Error ? error.message : 'load'
    throw new OrgUsageError('load', message || 'load')
  }
}

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new OrgUsageError('invalid')
  return value as Record<string, unknown>
}

function list(value: unknown): unknown[] {
  if (value === undefined) return []
  if (!Array.isArray(value)) throw new OrgUsageError('invalid')
  return value
}

function requireDay(value: unknown): string {
  if (typeof value !== 'string' || !DAY.test(value)) throw new OrgUsageError('invalid')
  return value
}

function count(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new OrgUsageError('invalid')
  return value
}

function text(value: unknown): string {
  return typeof value === 'string' ? value : ''
}

function parseLine(value: unknown): OrgUsageLine {
  const row = record(value)
  return {
    day: requireDay(row.day),
    modelId: text(row.model_id),
    endpoint: text(row.endpoint),
    tokens: count(row.tokens),
    requests: count(row.requests),
    succeeded: count(row.succeeded),
    failed: count(row.failed)
  }
}

function parseDay(value: unknown): OrgUsageLine {
  const row = record(value)
  return {
    day: requireDay(row.day),
    modelId: '',
    endpoint: '',
    tokens: count(row.tokens),
    requests: count(row.requests),
    succeeded: count(row.succeeded ?? 0),
    failed: count(row.failed ?? 0)
  }
}

function parseModel(value: unknown, day: string): OrgUsageLine {
  const row = record(value)
  const modelId = text(row.model_id)
  if (!modelId) throw new OrgUsageError('invalid')
  return {
    day,
    modelId,
    endpoint: '',
    tokens: count(row.tokens),
    requests: count(row.requests),
    succeeded: count(row.succeeded ?? 0),
    failed: count(row.failed ?? 0)
  }
}
