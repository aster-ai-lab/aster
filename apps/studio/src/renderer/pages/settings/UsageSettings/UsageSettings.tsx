import {
  Button,
  ButtonGroup,
  EmptyState,
  SegmentedControl,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@a3s-lab/ui'
import { ChartColumn, ChartLine, ChartPie, type LucideIcon } from 'lucide-react'
import { memo, useCallback, useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { usePersistCache } from '@data/hooks/useCache'
import { formatCompactNumber } from '@renderer/utils/number'
import { cn } from '@renderer/utils/style'
import type { AiUsageRecordGroupIdentity, AiUsageRecordStatsBucket } from '@shared/data/api/schemas/aiUsageRecords'

import type { OrgUsageScope } from './orgUsage'
import {
  CHART_TYPE_KEYS,
  CHART_TYPE_LABEL_KEYS,
  displayModelId,
  getLongestStreak,
  getRatioChange,
  getTimelineSeries,
  getWindowRange,
  GROUP_BY_KEYS,
  GROUP_BY_LABEL_KEYS,
  METRIC_KEYS,
  METRIC_LABEL_KEYS,
  ROLLUP_LABEL_KEYS,
  TOP_COUNT_KEYS,
  TREND_ROLLUP_KEYS,
  type UsageChartType,
  type UsageTopCount,
  WINDOW_KEYS,
  WINDOW_LABEL_KEYS
} from './usageAnalytics'
import { parseDateKey } from './usageDisplay'
import { UsageDistributionChart } from './UsageDistributionChart'
import { UsageEntriesTable } from './UsageEntriesTable'
import UsageHeatmap from './UsageHeatmap'
import {
  InsightCell,
  MetricCell,
  MetricStripSkeleton,
  UsageControlRow,
  UsageModelLabel,
  UsagePanel,
  UsagePanelHeader,
  UsageResponsiveShell,
  UsageSection,
  UsageSectionHeader,
  UsageSectionTitle
} from './UsageSettingsPrimitives'
import { type OrgUsageEntry, type OrgUsageSortBy, useUsageData } from './useUsageData'

const ENTRY_PAGE_SIZE = 25
const EMPTY_MODEL_NAMES = new Map<string, string>()

const CHART_TYPE_ICONS: Record<UsageChartType, LucideIcon> = {
  bar: ChartColumn,
  line: ChartLine,
  pie: ChartPie
}

const UsageEntriesSection = memo(function UsageEntriesSection({
  entries,
  isLoading,
  dateFormatter
}: {
  entries: OrgUsageEntry[]
  isLoading: boolean
  dateFormatter: Intl.DateTimeFormat
}) {
  const [sortBy, setSortBy] = usePersistCache('settings.usage.entry_sort_by')
  const [sortOrder, setSortOrder] = usePersistCache('settings.usage.entry_sort_order')
  const activeSort: OrgUsageSortBy =
    sortBy === 'totalTokens' || sortBy === 'requests' || sortBy === 'succeeded' || sortBy === 'failed'
      ? sortBy
      : 'createdAt'
  const [visible, setVisible] = useState(ENTRY_PAGE_SIZE)
  useEffect(() => {
    setVisible(ENTRY_PAGE_SIZE)
  }, [activeSort, entries, sortOrder])
  const sorted = useMemo(() => sortUsageEntries(entries, activeSort, sortOrder), [activeSort, entries, sortOrder])
  const page = sorted.slice(0, visible)
  const handleSort = (nextSortBy: OrgUsageSortBy) => {
    setSortOrder((currentOrder) => (activeSort === nextSortBy && currentOrder === 'desc' ? 'asc' : 'desc'))
    setSortBy(nextSortBy)
  }

  return (
    <UsageEntriesTable
      entries={page}
      entryTotal={sorted.length}
      isLoading={isLoading}
      isRefreshing={false}
      hasNextPage={visible < sorted.length}
      sortBy={activeSort}
      sortOrder={sortOrder}
      onSort={handleSort}
      onLoadNext={() => setVisible((current) => current + ENTRY_PAGE_SIZE)}
      dateFormatter={dateFormatter}
    />
  )
})

function sortUsageEntries(
  entries: OrgUsageEntry[],
  sortBy: OrgUsageSortBy,
  sortOrder: 'asc' | 'desc'
): OrgUsageEntry[] {
  const direction = sortOrder === 'asc' ? 1 : -1
  const valueOf = (entry: OrgUsageEntry) => {
    if (sortBy === 'totalTokens') return entry.tokens
    if (sortBy === 'requests') return entry.requests
    if (sortBy === 'succeeded') return entry.succeeded
    if (sortBy === 'failed') return entry.failed
    return entry.day
  }
  return entries.toSorted((left, right) => {
    const leftValue = valueOf(left)
    const rightValue = valueOf(right)
    if (leftValue < rightValue) return -1 * direction
    if (leftValue > rightValue) return 1 * direction
    return left.id < right.id ? -1 : left.id > right.id ? 1 : 0
  })
}

function UsageSettings({ scope }: { scope?: OrgUsageScope }) {
  const { t, i18n } = useTranslation()
  const [persistedWindow, setWindowKey] = usePersistCache('settings.usage.window')
  const [persistedGroupBy, setGroupBy] = usePersistCache('settings.usage.group_by')
  const [persistedMetric, setChartMetric] = usePersistCache('settings.usage.chart_metric')
  const [selectedChartType, setSelectedChartType] = usePersistCache('settings.usage.chart_type')
  const [persistedRollup, setRollup] = usePersistCache('settings.usage.rollup')
  const [topCount, setTopCount] = usePersistCache('settings.usage.top_count')
  const windowKey = WINDOW_KEYS.find((key) => key === persistedWindow) ?? '30d'
  const groupBy = persistedGroupBy === 'endpoint' ? 'endpoint' : 'model'
  const chartMetric = persistedMetric === 'requests' ? 'requests' : 'tokens'
  const chartType: UsageChartType =
    selectedChartType === 'line' || selectedChartType === 'pie' ? selectedChartType : 'bar'
  const rollup = persistedRollup === 'total' ? 'daily' : persistedRollup

  const windowRange = useMemo(() => getWindowRange(windowKey), [windowKey])

  const {
    timelineBuckets,
    overviewBuckets,
    exploreTimelineRows,
    overviewTotals,
    previousOverviewTotals,
    hasPrevious = false,
    entries = [],
    modelNames = EMPTY_MODEL_NAMES,
    timelineLoading,
    overviewLoading,
    exploreTimelineLoading,
    needsOrg = false,
    error = null
  } = useUsageData({
    windowKey,
    groupBy,
    scope
  })

  const activeDateKeys = useMemo(
    () => timelineBuckets.filter((bucket) => bucket.requestCount > 0).map((bucket) => bucket.date),
    [timelineBuckets]
  )
  const totalTokens = overviewTotals.totalTokens
  const totalRequests = overviewTotals.requestCount
  const previousTotalTokens = hasPrevious ? previousOverviewTotals.totalTokens : undefined
  const previousTotalRequests = hasPrevious ? previousOverviewTotals.requestCount : undefined
  const succeeded = overviewTotals.succeeded
  const failed = overviewTotals.failed
  const previousSucceeded = hasPrevious ? previousOverviewTotals.succeeded : undefined
  const previousFailed = hasPrevious ? previousOverviewTotals.failed : undefined
  const activeDays = activeDateKeys.length
  const longestStreak = useMemo(() => getLongestStreak(activeDateKeys), [activeDateKeys])
  const succeededTrendValues = useMemo(
    () => getTimelineSeries(timelineBuckets, windowRange, (bucket) => bucket.estimatedRequestCount),
    [timelineBuckets, windowRange]
  )
  const failedTrendValues = useMemo(
    () => getTimelineSeries(timelineBuckets, windowRange, (bucket) => bucket.unpricedRequestCount),
    [timelineBuckets, windowRange]
  )
  const requestTrendValues = useMemo(
    () => getTimelineSeries(timelineBuckets, windowRange, (bucket) => bucket.requestCount),
    [timelineBuckets, windowRange]
  )
  const tokenTrendValues = useMemo(
    () => getTimelineSeries(timelineBuckets, windowRange, (bucket) => bucket.totalTokens),
    [timelineBuckets, windowRange]
  )
  const peakDay = useMemo(
    () =>
      timelineBuckets.reduce<(typeof timelineBuckets)[number] | undefined>(
        (best, bucket) => (!best || bucket.totalTokens > best.totalTokens ? bucket : best),
        undefined
      ),
    [timelineBuckets]
  )
  const topModel = useMemo(
    () =>
      overviewBuckets
        .filter(
          (bucket): bucket is Extract<AiUsageRecordStatsBucket, { groupBy: 'model' }> => bucket.groupBy === 'model'
        )
        .reduce<Extract<AiUsageRecordStatsBucket, { groupBy: 'model' }> | undefined>(
          (best, bucket) => (!best || bucket.totalTokens > best.totalTokens ? bucket : best),
          undefined
        ),
    [overviewBuckets]
  )

  const dateFormatter = useMemo(
    () => new Intl.DateTimeFormat(i18n.language, { year: 'numeric', month: 'short', day: 'numeric' }),
    [i18n.language]
  )
  const monthFormatter = useMemo(
    () => new Intl.DateTimeFormat(i18n.language, { year: 'numeric', month: 'short' }),
    [i18n.language]
  )
  const percentFormatter = useMemo(
    () =>
      new Intl.NumberFormat(i18n.language, {
        style: 'percent',
        maximumFractionDigits: 1,
        signDisplay: 'exceptZero'
      }),
    [i18n.language]
  )
  const formatDelta = useCallback((value: number) => percentFormatter.format(value), [percentFormatter])
  const entryDateFormatter = dateFormatter

  const windowOptions = useMemo(
    () =>
      WINDOW_KEYS.map((value) => ({
        value,
        label: t(WINDOW_LABEL_KEYS[value])
      })),
    [t]
  )
  const groupByOptions = useMemo(
    () =>
      GROUP_BY_KEYS.map((value) => ({
        value,
        label: t(GROUP_BY_LABEL_KEYS[value])
      })),
    [t]
  )
  const metricOptions = useMemo(
    () =>
      METRIC_KEYS.map((value) => ({
        value,
        label: t(METRIC_LABEL_KEYS[value])
      })),
    [t]
  )
  const rollupOptions = useMemo(
    () =>
      TREND_ROLLUP_KEYS.map((value) => ({
        value,
        label: t(ROLLUP_LABEL_KEYS[value])
      })),
    [t]
  )
  const topCountOptions = useMemo(
    () => TOP_COUNT_KEYS.map((value) => ({ value: String(value), label: String(value) })),
    []
  )
  const chartTypeOptions = useMemo(
    () => CHART_TYPE_KEYS.map((value) => ({ value, label: t(CHART_TYPE_LABEL_KEYS[value]) })),
    [t]
  )

  const hasUsage = totalRequests > 0 || timelineBuckets.some((bucket) => bucket.requestCount > 0)
  // Explore refetches must not blank the window-scoped cards;
  // the distribution chart renders its own skeleton.
  const isInitialLoading = timelineLoading || overviewLoading
  const quiet = needsOrg || error !== null
  const totalExploreMetric = chartMetric === 'requests' ? totalRequests : totalTokens

  const modelLabel = (modelId: string | null | undefined) => {
    if (!modelId) return t('settings.usage.cards.none')
    return modelNames.get(modelId) || displayModelId(modelId) || t('settings.usage.cards.none')
  }
  const getBucketLabel = (bucket: AiUsageRecordGroupIdentity): string => {
    if (groupBy === 'endpoint')
      return bucket.sourceName || bucket.sourceId || t('settings.usage.cards.unattributedSource')
    return modelLabel(bucket.modelId)
  }

  const formatChartValue = (value: number) => formatCompactNumber(value)

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <UsageResponsiveShell>
        <div className="flex min-w-0 flex-col gap-3 @[640px]/usage:flex-row @[640px]/usage:items-start @[640px]/usage:justify-between">
          <div className="min-w-0">
            <UsageSectionTitle>{t('settings.usage.overview.title')}</UsageSectionTitle>
            <p className="mt-1 text-sm text-muted-foreground">
              {quiet
                ? t(WINDOW_LABEL_KEYS[windowKey])
                : t('settings.usage.summary', {
                    window: t(WINDOW_LABEL_KEYS[windowKey]),
                    tokens: formatCompactNumber(totalTokens),
                    requests: formatCompactNumber(totalRequests)
                  })}
            </p>
          </div>
          <div className="flex min-w-0 flex-wrap items-center gap-2">
            <SegmentedControl options={windowOptions} value={windowKey} onValueChange={setWindowKey} size="sm" />
          </div>
        </div>

        <UsageSection>
          <UsagePanel>
            {isInitialLoading ? (
              <MetricStripSkeleton />
            ) : quiet ? null : (
              <div
                className={cn(
                  'grid min-w-0 grid-cols-1 gap-px bg-border @[560px]/usage:grid-cols-2 @[900px]/usage:grid-cols-4',
                  hasUsage && 'border-b border-border'
                )}>
                <MetricCell
                  label={t('settings.usage.cards.succeeded')}
                  trendValues={succeededTrendValues}
                  delta={getRatioChange(succeeded, previousSucceeded)}
                  deltaLabel={t('settings.usage.cards.lastPeriod')}
                  formatDelta={formatDelta}
                  value={succeeded === undefined ? t('settings.usage.cards.none') : formatCompactNumber(succeeded)}
                />
                <MetricCell
                  label={t('settings.usage.cards.totalRequests')}
                  trendValues={requestTrendValues}
                  delta={getRatioChange(totalRequests, previousTotalRequests)}
                  deltaLabel={t('settings.usage.cards.lastPeriod')}
                  formatDelta={formatDelta}
                  value={formatCompactNumber(totalRequests)}
                />
                <MetricCell
                  label={t('settings.usage.cards.totalTokens')}
                  trendValues={tokenTrendValues}
                  delta={getRatioChange(totalTokens, previousTotalTokens)}
                  deltaLabel={t('settings.usage.cards.lastPeriod')}
                  formatDelta={formatDelta}
                  value={formatCompactNumber(totalTokens)}
                />
                <MetricCell
                  label={t('settings.usage.cards.failed')}
                  trendValues={failedTrendValues}
                  delta={getRatioChange(failed, previousFailed)}
                  deltaLabel={t('settings.usage.cards.lastPeriod')}
                  formatDelta={formatDelta}
                  value={failed === undefined ? t('settings.usage.cards.none') : formatCompactNumber(failed)}
                />
              </div>
            )}

            {!isInitialLoading && hasUsage && (
              <div className="grid min-w-0 grid-cols-1 gap-px bg-border @[560px]/usage:grid-cols-2 @[900px]/usage:grid-cols-4">
                <InsightCell
                  label={t('settings.usage.cards.activeDays')}
                  value={activeDays}
                  helper={t('settings.usage.cards.streak', { days: longestStreak })}
                />
                <InsightCell
                  label={t('settings.usage.cards.peakDay')}
                  value={peakDay ? formatCompactNumber(peakDay.totalTokens) : t('settings.usage.cards.none')}
                  helper={peakDay ? dateFormatter.format(parseDateKey(peakDay.date)) : undefined}
                />
                <InsightCell
                  label={t('settings.usage.cards.topModel')}
                  value={
                    topModel?.modelId ? (
                      <UsageModelLabel modelId={topModel.modelId} providerId={topModel.providerId ?? ''} size={16}>
                        {modelLabel(topModel.modelId)}
                      </UsageModelLabel>
                    ) : (
                      t('settings.usage.cards.none')
                    )
                  }
                  helper={topModel ? formatCompactNumber(topModel.totalTokens) : undefined}
                />
                <InsightCell
                  label={t('settings.usage.cards.dailyAverage')}
                  value={formatCompactNumber(activeDays > 0 ? totalTokens / activeDays : 0)}
                  helper={t('settings.usage.tooltip.requests', { count: totalRequests })}
                />
              </div>
            )}
          </UsagePanel>

          {hasUsage && (
            <UsagePanel>
              <UsageHeatmap buckets={timelineBuckets} isLoading={timelineLoading} range={windowRange} />
            </UsagePanel>
          )}

          {!hasUsage && !isInitialLoading && (
            <UsagePanel className="p-3 @[640px]/usage:p-4">
              <EmptyState
                compact
                preset="no-result"
                title={t(
                  needsOrg
                    ? 'settings.usage.orgRequired'
                    : error
                      ? 'settings.usage.loadFailed'
                      : 'settings.usage.empty.title'
                )}
                description={
                  needsOrg
                    ? t('settings.usage.orgRequired.description')
                    : error && error !== 'load'
                      ? error
                      : t('settings.usage.empty.description')
                }
              />
            </UsagePanel>
          )}
        </UsageSection>

        {!quiet && (
          <UsageSection className={cn(!hasUsage && 'hidden')}>
            <UsageSectionHeader>
              <div className="min-w-0">
                <UsageSectionTitle>{t('settings.usage.explore.analysis')}</UsageSectionTitle>
                <div className="mt-1 flex min-w-0 flex-wrap items-center gap-1 text-xs text-foreground-tertiary">
                  <span>{t(WINDOW_LABEL_KEYS[windowKey])}</span>
                  <span className="ml-1 text-muted-foreground">· {formatChartValue(totalExploreMetric)}</span>
                </div>
              </div>
            </UsageSectionHeader>

            <div className="flex min-w-0 flex-col gap-3">
              <UsagePanel>
                <UsagePanelHeader className="flex min-w-0 flex-wrap items-end gap-3">
                  <UsageControlRow className="w-40" label={t('settings.usage.explore.groupBy')}>
                    <Select value={groupBy} onValueChange={(value) => setGroupBy(value as typeof groupBy)}>
                      <SelectTrigger size="sm" className="w-full" aria-label={t('settings.usage.explore.groupBy')}>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {groupByOptions.map((option) => (
                          <SelectItem key={option.value} value={option.value}>
                            {option.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </UsageControlRow>
                  <UsageControlRow className="w-32" label={t('settings.usage.explore.metric')}>
                    <Select value={chartMetric} onValueChange={(value) => setChartMetric(value as typeof chartMetric)}>
                      <SelectTrigger size="sm" className="w-full" aria-label={t('settings.usage.explore.metric')}>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {metricOptions.map((option) => (
                          <SelectItem key={option.value} value={option.value}>
                            {option.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </UsageControlRow>
                  <UsageControlRow className="w-24" label={t('settings.usage.explore.top')}>
                    <Select
                      value={String(topCount)}
                      onValueChange={(value) => setTopCount(Number(value) as UsageTopCount)}>
                      <SelectTrigger size="sm" className="w-full" aria-label={t('settings.usage.explore.top')}>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {topCountOptions.map((option) => (
                          <SelectItem key={option.value} value={option.value}>
                            {option.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </UsageControlRow>
                  <UsageControlRow className="@[760px]/usage:ml-auto" label={t('settings.usage.explore.chart')}>
                    <ButtonGroup attached={false} aria-label={t('settings.usage.explore.chart')}>
                      {chartTypeOptions.map((option) => {
                        const Icon = CHART_TYPE_ICONS[option.value]
                        const isActive = chartType === option.value

                        return (
                          <Button
                            key={option.value}
                            type="button"
                            variant={isActive ? 'secondary' : 'ghost'}
                            size="sm"
                            aria-pressed={isActive}
                            onClick={() => setSelectedChartType(option.value)}>
                            <Icon className="size-3.5" />
                            {option.label}
                          </Button>
                        )
                      })}
                    </ButtonGroup>
                  </UsageControlRow>
                  <UsageControlRow label={t('settings.usage.explore.granularity')}>
                    <SegmentedControl options={rollupOptions} value={rollup} onValueChange={setRollup} size="sm" />
                  </UsageControlRow>
                </UsagePanelHeader>
                <UsageDistributionChart
                  range={windowRange}
                  timelineBuckets={timelineBuckets}
                  exploreTimelineRows={exploreTimelineRows}
                  rollup={rollup}
                  chartMetric={chartMetric}
                  chartType={chartType}
                  topCount={topCount}
                  exploreTimelineLoading={exploreTimelineLoading}
                  dateFormatter={dateFormatter}
                  monthFormatter={monthFormatter}
                  getBucketLabel={getBucketLabel}
                />
              </UsagePanel>

              <UsageEntriesSection entries={entries} isLoading={isInitialLoading} dateFormatter={entryDateFormatter} />
            </div>
          </UsageSection>
        )}
      </UsageResponsiveShell>
    </div>
  )
}

export default UsageSettings
