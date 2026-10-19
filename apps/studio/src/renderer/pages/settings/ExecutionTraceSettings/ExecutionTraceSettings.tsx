import { SearchInput } from '@a3s-lab/ui'
import { useNavigate } from '@tanstack/react-router'
import type { TFunction } from 'i18next'
import {
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Download,
  FileSearch,
  FileText,
  Flag,
  FoldHorizontal,
  Gauge,
  GitBranch,
  Globe,
  Hash,
  MessageCircle,
  Play,
  ScanText,
  Send,
  Sparkles,
  SquareTerminal,
  Timer,
  Wrench,
  type LucideIcon
} from 'lucide-react'
import { useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { SettingsHeaderAction } from '@renderer/pages/settings/settingsHeaderAction'
import { clearTraceFocus, readTraceFocus } from '@renderer/services/traceFocus'

import { exportTraceDocument, saveTraceFile } from './exportTraceBook'
import {
  type SessionTrace,
  type StoredTrainingEvent,
  type TraceBook,
  type TraceStepSignals,
  scanSessionRecords,
  trainingEventDetail,
  trainingEventSignals,
  trainingEventState,
  trainingEventTool,
  trainingEventUsage
} from './extractExecutionTrace'
import { loadLocalSessionRecords } from './loadLocalSessionRecords'
import { readTraceBook, writeTraceBook } from './traceBookStore'
import {
  countTraceHits,
  filterTraceHits,
  groupTraceHits,
  pageTraceGroups,
  sessionTraceOutcome,
  type TraceOutcome
} from './traceBrowse'

type ScanStatus = 'idle' | 'scanning' | 'done' | 'failed'

const EVENT_LABEL: Record<StoredTrainingEvent['record']['event_type'], string> = {
  context_compacted: 'settings.traces.event.context_compacted',
  execution_end: 'settings.traces.event.execution_end',
  execution_start: 'settings.traces.event.execution_start',
  llm_request: 'settings.traces.event.llm_request',
  llm_response: 'settings.traces.event.llm_response',
  tool_call: 'settings.traces.event.tool_call',
  tool_result: 'settings.traces.event.tool_result'
}

const DECISION_KEY: Record<string, string> = {
  approved: 'settings.traces.decision.approved',
  awaiting_user: 'settings.traces.decision.awaiting_user',
  denied: 'settings.traces.decision.denied',
  failed: 'settings.traces.decision.failed',
  finished: 'settings.traces.decision.finished',
  invoked: 'settings.traces.decision.invoked',
  reasoning: 'settings.traces.decision.reasoning',
  reply: 'settings.traces.decision.reply',
  stopped: 'settings.traces.decision.stopped',
  succeeded: 'settings.traces.decision.succeeded',
  tool_use: 'settings.traces.decision.tool_use'
}

const EVENT_ICON: Record<StoredTrainingEvent['record']['event_type'], LucideIcon> = {
  context_compacted: FoldHorizontal,
  execution_end: Flag,
  execution_start: Play,
  llm_request: Send,
  llm_response: Sparkles,
  tool_call: Wrench,
  tool_result: Wrench
}

const APPROVAL_DECISIONS = new Set(['approved', 'awaiting_user', 'denied'])

const TOOL_ICON: Record<string, LucideIcon> = {
  bash: SquareTerminal,
  edit: FileText,
  glob: FileSearch,
  grep: FileSearch,
  read: FileText,
  webfetch: Globe,
  websearch: Globe,
  web_fetch: Globe,
  web_search: Globe,
  write: FileText
}

export function ExecutionTraceSettings() {
  const { t } = useTranslation()
  const [book, setBook] = useState<TraceBook>(() => readTraceBook())
  const bookRef = useRef(book)
  bookRef.current = book
  const [status, setStatus] = useState<ScanStatus>('idle')
  const [delta, setDelta] = useState<{ skipped: number; added: number } | null>(null)
  const [query, setQuery] = useState('')
  const [outcomeFilter, setOutcomeFilter] = useState<TraceOutcome | 'all'>('all')
  const [page, setPage] = useState(0)
  const [focusId, setFocusId] = useState(readTraceFocus)
  const [exportMeasured, setExportMeasured] = useState<number | null>(null)
  const [openSessions, setOpenSessions] = useState<ReadonlySet<string>>(() => new Set())

  const analyze = async () => {
    if (status === 'scanning') return
    setStatus('scanning')
    try {
      const records = await loadLocalSessionRecords(bookRef.current)
      const result = await scanSessionRecords(bookRef.current, records)
      writeTraceBook(result.book)
      setBook(result.book)
      setDelta({ skipped: result.skipped, added: result.added })
      setPage(0)
      setStatus('done')
    } catch {
      setStatus('failed')
    }
  }

  const exportRecords = () => {
    const result = exportTraceDocument(bookRef.current)
    setExportMeasured(result.measured)
    saveTraceFile('a3s.rl_trajectory.v1.jsonl', result.text)
  }

  const sessions = useMemo(
    () => (focusId ? book.sessions.filter((session) => session.sessionId === focusId) : book.sessions),
    [book.sessions, focusId]
  )
  const counts = useMemo(() => {
    const totals = { all: sessions.length, failed: 0, done: 0, running: 0 }
    for (const session of sessions) totals[sessionTraceOutcome(session)] += 1
    return totals
  }, [sessions])
  const filteredSessions = useMemo(
    () =>
      outcomeFilter === 'all' ? sessions : sessions.filter((session) => sessionTraceOutcome(session) === outcomeFilter),
    [outcomeFilter, sessions]
  )
  const hits = useMemo(
    () => filterTraceHits(filteredSessions, query, (session, event) => traceHaystack(session, event, t)),
    [filteredSessions, query, t]
  )
  const groups = useMemo(() => groupTraceHits(hits, countTraceHits(hits)), [hits])
  const paged = pageTraceGroups(groups, page)

  const statusText =
    status === 'scanning'
      ? t('settings.traces.analyzing')
      : status === 'failed'
        ? t('settings.traces.scan.failed')
        : status === 'done' && delta && delta.added === 0
          ? t('settings.traces.scan.none')
          : status === 'done' && delta
            ? t('settings.traces.scan.added', delta)
            : book.sessions.length === 0
              ? t('settings.traces.empty')
              : t('settings.traces.hint')

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-1 flex-col" data-ui="settings.traces">
      <SettingsHeaderAction>
        <button
          className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg border border-[var(--color-border)] bg-white px-2.5 text-xs whitespace-nowrap text-[var(--color-ink)] disabled:opacity-60"
          data-ui="settings.traces.export"
          disabled={book.sessions.length === 0}
          onClick={exportRecords}
          type="button">
          <Download aria-hidden size={14} strokeWidth={1.75} />
          {t('settings.traces.export')}
        </button>
        <button
          className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg border border-[var(--color-ink)] bg-[var(--color-ink)] px-2.5 text-xs whitespace-nowrap text-white disabled:opacity-60"
          data-ui="settings.traces.analyze"
          disabled={status === 'scanning'}
          onClick={() => void analyze()}
          type="button">
          <ScanText aria-hidden size={14} strokeWidth={1.75} />
          {status === 'scanning' ? t('settings.traces.analyzing') : t('settings.traces.analyze')}
        </button>
      </SettingsHeaderAction>
      <p
        className="shrink-0 px-6 pb-3 text-[13px] text-[var(--color-ink-muted)]"
        data-added={delta?.added ?? ''}
        data-skipped={delta?.skipped ?? ''}
        data-ui="settings.traces.status">
        {statusText}
      </p>
      {exportMeasured === null ? null : (
        <p
          className="shrink-0 px-6 pb-3 text-[13px] text-[var(--color-ink-muted)]"
          data-measured={exportMeasured}
          data-ui="settings.traces.export.status">
          {exportMeasured > 0
            ? t('settings.traces.export.sample', { count: exportMeasured })
            : t('settings.traces.export.plain')}
        </p>
      )}
      {book.sessions.length === 0 ? null : (
        <div className="flex shrink-0 flex-col gap-2 px-6 pb-3" data-ui="settings.traces.toolbar">
          {focusId ? (
            <div className="flex items-center justify-between gap-2">
              <p className="m-0 text-xs text-[var(--color-ink-muted)]" data-ui="settings.traces.focus">
                {t('settings.traces.focus')}
              </p>
              <button
                className="inline-flex h-8 shrink-0 items-center rounded-lg border border-[var(--color-border)] bg-white px-2.5 text-xs whitespace-nowrap text-[var(--color-ink)]"
                data-ui="settings.traces.focus.clear"
                onClick={() => {
                  clearTraceFocus()
                  setFocusId('')
                  setPage(0)
                }}
                type="button">
                {t('settings.traces.focus.clear')}
              </button>
            </div>
          ) : null}
          <div
            aria-label={t('settings.traces.filter.label')}
            className="flex flex-wrap gap-1"
            data-ui="settings.traces.filter"
            role="group">
            <FilterButton
              count={counts.all}
              label={t('settings.traces.filter.all')}
              onClick={() => {
                setOutcomeFilter('all')
                setPage(0)
              }}
              selected={outcomeFilter === 'all'}
            />
            <FilterButton
              count={counts.failed}
              label={t('settings.traces.filter.failed')}
              onClick={() => {
                setOutcomeFilter('failed')
                setPage(0)
              }}
              selected={outcomeFilter === 'failed'}
            />
            <FilterButton
              count={counts.done}
              label={t('settings.traces.filter.done')}
              onClick={() => {
                setOutcomeFilter('done')
                setPage(0)
              }}
              selected={outcomeFilter === 'done'}
            />
            <FilterButton
              count={counts.running}
              label={t('settings.traces.filter.running')}
              onClick={() => {
                setOutcomeFilter('running')
                setPage(0)
              }}
              selected={outcomeFilter === 'running'}
            />
          </div>
          <SearchInput
            aria-label={t('settings.traces.search.placeholder')}
            clearLabel={t('settings.traces.search.clear')}
            containerClassName="h-8 w-full bg-white"
            data-ui="settings.traces.search"
            onChange={(event) => {
              setQuery(event.target.value)
              setPage(0)
            }}
            onClear={() => {
              setQuery('')
              setPage(0)
            }}
            placeholder={t('settings.traces.search.placeholder')}
            size="sm"
            value={query}
          />
          {paged.total > 0 ? (
            <div className="flex items-center justify-between gap-2">
              <p className="m-0 text-xs text-[var(--color-ink-muted)]" data-ui="settings.traces.page.range">
                {t('settings.traces.page.range', { from: paged.from, to: paged.to, total: paged.total })}
              </p>
              <div
                className="flex shrink-0 gap-1"
                data-page={paged.page + 1}
                data-pages={paged.pages}
                data-ui="settings.traces.pager">
                <PagerButton
                  disabled={paged.page <= 0}
                  icon={ChevronLeft}
                  label={t('settings.traces.page.prev')}
                  onClick={() => setPage(paged.page - 1)}
                  ui="settings.traces.page.prev"
                />
                <PagerButton
                  disabled={paged.page + 1 >= paged.pages}
                  icon={ChevronRight}
                  label={t('settings.traces.page.next')}
                  onClick={() => setPage(paged.page + 1)}
                  ui="settings.traces.page.next"
                />
              </div>
            </div>
          ) : null}
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-auto px-6 pb-6">
        {book.sessions.length === 0 ? null : paged.total === 0 ? (
          <p className="m-0 text-[13px] text-[var(--color-ink-muted)]" data-ui="settings.traces.search.empty">
            {query.trim() ? t('settings.traces.search.empty') : t('settings.traces.filter.empty')}
          </p>
        ) : (
          <div className="flex flex-col gap-3">
            {paged.groups.map((group) => (
              <SessionCard
                key={group.session.sessionId}
                count={group.count}
                events={group.events}
                onToggle={() =>
                  setOpenSessions((current) => {
                    const next = new Set(current)
                    if (next.has(group.session.sessionId)) next.delete(group.session.sessionId)
                    else next.add(group.session.sessionId)
                    return next
                  })
                }
                open={openSessions.has(group.session.sessionId) || query.trim().length > 0}
                session={group.session}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

function FilterButton({
  count,
  label,
  onClick,
  selected
}: {
  count: number
  label: string
  onClick: () => void
  selected: boolean
}) {
  return (
    <button
      aria-pressed={selected}
      className={`inline-flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-xs whitespace-nowrap ${
        selected
          ? 'bg-[var(--color-ink)] text-white'
          : 'border border-[var(--color-border)] bg-white text-[var(--color-ink)]'
      }`}
      onClick={onClick}
      type="button">
      <span>{label}</span>
      <span className={selected ? 'text-white/80' : 'text-[var(--color-ink-muted)]'}>{count}</span>
    </button>
  )
}

function PagerButton({
  disabled,
  icon: Icon,
  label,
  onClick,
  ui
}: {
  disabled: boolean
  icon: LucideIcon
  label: string
  onClick: () => void
  ui: string
}) {
  return (
    <button
      aria-label={label}
      className="inline-flex h-8 w-8 items-center justify-center rounded-lg border border-[var(--color-border)] bg-white text-[var(--color-ink)] disabled:opacity-40"
      data-ui={ui}
      disabled={disabled}
      onClick={onClick}
      type="button">
      <Icon aria-hidden size={14} strokeWidth={1.75} />
    </button>
  )
}

function SessionCard({
  session,
  events,
  count,
  open,
  onToggle
}: {
  session: SessionTrace
  events: readonly StoredTrainingEvent[]
  count: number
  open: boolean
  onToggle: () => void
}) {
  const { t } = useTranslation()
  const title = session.name.trim() || t('settings.traces.session.untitled')
  const navigate = useNavigate()
  const outcome = sessionTraceOutcome(session)
  const error = outcome === 'failed' ? sessionErrorLine(session) : ''
  const when = sessionWhen(session.updatedAt)
  return (
    <article
      className={`min-w-0 overflow-hidden rounded-xl border px-3 py-2.5 ${
        outcome === 'failed' ? 'border-[#fecaca] bg-[#fff7f7]' : 'border-[var(--color-border)] bg-white'
      }`}
      data-outcome={outcome}
      data-session-id={session.sessionId}
      data-ui="settings.traces.session">
      <header className="flex items-start gap-2">
        <button
          aria-expanded={open}
          className="flex min-w-0 flex-1 items-start gap-2 bg-transparent p-0 text-left"
          data-ui="settings.traces.session.toggle"
          onClick={onToggle}
          type="button">
          <span className="mt-0.5 inline-flex h-5 w-5 shrink-0 items-center justify-center text-[var(--color-ink-muted)]">
            {open ? (
              <ChevronDown aria-hidden size={14} strokeWidth={1.75} />
            ) : (
              <ChevronRight aria-hidden size={14} strokeWidth={1.75} />
            )}
          </span>
          <span className="min-w-0 flex-1">
            <span className="flex min-w-0 items-center gap-2">
              <h2 className="m-0 min-w-0 truncate text-[13px] font-medium text-[var(--color-ink)]">{title}</h2>
              <OutcomeBadge outcome={outcome} />
            </span>
            {error && !open ? (
              <span className="mt-1 line-clamp-2 block text-[12px] leading-5 break-words text-[#b42318]">{error}</span>
            ) : null}
          </span>
        </button>
        <span className="flex shrink-0 items-center gap-2 pt-0.5 text-[11px] text-[var(--color-ink-muted)]">
          {when ? <time dateTime={session.updatedAt}>{when}</time> : null}
          <span>{t('settings.traces.session.steps', { count })}</span>
          <button
            className="inline-flex h-7 items-center rounded-md border border-[var(--color-border)] bg-white px-2 text-[11px] whitespace-nowrap text-[var(--color-ink)]"
            data-ui="settings.traces.open"
            onClick={() => {
              void navigate({ to: '/app/agents', search: { sessionId: session.sessionId } })
            }}
            type="button">
            {t('settings.traces.open')}
          </button>
        </span>
      </header>
      {open ? (
        <ol className="relative m-0 mt-1 flex list-none flex-col p-0">
          <span
            aria-hidden
            className="pointer-events-none absolute top-3 bottom-3 left-[11px] w-px bg-[var(--color-border)]"
          />
          {events.map((event) => (
            <EventRow event={event} key={`${event.messageId}:${event.record.sequence}`} sessionId={session.sessionId} />
          ))}
        </ol>
      ) : null}
    </article>
  )
}

function OutcomeBadge({ outcome }: { outcome: 'failed' | 'done' | 'running' }) {
  const { t } = useTranslation()
  const label =
    outcome === 'failed'
      ? t('settings.traces.states.failed')
      : outcome === 'done'
        ? t('settings.traces.states.done')
        : t('settings.traces.states.running')
  const tone =
    outcome === 'failed'
      ? 'bg-[#fde8e8] text-[#b42318]'
      : outcome === 'done'
        ? 'bg-[#e8f6ee] text-[#067647]'
        : 'bg-black/[0.04] text-[var(--color-ink-muted)]'
  return (
    <span
      className={`inline-flex shrink-0 items-center rounded-full px-1.5 py-0.5 text-[11px] leading-4 font-medium ${tone}`}
      data-ui="settings.traces.outcome">
      {label}
    </span>
  )
}

function sessionErrorLine(session: SessionTrace): string {
  for (let index = session.events.length - 1; index >= 0; index -= 1) {
    const record = session.events[index]?.record
    if (!record || trainingEventState(record) !== 'failed') continue
    const detail = tracePreview(trainingEventDetail(record))
    if (detail) return detail
  }
  return ''
}

function sessionWhen(updatedAt: string): string {
  const date = new Date(updatedAt)
  if (Number.isNaN(date.getTime())) return ''
  return new Intl.DateTimeFormat(undefined, {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  }).format(date)
}

function EventRow({ event, sessionId }: { event: StoredTrainingEvent; sessionId: string }) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const record = event.record
  const state = trainingEventState(record)
  const tool = trainingEventTool(record)
  const signals = trainingEventSignals(record)
  const usage = trainingEventUsage(record)
  const detail =
    trainingEventDetail(record) ||
    (record.event_type === 'context_compacted'
      ? t('settings.traces.detail.compact')
      : record.event_type === 'execution_end' && record.payload.success === false
        ? t('settings.traces.detail.error')
        : '')
  const Icon = rowIcon(event, state)
  const openSession = () => {
    void navigate({ to: '/app/agents', search: { sessionId } })
  }
  return (
    <li
      className={`relative min-w-0 py-1.5 text-xs ${state === 'failed' ? 'text-[#b42318]' : ''}`}
      data-decision={signals.decision}
      data-duration-ms={signals.durationMs ?? ''}
      data-event-type={record.event_type}
      data-feedback={signals.feedback}
      data-schema={record.schema}
      data-sequence={record.sequence}
      data-state={state ?? ''}
      data-tool={tool}
      data-ui="settings.traces.event">
      <div className="flex min-w-0 gap-2.5">
        <span
          className="relative z-10 mt-0.5 inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-[var(--color-border)] bg-white text-[var(--color-ink)]"
          data-ui="settings.traces.event.icon">
          <Icon aria-hidden size={13} strokeWidth={1.75} />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <button
            aria-label={t('settings.traces.open')}
            className="flex w-full min-w-0 items-baseline gap-2 bg-transparent p-0 text-left text-[13px] text-inherit"
            data-ui="settings.traces.open"
            onClick={openSession}
            type="button">
            <span className="shrink-0 text-[var(--color-ink)]">{t(EVENT_LABEL[record.event_type])}</span>
            {tool ? <span className="min-w-0 truncate text-[var(--color-ink-muted)]">{tool}</span> : null}
            {state ? (
              <span
                className={`ml-auto shrink-0 text-[11px] font-medium ${
                  state === 'failed' ? 'text-[#b42318]' : 'text-[#067647]'
                }`}>
                {t(`settings.traces.states.${state}`)}
              </span>
            ) : null}
          </button>
          <SignalRow signals={signals} t={t} usage={usage} />
          {detail ? <DetailText detail={detail} failed={state === 'failed'} /> : null}
        </div>
      </div>
    </li>
  )
}

function DetailText({ detail, failed = false }: { detail: string; failed?: boolean }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const preview = tracePreview(detail)
  const long = detail.length > 160 || detail.includes('\n')
  return (
    <div className="min-w-0" data-open={open ? 'true' : 'false'} data-ui="settings.traces.detail">
      {open ? (
        <pre
          className={`m-0 max-h-40 max-w-full overflow-auto rounded-lg px-2 py-1.5 font-mono text-[11px] leading-5 break-words whitespace-pre-wrap ${
            failed ? 'bg-[#fde8e8] text-[#7a271a]' : 'bg-black/[0.03] text-[var(--color-ink-muted)]'
          }`}>
          {detail}
        </pre>
      ) : (
        <p
          className={`m-0 line-clamp-2 text-[12px] leading-5 break-words ${
            failed ? 'text-[#7a271a]' : 'text-[var(--color-ink-muted)]'
          }`}>
          {preview}
        </p>
      )}
      {long ? (
        <button
          className="mt-0.5 bg-transparent p-0 text-[11px] text-[var(--color-ink-muted)]"
          data-ui="settings.traces.detail.toggle"
          onClick={() => setOpen((value) => !value)}
          type="button">
          {open ? t('settings.traces.detail.collapse') : t('settings.traces.detail.expand')}
        </button>
      ) : null}
    </div>
  )
}

function SignalRow({
  signals,
  usage,
  t
}: {
  signals: TraceStepSignals
  usage: { prompt: number; completion: number } | null
  t: TFunction
}) {
  const chips: { kind: string; icon: LucideIcon; label: string; value: string }[] = []
  const decision = visibleDecision(signals.decision)
  if (decision) {
    const key = DECISION_KEY[decision]
    chips.push({
      kind: 'decision',
      icon: GitBranch,
      label: t('settings.traces.signal.decision'),
      value: key ? t(key) : decision
    })
  }
  if (typeof signals.durationMs === 'number' && Number.isFinite(signals.durationMs) && signals.durationMs >= 0) {
    chips.push({
      kind: 'duration',
      icon: Timer,
      label: t('settings.traces.signal.duration'),
      value: durationLabel(signals.durationMs, t)
    })
  }
  if (signals.feedback) {
    chips.push({
      kind: 'feedback',
      icon: MessageCircle,
      label: t('settings.traces.signal.feedback'),
      value: signals.feedback
    })
  }
  if (signals.stopReason) {
    chips.push({
      kind: 'stop',
      icon: Flag,
      label: t('settings.traces.signal.stop'),
      value: signals.stopReason
    })
  }
  if (typeof signals.exitCode === 'number') {
    chips.push({
      kind: 'exit',
      icon: Hash,
      label: t('settings.traces.signal.exit'),
      value: String(signals.exitCode)
    })
  }
  if (signals.errorKind) {
    chips.push({
      kind: 'error',
      icon: CircleAlert,
      label: t('settings.traces.signal.error'),
      value: signals.errorKind
    })
  }
  if (usage) {
    chips.push({
      kind: 'usage',
      icon: Gauge,
      label: t('settings.traces.signal.usage'),
      value: t('settings.traces.usage.value', { in: usage.prompt, out: usage.completion })
    })
  }
  if (chips.length === 0) return null
  return (
    <span className="flex min-w-0 flex-wrap gap-1">
      {chips.map((chip) => {
        const Icon = chip.icon
        return (
          <span
            className="inline-flex max-w-full items-center gap-1 rounded-md border border-[var(--color-border)] bg-white px-1.5 py-0.5 text-[11px] text-[var(--color-ink)]"
            data-signal={chip.kind}
            data-ui="settings.traces.signal"
            key={chip.kind}
            title={`${chip.label} ${chip.value}`}>
            <Icon aria-hidden size={12} strokeWidth={1.75} />
            <span className="shrink-0 text-[var(--color-ink-muted)]">{chip.label}</span>
            <span className="min-w-0 truncate font-mono">{chip.value}</span>
          </span>
        )
      })}
    </span>
  )
}

function rowIcon(event: StoredTrainingEvent, state: 'done' | 'failed' | null): LucideIcon {
  const type = event.record.event_type
  if (type === 'tool_call' || type === 'tool_result') {
    const name = trainingEventTool(event.record).trim().toLowerCase()
    return TOOL_ICON[name] ?? Wrench
  }
  if (state === 'failed' && type === 'execution_end') return CircleAlert
  return EVENT_ICON[type]
}

function durationLabel(ms: number, t: TFunction): string {
  if (!Number.isFinite(ms) || ms < 0) return ''
  if (ms < 1000) return t('settings.traces.duration.ms', { value: Math.round(ms) })
  const rounded = Math.round((ms / 1000) * 10) / 10
  const value = Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1)
  return t('settings.traces.duration.s', { value })
}

function visibleDecision(decision: string): string {
  return APPROVAL_DECISIONS.has(decision) ? decision : ''
}

function tracePreview(text: string): string {
  return text
    .replace(/<\s*\|[^>\n]{0,80}DSML[^>\n]*>/gi, ' ')
    .replace(/\|\|DSML\|[^|\n]*\|/gi, ' ')
    .replace(/\s+/g, ' ')
    .trim()
}

function decisionText(t: TFunction, decision: string): string {
  const key = DECISION_KEY[decision]
  return key ? t(key) : decision
}

function traceHaystack(session: SessionTrace, event: StoredTrainingEvent, t: TFunction): string {
  const record = event.record
  const signals = trainingEventSignals(record)
  const usage = trainingEventUsage(record)
  const state = trainingEventState(record)
  return [
    session.name,
    t(EVENT_LABEL[record.event_type]),
    trainingEventTool(record),
    trainingEventDetail(record),
    decisionText(t, signals.decision),
    signals.feedback,
    signals.durationMs !== undefined ? durationLabel(signals.durationMs, t) : '',
    signals.stopReason,
    signals.errorKind,
    signals.exitCode !== undefined ? String(signals.exitCode) : '',
    state ? t(`settings.traces.states.${state}`) : '',
    usage ? `${usage.prompt} ${usage.completion}` : ''
  ].join('\n')
}
