import {
  Button,
  EmptyState,
  Skeleton,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from '@a3s-lab/ui'
import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { formatCompactNumber } from '@renderer/utils/number'
import { cn } from '@renderer/utils/style'

import { parseDateKey } from './usageDisplay'
import { UsageModelAvatar, UsagePanel, UsagePanelHeader, UsagePanelTitle } from './UsageSettingsPrimitives'
import type { OrgUsageEntry, OrgUsageSortBy } from './useUsageData'

const EMPTY_VALUE = '-'

interface UsageEntriesTableProps {
  entries: OrgUsageEntry[]
  entryTotal: number
  isLoading: boolean
  isRefreshing: boolean
  hasNextPage: boolean
  sortBy: OrgUsageSortBy
  sortOrder: 'asc' | 'desc'
  onSort: (sortBy: OrgUsageSortBy) => void
  onLoadNext: () => void
  dateFormatter: Intl.DateTimeFormat
}

export function UsageEntriesTable({
  entries,
  entryTotal,
  isLoading,
  isRefreshing,
  hasNextPage,
  sortBy,
  sortOrder,
  onSort,
  onLoadNext,
  dateFormatter
}: UsageEntriesTableProps) {
  const { t } = useTranslation()
  const isInitialLoading = isLoading && entries.length === 0
  const getAriaSort = (column: OrgUsageSortBy) =>
    sortBy === column ? (sortOrder === 'asc' ? 'ascending' : 'descending') : 'none'
  const renderSortHeader = (column: OrgUsageSortBy, label: string, align: 'left' | 'right' = 'left') => {
    const isActive = sortBy === column
    const Icon = isActive ? (sortOrder === 'asc' ? ArrowUp : ArrowDown) : ArrowUpDown

    return (
      <Button
        variant="ghost"
        size="sm"
        className={cn(
          '-mx-2 h-7 gap-1.5 px-2 font-medium text-muted-foreground hover:text-foreground',
          align === 'right' && 'ml-auto'
        )}
        onClick={() => onSort(column)}>
        <span>{label}</span>
        <Icon className="size-3.5" />
      </Button>
    )
  }
  return (
    <UsagePanel>
      <UsagePanelHeader className="flex min-w-0 items-center justify-between gap-3">
        <UsagePanelTitle>{t('settings.usage.explore.entries')}</UsagePanelTitle>
        <div className="text-xs text-foreground-tertiary">
          {t('settings.usage.explore.totalEntries', { count: entryTotal })}
        </div>
      </UsagePanelHeader>
      <div className="min-w-0 p-3" aria-busy={isLoading || isRefreshing}>
        {isInitialLoading ? (
          <div className="flex flex-col gap-2">
            {Array.from({ length: 6 }, (_, index) => (
              <Skeleton key={index} className="h-9 rounded-md" />
            ))}
          </div>
        ) : entries.length > 0 ? (
          <>
            <Table className="min-w-[880px] table-fixed">
              <colgroup>
                <col className="w-[28%]" />
                <col className="w-[22%]" />
                <col className="w-[16%]" />
                <col className="w-[10%]" />
                <col className="w-[8%]" />
                <col className="w-[8%]" />
                <col className="w-[8%]" />
              </colgroup>
              <TableHeader>
                <TableRow>
                  <TableHead>{t('settings.usage.table.model')}</TableHead>
                  <TableHead>{t('settings.usage.table.source')}</TableHead>
                  <TableHead aria-sort={getAriaSort('createdAt')}>
                    {renderSortHeader('createdAt', t('settings.usage.table.date'))}
                  </TableHead>
                  <TableHead className="text-right" aria-sort={getAriaSort('totalTokens')}>
                    {renderSortHeader('totalTokens', t('settings.usage.table.tokens'), 'right')}
                  </TableHead>
                  <TableHead className="text-right" aria-sort={getAriaSort('requests')}>
                    {renderSortHeader('requests', t('settings.usage.table.requests'), 'right')}
                  </TableHead>
                  <TableHead className="text-right" aria-sort={getAriaSort('succeeded')}>
                    {renderSortHeader('succeeded', t('settings.usage.table.succeeded'), 'right')}
                  </TableHead>
                  <TableHead className="text-right" aria-sort={getAriaSort('failed')}>
                    {renderSortHeader('failed', t('settings.usage.table.failed'), 'right')}
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {entries.map((entry) => {
                  const modelName = entry.modelName || entry.modelId || EMPTY_VALUE
                  const sourceName = entry.endpoint || t('settings.usage.cards.unattributedSource')
                  const createdAtLabel = dateFormatter.format(parseDateKey(entry.day))

                  return (
                    <TableRow key={entry.id}>
                      <TableCell className="min-w-0">
                        <div className="flex min-w-0 items-center gap-2">
                          <UsageModelAvatar modelId={entry.modelId} providerId="" size={18} />
                          <div
                            className="min-w-0 truncate text-sm leading-5 font-medium text-foreground"
                            title={modelName}>
                            {modelName}
                          </div>
                        </div>
                      </TableCell>
                      <TableCell className="min-w-0">
                        <div className="min-w-0 truncate text-sm text-foreground" title={sourceName}>
                          {sourceName}
                        </div>
                      </TableCell>
                      <TableCell className="min-w-0 text-xs text-muted-foreground">
                        <time
                          dateTime={entry.day}
                          className="block truncate whitespace-nowrap tabular-nums"
                          title={createdAtLabel}>
                          {createdAtLabel}
                        </time>
                      </TableCell>
                      <TableCell className="text-right font-medium whitespace-nowrap tabular-nums">
                        {formatCompactNumber(entry.tokens)}
                      </TableCell>
                      <TableCell className="text-right whitespace-nowrap tabular-nums">
                        {formatCompactNumber(entry.requests)}
                      </TableCell>
                      <TableCell className="text-right whitespace-nowrap tabular-nums">
                        {formatCompactNumber(entry.succeeded)}
                      </TableCell>
                      <TableCell className="text-right whitespace-nowrap tabular-nums">
                        {formatCompactNumber(entry.failed)}
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
            {hasNextPage && (
              <div className="flex justify-center pt-3">
                <Button variant="outline" size="sm" disabled={isRefreshing} onClick={onLoadNext}>
                  {isRefreshing ? t('settings.usage.explore.loading') : t('settings.usage.explore.loadMore')}
                </Button>
              </div>
            )}
          </>
        ) : (
          <EmptyState
            compact
            preset="no-result"
            title={t('settings.usage.explore.noEntries')}
            description={t('settings.usage.explore.noEntriesDescription')}
          />
        )}
      </div>
    </UsagePanel>
  )
}
