import { useSyncExternalStore } from 'react'
import { useEffect, useMemo, useState } from 'react'

import { PERSONAL_CENTER, readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'
import { fetchOrgModels } from '@renderer/data/orgModels'
import type { AiUsageRecordStatsBucket, AiUsageRecordTimelineBucket } from '@shared/data/api/schemas/aiUsageRecords'

import {
  fetchOrgUsage,
  type OrgUsageLine,
  type OrgUsageScope,
  type OrgUsageSummary,
  OrgUsageError,
  sliceUsage,
  sumLines,
  usageSpan
} from './orgUsage'
import { EMPTY_STATS_METRICS, type GroupByKey, type WindowKey } from './usageAnalytics'

export type OrgUsageSortBy = 'createdAt' | 'totalTokens' | 'requests' | 'succeeded' | 'failed'

export interface OrgUsageEntry {
  id: string
  modelId: string
  modelName: string
  endpoint: string
  day: string
  tokens: number
  requests: number
  succeeded: number
  failed: number
}

interface ReadyUsage {
  key: string
  summary: OrgUsageSummary
  names: ReadonlyMap<string, string>
}

interface UseUsageDataOptions {
  windowKey: WindowKey
  groupBy: GroupByKey
  scope?: OrgUsageScope
}

const EMPTY_BUCKETS: AiUsageRecordTimelineBucket[] = []
const EMPTY_STATS: AiUsageRecordStatsBucket[] = []
const EMPTY_ENTRIES: OrgUsageEntry[] = []

export function useUsageData({ windowKey, groupBy, scope }: UseUsageDataOptions) {
  const orgId = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER)
  const span = usageSpan(windowKey)
  const scopeKey = scope?.userId ? `user:${scope.userId}` : scope?.departmentId ? `dept:${scope.departmentId}` : 'team'
  const key = `${orgId}:${span.days}:${scopeKey}`
  const [ready, setReady] = useState<ReadyUsage | null>(null)
  const [error, setError] = useState<OrgUsageError | null>(() =>
    orgId === PERSONAL_CENTER ? new OrgUsageError('org') : null
  )
  const [loading, setLoading] = useState(orgId !== PERSONAL_CENTER)

  useEffect(() => {
    if (orgId === PERSONAL_CENTER) {
      setReady(null)
      setError(new OrgUsageError('org'))
      setLoading(false)
      return
    }
    let cancelled = false
    setLoading(true)
    setError(null)
    void Promise.all([fetchOrgUsage(orgId, span.days, fetch, scope), modelNames()]).then(
      ([summary, names]) => {
        if (cancelled) return
        setReady({ key, summary, names })
        setLoading(false)
      },
      (reason: unknown) => {
        if (cancelled) return
        setError(reason instanceof OrgUsageError ? reason : new OrgUsageError('load'))
        setLoading(false)
      }
    )
    return () => {
      cancelled = true
    }
  }, [key, orgId, scope, span.days])

  const shown = ready?.key === key ? ready : null
  const needsOrg = error?.code === 'org'
  const loadError = !needsOrg && error ? error.message : null
  const awaiting = shown === null && error === null && orgId !== PERSONAL_CENTER

  return useMemo(() => {
    if (!shown) {
      return emptyUsage(loading || awaiting, needsOrg, loadError)
    }
    const slice = sliceUsage(shown.summary, span.currentDays)
    const totals = sumLines(slice.current)
    const previous = slice.previous ? sumLines(slice.previous) : null
    const names = shown.names
    return {
      timelineBuckets: groupByDay(slice.current),
      overviewBuckets: modelBuckets(slice.modelLines, names),
      exploreTimelineRows: exploreRows(slice.modelLines, groupBy),
      overviewTotals: metrics(totals, slice.outcomes),
      previousOverviewTotals: metrics(previous, slice.previousOutcomes),
      hasPrevious: slice.previous !== null,
      entries: toEntries(slice.current, names),
      modelNames: names,
      timelineLoading: false,
      overviewLoading: false,
      exploreTimelineLoading: false,
      needsOrg: false,
      error: null as string | null
    }
  }, [awaiting, groupBy, loadError, loading, needsOrg, shown, span.currentDays])
}

async function modelNames(): Promise<Map<string, string>> {
  try {
    const catalog = await fetchOrgModels()
    return new Map(catalog.items.map((model) => [model.id, model.name]))
  } catch {
    return new Map()
  }
}

function emptyUsage(loading: boolean, needsOrg: boolean, error: string | null) {
  return {
    timelineBuckets: EMPTY_BUCKETS,
    overviewBuckets: EMPTY_STATS,
    exploreTimelineRows: EMPTY_BUCKETS,
    overviewTotals: metrics(null, null),
    previousOverviewTotals: metrics(null, null),
    hasPrevious: false,
    entries: EMPTY_ENTRIES,
    modelNames: new Map<string, string>(),
    timelineLoading: loading,
    overviewLoading: loading,
    exploreTimelineLoading: loading,
    needsOrg,
    error
  }
}

function metrics(
  totals: { tokens: number; requests: number } | null,
  outcomes: { succeeded: number; failed: number } | null
) {
  return {
    ...EMPTY_STATS_METRICS,
    totalTokens: totals?.tokens ?? 0,
    requestCount: totals?.requests ?? 0,
    recordCount: totals?.requests ?? 0,
    succeeded: outcomes?.succeeded,
    failed: outcomes?.failed
  }
}

function bucket(line: OrgUsageLine, extra: Partial<AiUsageRecordTimelineBucket>): AiUsageRecordTimelineBucket {
  return {
    date: line.day,
    costCurrency: null,
    totalTokens: line.tokens,
    totalNoCacheTokens: 0,
    totalCacheReadTokens: 0,
    totalCacheWriteTokens: 0,
    totalCost: 0,
    recordCount: line.requests,
    requestCount: line.requests,
    estimatedRequestCount: line.succeeded,
    unpricedRequestCount: line.failed,
    ...extra
  }
}

function groupByDay(lines: readonly OrgUsageLine[]): AiUsageRecordTimelineBucket[] {
  const days = new Map<string, OrgUsageLine>()
  for (const line of lines) {
    const current = days.get(line.day)
    days.set(
      line.day,
      current
        ? {
            ...current,
            tokens: current.tokens + line.tokens,
            requests: current.requests + line.requests,
            succeeded: current.succeeded + line.succeeded,
            failed: current.failed + line.failed
          }
        : { ...line, modelId: '', endpoint: '' }
    )
  }
  return Array.from(days.values())
    .sort((left, right) => left.day.localeCompare(right.day))
    .map((line) => bucket(line, {}))
}

function modelBuckets(lines: readonly OrgUsageLine[], names: ReadonlyMap<string, string>): AiUsageRecordStatsBucket[] {
  const groups = groupLines(lines, (line) => line.modelId)
  return Array.from(groups.values())
    .filter((line) => line.modelId)
    .sort((left, right) => right.tokens - left.tokens)
    .map((line) => ({
      groupBy: 'model' as const,
      providerId: null,
      providerName: names.get(line.modelId) ?? null,
      modelId: line.modelId,
      ...metrics(line, line)
    }))
}

function exploreRows(lines: readonly OrgUsageLine[], groupBy: GroupByKey): AiUsageRecordTimelineBucket[] {
  if (groupBy === 'endpoint') {
    const groups = groupLines(
      lines.filter((line) => line.endpoint),
      (line) => `${line.day}\n${line.endpoint}`
    )
    return Array.from(groups.values()).map((line) =>
      bucket(line, {
        sourceType: 'agent',
        sourceId: line.endpoint,
        sourceName: line.endpoint
      })
    )
  }
  const groups = groupLines(
    lines.filter((line) => line.modelId),
    (line) => `${line.day}\n${line.modelId}`
  )
  return Array.from(groups.values()).map((line) =>
    bucket(line, {
      modelId: line.modelId,
      providerId: null
    })
  )
}

function groupLines(lines: readonly OrgUsageLine[], keyOf: (line: OrgUsageLine) => string): Map<string, OrgUsageLine> {
  const groups = new Map<string, OrgUsageLine>()
  for (const line of lines) {
    const key = keyOf(line)
    const current = groups.get(key)
    groups.set(
      key,
      current
        ? {
            ...current,
            tokens: current.tokens + line.tokens,
            requests: current.requests + line.requests,
            succeeded: current.succeeded + line.succeeded,
            failed: current.failed + line.failed
          }
        : { ...line }
    )
  }
  return groups
}

function toEntries(lines: readonly OrgUsageLine[], names: ReadonlyMap<string, string>): OrgUsageEntry[] {
  return lines.map((line) => ({
    id: `${line.day}:${line.modelId}:${line.endpoint}`,
    modelId: line.modelId,
    modelName: names.get(line.modelId) || line.modelId,
    endpoint: line.endpoint,
    day: line.day,
    tokens: line.tokens,
    requests: line.requests,
    succeeded: line.succeeded,
    failed: line.failed
  }))
}
