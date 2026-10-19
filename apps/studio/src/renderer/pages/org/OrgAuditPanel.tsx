import { SearchSelect } from '@a3s-lab/ui'
import { Download, RefreshCw, ScrollText } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWRInfinite from 'swr/infinite'

import { PageEntry } from '@renderer/components/layout/PageEntry'
import { MemberOrgError } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import {
  AUDIT_KINDS,
  AUDIT_WINDOWS,
  auditCsv,
  isPlanId,
  listTeamAudit,
  type AuditEntry,
  type AuditFilter,
  type AuditKind,
  type AuditPage,
  type AuditValue,
  type AuditWindow
} from './orgAdminApi'
import { OrgAdminFrame } from './OrgAdminFrame'
import type { OrgMemberRow } from './orgApi'
import { ghostIconButtonClassName, quietButtonClassName } from './orgUi'

const DASH = '—'
const MEMBER_STATUSES = new Set(['active', 'invited', 'disabled', 'resigned'])
const COLUMNS = ['time', 'actor', 'action', 'target', 'change'] as const
const WIDTHS = ['15%', '15%', '18%', '24%', '28%']
const INITIAL_FILTER: AuditFilter = { kind: null, actor: null, days: 30 }

type AuditKey = readonly ['studio/org-audit', string, AuditKind | null, string | null, AuditWindow, AuditPage['next']]

function pad(value: number) {
  return String(value).padStart(2, '0')
}

/** 本地时间，精确到分钟。 */
export function auditTime(iso: string): string {
  const at = new Date(iso)
  if (Number.isNaN(at.getTime())) return DASH
  return `${at.getFullYear()}-${pad(at.getMonth() + 1)}-${pad(at.getDate())} ${pad(at.getHours())}:${pad(at.getMinutes())}`
}

function useAuditText() {
  const { t } = useTranslation()
  const value = (row: AuditValue): string => {
    switch (row.kind) {
      case 'status':
        return row.code && MEMBER_STATUSES.has(row.code) ? t(`org.status.${row.code}`) : (row.code ?? DASH)
      case 'plan': {
        const plan = isPlanId(row.code) ? t(`org.plan.name.${row.code}`) : (row.code ?? DASH)
        return t('org.audit.plan', { plan, count: row.seats ?? 0 })
      }
      case 'capabilities':
        return row.code === 'restricted'
          ? t('org.audit.capabilitiesRestricted', { count: row.count ?? 0 })
          : t('org.audit.capabilitiesOpen')
      default:
        return row.label ?? DASH
    }
  }
  return {
    actor: (entry: AuditEntry) =>
      entry.actor.kind === 'operator' ? t('org.audit.operator') : (entry.actor.name ?? t('org.audit.formerMember')),
    action: (entry: AuditEntry) =>
      t(`org.audit.action.${entry.action.replace(/\./g, '_')}`, { defaultValue: entry.action }),
    target: (entry: AuditEntry): string => {
      const target = entry.target
      switch (target.kind) {
        case 'member':
        case 'applicant':
          return target.name ?? t('org.audit.formerMember')
        case 'role':
          return target.name ?? DASH
        case 'invitation':
          return target.phone ?? t('org.audit.joinLink')
        case 'organization':
          return t('org.audit.team')
        case 'repository':
          return target.change_request === null
            ? (target.repository ?? DASH)
            : `${target.repository ?? DASH} #${target.change_request}`
      }
    },
    change: (entry: AuditEntry): string => {
      if (entry.from && entry.to) return `${value(entry.from)} → ${value(entry.to)}`
      const only = entry.to ?? entry.from
      return only ? value(only) : DASH
    }
  }
}

/** Resolves to the saved path, or null when the person dismisses the folder panel. */
function saveCsv(entries: readonly AuditEntry[], orgId: string): Promise<string | null> {
  const stamp = new Date().toISOString().slice(0, 10)
  // The byte-order mark lets spreadsheet apps read the Chinese names as UTF-8.
  return window.api.file.save(`team-audit-${orgId.slice(0, 8)}-${stamp}.csv`, `\uFEFF${auditCsv(entries)}`, {
    filters: [{ name: 'CSV', extensions: ['csv'] }]
  })
}

/** 团队审计日志：只读，新的在前，按游标加载更早的记录。 */
export function OrgAuditPanel({
  orgId,
  members
}: {
  orgId: string
  members: readonly Pick<OrgMemberRow, 'member_id' | 'name'>[]
}) {
  const { t } = useTranslation()
  const [filter, onFilterChange] = useState<AuditFilter>(INITIAL_FILTER)
  const text = useAuditText()
  const { data, error, isValidating, size, setSize, mutate } = useSWRInfinite<
    AuditPage,
    unknown,
    (index: number, previous: AuditPage | null) => AuditKey | null
  >(
    (index, previous) =>
      index > 0 && !previous?.next
        ? null
        : ['studio/org-audit', orgId, filter.kind, filter.actor, filter.days, previous?.next ?? null],
    ([, org, kind, actor, days, cursor]) => listTeamAudit(org, { kind, actor, days }, cursor),
    // Page one is re-read on every visit, filter switch and load; later pages follow its cursor,
    // so a newer first page moves every cached page after it.
    { revalidateOnFocus: false }
  )
  const entries = useMemo(() => (data ?? []).flatMap((page) => page.entries), [data])
  const hasMore = !!data && data[data.length - 1]?.next !== null
  const loadingMore = isValidating && size > (data?.length ?? 0)
  const refreshing = isValidating && !loadingMore
  const denied = error instanceof MemberOrgError && (error.status === 404 || error.status === 403)
  const filtered = filter.kind !== null || filter.actor !== null
  const clearFilters = () => onFilterChange({ ...filter, kind: null, actor: null })
  const actorOptions = useMemo(
    () => members.flatMap((row) => (row.member_id ? [{ value: row.member_id, label: row.name || row.member_id }] : [])),
    [members]
  )

  return (
    <OrgAdminFrame
      ui="org.audit"
      title={t('org.nav.audit')}
      description={t('org.audit.description')}
      actions={
        <button
          type="button"
          data-ui="org.audit.export"
          disabled={entries.length === 0}
          className={quietButtonClassName(30)}
          onClick={() => {
            const count = entries.length
            saveCsv(entries, orgId).then(
              (saved) => {
                if (saved) toast.success(t('org.audit.exported', { count }))
              },
              () => toast.error(t('org.audit.exportFailed'))
            )
          }}>
          <Download size={16} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
          {t('org.audit.export')}
        </button>
      }>
      <div data-ui="org.audit.filters" className="mb-3 flex flex-wrap items-center gap-2">
        <SearchSelect
          size="sm"
          searchable={false}
          aria-label={t('org.audit.filter.kind')}
          placeholder={t('org.audit.filter.kindAll')}
          className="w-36"
          value={filter.kind ?? ''}
          options={AUDIT_KINDS.map((kind) => ({ value: kind, label: t(`org.audit.kind.${kind}`) }))}
          onChange={(next) =>
            onFilterChange({
              ...filter,
              kind: (AUDIT_KINDS as readonly string[]).includes(next) ? (next as AuditKind) : null
            })
          }
        />
        <SearchSelect
          size="sm"
          aria-label={t('org.audit.filter.actor')}
          placeholder={t('org.audit.filter.actorAll')}
          searchPlaceholder={t('org.audit.filter.actorSearch')}
          emptyText={t('org.audit.filter.actorNone')}
          className="w-44"
          value={filter.actor ?? ''}
          options={actorOptions}
          onChange={(next) => onFilterChange({ ...filter, actor: next || null })}
        />
        <SearchSelect
          size="sm"
          searchable={false}
          allowClear={false}
          aria-label={t('org.audit.filter.days')}
          className="w-32"
          value={String(filter.days)}
          options={AUDIT_WINDOWS.map((days) => ({
            value: String(days),
            label: t('org.audit.window', { count: days })
          }))}
          onChange={(next) => {
            const days = Number(next)
            if ((AUDIT_WINDOWS as readonly number[]).includes(days)) {
              onFilterChange({ ...filter, days: days as AuditWindow })
            }
          }}
        />
        {filtered ? (
          <button
            type="button"
            data-ui="org.audit.clear"
            className="h-[30px] rounded-[6px] px-2 text-[12px] text-[#5f5f5f] hover:bg-black/[0.04] hover:text-[#1f1f1f]"
            onClick={clearFilters}>
            {t('org.audit.clearFilters')}
          </button>
        ) : null}
        {data ? (
          <span data-ui="org.audit.count" className="ml-auto text-[12px] text-[#8a8a8a]">
            {t('org.audit.loaded', { count: entries.length })}
          </span>
        ) : null}
        <button
          type="button"
          data-ui="org.audit.refresh"
          aria-label={t('common.refresh')}
          title={t('common.refresh')}
          aria-busy={refreshing || undefined}
          disabled={refreshing}
          onClick={() => void mutate()}
          className={cn(ghostIconButtonClassName, !data && 'ml-auto')}>
          <RefreshCw
            className={cn('size-4 text-current', refreshing && 'animate-spin')}
            style={refreshing ? { animationDuration: '0.7s' } : undefined}
            strokeWidth={1.7}
            aria-hidden
          />
        </button>
      </div>
      {error && !data ? (
        <div data-ui="org.audit.error" className="flex flex-1 flex-col items-center justify-center gap-3 py-16">
          <p className="m-0 max-w-sm text-center text-[13px] text-[#8a8a8a]">
            {t(denied ? 'org.audit.denied' : 'org.error.generic')}
          </p>
          {denied ? null : (
            <button type="button" onClick={() => void mutate()} className={quietButtonClassName(30)}>
              {t('common.retry')}
            </button>
          )}
        </div>
      ) : !data ? (
        <div className="relative min-h-40 flex-1">
          <PageEntry id="org.audit" ready={false} />
        </div>
      ) : entries.length === 0 ? (
        <div
          data-ui="org.audit.empty"
          className="flex flex-1 flex-col items-center justify-center gap-2 py-16 text-center">
          <ScrollText className="size-6 text-[#8a8a8a]" strokeWidth={1.5} aria-hidden />
          <p className="m-0 text-[13px] text-[#3f3f3f]">
            {t(filtered ? 'org.audit.emptyFiltered' : 'org.audit.empty')}
          </p>
          {filtered ? (
            <button type="button" className={quietButtonClassName(30)} onClick={clearFilters}>
              {t('org.audit.clearFilters')}
            </button>
          ) : null}
        </div>
      ) : (
        <>
          <table
            data-ui="org.audit-table"
            aria-label={t('org.nav.audit')}
            className="w-full min-w-[760px] table-fixed border-collapse text-left">
            <colgroup>
              {WIDTHS.map((width, index) => (
                <col key={COLUMNS[index]} style={{ width }} />
              ))}
            </colgroup>
            <thead className="sticky top-0 z-[1] bg-white">
              <tr className="border-b border-[#efefef] text-[12px] text-[#8a8a8a]">
                {COLUMNS.map((column, index) => (
                  <th key={column} scope="col" className={cn('h-9 font-normal', index === 0 && 'pl-2.5')}>
                    {t(`org.audit.col.${column}`)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {entries.map((entry) => (
                <tr key={entry.id} data-action={entry.action} className="hover:bg-[#f7f7f7]">
                  <td className="h-11 pl-2.5 text-[13px] whitespace-nowrap text-[#8a8a8a]">
                    <time dateTime={entry.occurred_at}>{auditTime(entry.occurred_at)}</time>
                  </td>
                  <td
                    className={cn(
                      'h-11 truncate text-[13px]',
                      entry.actor.kind === 'operator' ? 'text-[#8a8a8a]' : 'text-[#1a1a1a]'
                    )}>
                    {text.actor(entry)}
                  </td>
                  <td className="h-11 truncate text-[13px] text-[#1a1a1a]">{text.action(entry)}</td>
                  <td className="h-11 truncate text-[13px] text-[#3f3f3f]">{text.target(entry)}</td>
                  <td className="h-11 truncate pr-2.5 text-[13px] text-[#3f3f3f]">{text.change(entry)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="flex justify-center py-4">
            {hasMore ? (
              <button
                type="button"
                data-ui="org.audit.more"
                disabled={loadingMore}
                className={quietButtonClassName(30)}
                onClick={() => void setSize(size + 1)}>
                {t(loadingMore ? 'org.audit.loading' : 'org.audit.more')}
              </button>
            ) : (
              <span data-ui="org.audit.end" className="text-[12px] text-[#8a8a8a]">
                {t('org.audit.end')}
              </span>
            )}
          </div>
          {error ? <p className="m-0 text-center text-[12px] text-[#8a8a8a]">{t('org.error.generic')}</p> : null}
        </>
      )}
    </OrgAdminFrame>
  )
}
