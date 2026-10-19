import { Dialog, DialogContent, DialogTitle, PopConfirm, rowMotionClassName, rowMotionStyle } from '@a3s-lab/ui'
import { CirclePlay, RefreshCw, ScrollText, Square } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWRInfinite from 'swr/infinite'

import { beginOptimistic, useOptimisticRows } from '@renderer/components/collection/optimisticRows'
import { PageEntry } from '@renderer/components/layout/PageEntry'
import { MemberOrgError } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import {
  isRunStatus,
  listTeamRuns,
  runLogPage,
  runStoppable,
  stopTeamRun,
  teamRunLogs,
  type RunLogPage,
  type RunPage,
  type TeamRun
} from './orgAdminApi'
import { OrgAdminFrame } from './OrgAdminFrame'
import { auditTime } from './OrgAuditPanel'
import { ghostIconButtonClassName, quietButtonClassName, rowActionClassName } from './orgUi'

const COLUMNS = ['status', 'starter', 'started', 'conversation', 'actions'] as const
const WIDTHS = ['20%', '20%', '20%', '24%', '16%']
const COLLECTION = 'member/team-runs'
const runId = (row: TeamRun) => row.execution_id

type RunsKey = readonly ['studio/org-runs', string, RunPage['next']]
type LogsKey = readonly ['studio/org-run-logs', string, string, string | null]

const STATUS_TONE: Record<string, string> = {
  pending: 'bg-[#d97706]',
  running: 'bg-[#16a34a]',
  awaiting_approval: 'bg-[#d97706]',
  cancelling: 'bg-[#8a8a8a] animate-pulse',
  succeeded: 'bg-[#171717]',
  failed: 'bg-[#dc2626]',
  cancelled: 'bg-[#c4c4c4]'
}

function gone(err: unknown) {
  return err instanceof MemberOrgError && (err.status === 404 || err.status === 403)
}

function RunStatus({ status }: { status: string | null }) {
  const { t } = useTranslation()
  if (status === null) {
    return <span className="text-[13px] text-[#8a8a8a]">{t('org.runs.status.missing')}</span>
  }
  return (
    <span className="inline-flex items-center gap-2 text-[13px] text-[#1a1a1a]">
      <span aria-hidden className={cn('size-1.5 shrink-0 rounded-full', STATUS_TONE[status] ?? 'bg-[#c4c4c4]')} />
      {isRunStatus(status) ? t(`org.runs.status.${status}`) : status}
    </span>
  )
}

/** 团队的 Cloud 智能体运行：新的在前，按游标加载更早的运行。 */
export function OrgRunsPanel({ orgId, canStop }: { orgId: string; canStop: boolean }) {
  const { t } = useTranslation()
  const { data, error, isValidating, size, setSize, mutate } = useSWRInfinite<
    RunPage,
    unknown,
    (index: number, previous: RunPage | null) => RunsKey | null
  >(
    (index, previous) => (index > 0 && !previous?.next ? null : ['studio/org-runs', orgId, previous?.next ?? null]),
    ([, org, cursor]) => listTeamRuns(org, cursor),
    { revalidateOnFocus: false }
  )
  const runs = useMemo(() => (data ?? []).flatMap((page) => page.runs), [data])
  const optimistic = useOptimisticRows(COLLECTION, runs, runId)
  const hasMore = !!data && data[data.length - 1]?.next !== null
  const loadingMore = isValidating && size > (data?.length ?? 0)
  const refreshing = isValidating && !loadingMore
  const denied = gone(error)
  const [pendingId, setPendingId] = useState<string | null>(null)
  const [logsOf, setLogsOf] = useState<TeamRun | null>(null)
  /** A retried stop reuses its request id, so the owner never stops the same run twice. */
  const stopRequests = useRef(new Map<string, string>())
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  async function stop(run: TeamRun) {
    if (pendingId !== null || !runStoppable(run.status)) return
    setPendingId(run.execution_id)
    const requestId = stopRequests.current.get(run.execution_id) ?? crypto.randomUUID()
    stopRequests.current.set(run.execution_id, requestId)
    const handle = beginOptimistic<TeamRun>(COLLECTION, {
      patch: { [run.execution_id]: (row) => ({ ...row, status: 'cancelling' }) }
    })
    try {
      await stopTeamRun(orgId, run.execution_id, requestId)
      stopRequests.current.delete(run.execution_id)
      handle.confirm()
      await mutate()
      if (mounted.current) toast.success(t('org.runs.stopRequested'))
    } catch (err) {
      handle.rollback()
      if (gone(err)) stopRequests.current.delete(run.execution_id)
      if (mounted.current) toast.error(t(gone(err) ? 'org.runs.stopGone' : 'org.error.generic'))
    } finally {
      handle.release()
      if (mounted.current) setPendingId(null)
    }
  }

  return (
    <OrgAdminFrame
      ui="org.runs"
      title={t('org.nav.runs')}
      description={t('org.runs.description')}
      actions={
        <button
          type="button"
          data-ui="org.runs.refresh"
          aria-label={t('common.refresh')}
          title={t('common.refresh')}
          aria-busy={refreshing || undefined}
          disabled={refreshing}
          onClick={() => void mutate()}
          className={ghostIconButtonClassName}>
          <RefreshCw
            className={cn('size-4 text-current', refreshing && 'animate-spin')}
            style={refreshing ? { animationDuration: '0.7s' } : undefined}
            strokeWidth={1.7}
            aria-hidden
          />
        </button>
      }>
      {error && !data ? (
        <div data-ui="org.runs.error" className="flex flex-1 flex-col items-center justify-center gap-3 py-16">
          <p className="m-0 max-w-sm text-center text-[13px] text-[#8a8a8a]">
            {t(denied ? 'org.runs.denied' : 'org.error.generic')}
          </p>
          {denied ? null : (
            <button type="button" onClick={() => void mutate()} className={quietButtonClassName(30)}>
              {t('common.retry')}
            </button>
          )}
        </div>
      ) : !data ? (
        <div className="relative min-h-40 flex-1">
          <PageEntry id="org.runs" ready={false} />
        </div>
      ) : optimistic.rows.length === 0 ? (
        <div
          data-ui="org.runs.empty"
          className="flex flex-1 flex-col items-center justify-center gap-2 py-16 text-center">
          <CirclePlay className="size-6 text-[#8a8a8a]" strokeWidth={1.5} aria-hidden />
          <p className="m-0 text-[13px] text-[#3f3f3f]">{t('org.runs.empty')}</p>
          <p className="m-0 max-w-sm text-[12px] leading-5 text-[#8a8a8a]">{t('org.runs.emptyHint')}</p>
        </div>
      ) : (
        <>
          <p data-ui="org.runs.count" className="m-0 mb-2 text-right text-[12px] text-[#8a8a8a]">
            {t('org.runs.loaded', { count: optimistic.rows.length })}
          </p>
          <table
            data-ui="org.runs-table"
            aria-label={t('org.nav.runs')}
            className="w-full min-w-[720px] table-fixed border-collapse text-left">
            <colgroup>
              {WIDTHS.map((width, index) => (
                <col key={COLUMNS[index]} style={{ width }} />
              ))}
            </colgroup>
            <thead className="sticky top-0 z-[1] bg-white">
              <tr className="border-b border-[#efefef] text-[12px] text-[#8a8a8a]">
                {COLUMNS.map((column, index) => (
                  <th
                    key={column}
                    scope="col"
                    className={cn(
                      'h-9 font-normal',
                      index === 0 && 'pl-2.5',
                      index === COLUMNS.length - 1 && 'pr-2.5 text-right'
                    )}>
                    {t(`org.runs.col.${column}`)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {optimistic.rows.map((run, index) => {
                const motion = optimistic.motion(run.execution_id)
                const starter = run.started_by.name ?? t('org.runs.formerMember')
                return (
                  <tr
                    key={run.execution_id}
                    data-run={run.execution_id}
                    data-status={run.status ?? 'missing'}
                    data-motion={motion}
                    aria-busy={motion === 'pending' || undefined}
                    style={rowMotionStyle(index, motion)}
                    className={cn('group hover:bg-[#f7f7f7]', rowMotionClassName(motion))}>
                    <td className="h-11 pl-2.5">
                      <RunStatus status={run.status} />
                    </td>
                    <td
                      className={cn(
                        'h-11 truncate text-[13px]',
                        run.started_by.name ? 'text-[#1a1a1a]' : 'text-[#8a8a8a]'
                      )}>
                      {starter}
                    </td>
                    <td className="h-11 text-[13px] whitespace-nowrap text-[#8a8a8a]">
                      <time dateTime={run.started_at}>{auditTime(run.started_at)}</time>
                    </td>
                    <td className="h-11 truncate font-mono text-[12px] text-[#3f3f3f]" title={run.conversation_id}>
                      {run.conversation_id.slice(0, 8)}
                    </td>
                    <td className="h-11 pr-2.5">
                      <div className="flex items-center justify-end gap-1">
                        <button
                          type="button"
                          data-ui="org.runs.logs"
                          aria-label={`${t('org.runs.logs')} ${run.execution_id.slice(0, 8)}`}
                          title={t('org.runs.logs')}
                          disabled={run.status === null}
                          className={rowActionClassName}
                          onClick={() => setLogsOf(run)}>
                          <ScrollText className="size-3.5 text-current" strokeWidth={1.7} />
                        </button>
                        {canStop && runStoppable(run.status) ? (
                          <PopConfirm
                            tone="danger"
                            title={t('org.runs.stopTitle')}
                            description={t('org.runs.stopDesc')}
                            confirmLabel={t('org.runs.stop')}
                            cancelLabel={t('common.cancel')}
                            disabled={pendingId !== null}
                            onConfirm={() => void stop(run)}>
                            <button
                              type="button"
                              data-ui="org.runs.stop"
                              aria-label={`${t('org.runs.stop')} ${run.execution_id.slice(0, 8)}`}
                              title={t('org.runs.stop')}
                              disabled={pendingId !== null}
                              className={rowActionClassName}>
                              <Square className="size-3.5 text-current" strokeWidth={1.7} />
                            </button>
                          </PopConfirm>
                        ) : null}
                      </div>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
          <div className="flex justify-center py-4">
            {hasMore ? (
              <button
                type="button"
                data-ui="org.runs.more"
                disabled={loadingMore}
                className={quietButtonClassName(30)}
                onClick={() => void setSize(size + 1)}>
                {t(loadingMore ? 'org.runs.loading' : 'org.runs.more')}
              </button>
            ) : (
              <span data-ui="org.runs.end" className="text-[12px] text-[#8a8a8a]">
                {t('org.runs.end')}
              </span>
            )}
          </div>
          {error ? <p className="m-0 text-center text-[12px] text-[#8a8a8a]">{t('org.error.generic')}</p> : null}
        </>
      )}
      <RunLogsDialog orgId={orgId} run={logsOf} onClose={() => setLogsOf(null)} />
    </OrgAdminFrame>
  )
}

function RunLogsDialog({ orgId, run, onClose }: { orgId: string; run: TeamRun | null; onClose: () => void }) {
  const { t } = useTranslation()
  const executionId = run?.execution_id ?? null
  const { data, error, isValidating, size, setSize } = useSWRInfinite<
    RunLogPage,
    unknown,
    (index: number, previous: RunLogPage | null) => LogsKey | null
  >(
    (index, previous) =>
      !executionId || (index > 0 && !previous?.next)
        ? null
        : ['studio/org-run-logs', orgId, executionId, previous?.next ?? null],
    async ([, org, execution, cursor]) => runLogPage(await teamRunLogs(org, execution, cursor)),
    { revalidateOnFocus: false }
  )
  const lines = useMemo(() => (data ?? []).flatMap((page) => page.lines), [data])
  const hasMore = !!data && data[data.length - 1]?.next !== null
  const loadingMore = isValidating && size > (data?.length ?? 0)

  return (
    <Dialog
      open={run !== null}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}>
      <DialogContent aria-describedby={undefined} className="sm:max-w-[640px]">
        <DialogTitle className="text-[15px] leading-5 font-medium">
          {t('org.runs.logsTitle')}
          {run ? (
            <span className="ml-2 font-mono text-[12px] text-[#8a8a8a]">{run.execution_id.slice(0, 8)}</span>
          ) : null}
        </DialogTitle>
        <div data-ui="org.runs.logs-body" className="max-h-[60vh] min-h-40 overflow-y-auto">
          {error && !data ? (
            <p data-ui="org.runs.logs-error" className="m-0 py-10 text-center text-[13px] text-[#8a8a8a]">
              {t(gone(error) ? 'org.runs.logsGone' : 'org.error.generic')}
            </p>
          ) : !data ? (
            <div className="relative min-h-40">
              <PageEntry id="org.runs.logs" ready={false} />
            </div>
          ) : lines.length === 0 ? (
            <p data-ui="org.runs.logs-empty" className="m-0 py-10 text-center text-[13px] text-[#8a8a8a]">
              {t('org.runs.logsEmpty')}
            </p>
          ) : (
            <ol className="m-0 grid list-none gap-1 p-0 font-mono text-[12px] leading-5">
              {lines.map((line) => (
                <li key={line.key} className="grid grid-cols-[140px_1fr] gap-3">
                  <span className="truncate text-[#8a8a8a]">{line.kind}</span>
                  <span className="break-words whitespace-pre-wrap text-[#1a1a1a]">{line.text}</span>
                </li>
              ))}
            </ol>
          )}
        </div>
        {hasMore ? (
          <div className="flex justify-center">
            <button
              type="button"
              disabled={loadingMore}
              className={quietButtonClassName(30)}
              onClick={() => void setSize(size + 1)}>
              {t(loadingMore ? 'org.runs.loading' : 'org.runs.logsMore')}
            </button>
          </div>
        ) : null}
      </DialogContent>
    </Dialog>
  )
}
