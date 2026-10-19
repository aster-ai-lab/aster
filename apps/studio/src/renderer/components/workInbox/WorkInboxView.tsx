import { PopConfirm } from '@a3s-lab/ui'
import {
  BadgeCheck,
  Check,
  ChevronDown,
  CircleCheck,
  Clock,
  Copy,
  CalendarDays,
  Cpu,
  FileCheck,
  Inbox,
  ListChecks,
  Megaphone,
  PanelLeftClose,
  PanelLeftOpen,
  Plane,
  Receipt,
  Search,
  Send,
  Stamp,
  TreePalm,
  UserPlus,
  X,
  type LucideIcon
} from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { useRestingDialog } from '@renderer/components/restingDialog'
import { subscribeCenter } from '@renderer/data/memberOrgs'
import {
  decideWorkItem,
  retryMeetingSchedule,
  loadWorkItems,
  loadWorkSummary,
  notifyWorkInboxChanged,
  WORK_INBOX_CHANGED,
  type WorkBox,
  type WorkDecision,
  type WorkItem,
  type WorkKind,
  type WorkStatus,
  type WorkSummary
} from '@renderer/workInbox/client'

import { showsApprovalFlow } from './approvalFlow'
import { ApprovalFlowDiagram } from './ApprovalFlowDiagram'

type Days = 0 | 7 | 30

type Place = {
  box: WorkBox
  kind: WorkKind | ''
  status: WorkStatus | ''
  days: Days
  query: string
  department: string
}

const HOME_PLACE: Place = { box: 'todo', kind: '', status: '', days: 0, query: '', department: '' }
const BOXES: WorkBox[] = ['todo', 'done', 'cc', 'sent']

function samePlace(left: Place, right: Place): boolean {
  return (
    left.box === right.box &&
    left.kind === right.kind &&
    left.status === right.status &&
    left.days === right.days &&
    left.query === right.query &&
    left.department === right.department
  )
}

export type WorkInboxNav = {
  canBack: boolean
  canForward: boolean
  back: () => void
  forward: () => void
  home: () => void
  reload: () => void
}

const BOX_ICON: Record<WorkBox, LucideIcon> = {
  todo: Inbox,
  done: CircleCheck,
  cc: Copy,
  sent: Send
}

const KIND_NAV: { kind: WorkKind | ''; labelKey: string; icon: LucideIcon }[] = [
  { kind: '', labelKey: 'work.inbox.kind.all', icon: ListChecks },
  { kind: 'join_request', labelKey: 'work.inbox.kind.join', icon: UserPlus },
  { kind: 'approval', labelKey: 'work.inbox.kind.approval', icon: FileCheck },
  { kind: 'leave', labelKey: 'work.inbox.kind.leave', icon: TreePalm },
  { kind: 'travel', labelKey: 'work.inbox.kind.travel', icon: Plane },
  { kind: 'expense', labelKey: 'work.inbox.kind.expense', icon: Receipt },
  { kind: 'meeting', labelKey: 'work.inbox.kind.meeting', icon: CalendarDays },
  { kind: 'seal', labelKey: 'work.inbox.kind.seal', icon: Stamp },
  { kind: 'notice', labelKey: 'work.inbox.kind.notice', icon: Megaphone },
  { kind: 'correction', labelKey: 'work.inbox.kind.correction', icon: Clock },
  { kind: 'probation', labelKey: 'work.inbox.kind.probation', icon: BadgeCheck },
  { kind: 'ai_resource', labelKey: 'work.inbox.kind.ai', icon: Cpu }
]

export type WorkInboxViewProps = {
  loadItems?: typeof loadWorkItems
  loadSummary?: typeof loadWorkSummary
  decideItem?: typeof decideWorkItem
  retrySchedule?: typeof retryMeetingSchedule
  onNav?: (nav: WorkInboxNav) => void
}

function formatStamp(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  const pad = (part: number) => String(part).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}

function trailInk(action: string): string {
  if (action === '跳过') return 'text-[#8a8a8a]'
  if (action === '退回' || action === '拒绝') return 'text-[#b42318]'
  return 'text-[#171717]'
}

function trailDot(action: string): string {
  if (action === '跳过') return 'bg-[#c5c5c5]'
  if (action === '退回' || action === '拒绝') return 'bg-[#b42318]'
  return 'bg-[#171717]'
}

function ProcessTrail({ item }: { item: WorkItem }) {
  if (item.trail.length === 0) return null
  return (
    <ol data-ui="work.inbox.trail" className="mt-2 flex flex-col">
      {item.trail.map((mark, index) => (
        <li key={`${mark.node_name}-${mark.action}-${index}`} className="flex gap-2">
          <span className="flex w-3 shrink-0 flex-col items-center">
            <span aria-hidden className={`mt-1.5 size-1.5 shrink-0 rounded-full ${trailDot(mark.action)}`} />
            {index < item.trail.length - 1 ? <span aria-hidden className="my-0.5 w-px flex-1 bg-[#e5e5e5]" /> : null}
          </span>
          <span className={`min-w-0 pb-2 text-[13px] leading-5 ${trailInk(mark.action)}`}>
            <span className="font-medium">{mark.action}</span>
            <span className="text-[#8a8a8a]">
              {' '}
              {mark.node_name} · {mark.actor_name}
            </span>
          </span>
        </li>
      ))}
    </ol>
  )
}

function statusClass(status: WorkStatus): string {
  const base = 'inline-flex h-5 shrink-0 items-center rounded-[4px] px-1.5 text-[11px]'
  if (status === 'approved') return `${base} bg-[#e7f6ee] text-[#137333]`
  if (status === 'rejected') return `${base} bg-[#fdecec] text-[#b42318]`
  if (status === 'cancelled') return `${base} bg-[#f5f5f5] text-[#8a8a8a]`
  return `${base} bg-[#f2f3f5] font-medium text-[#171717]`
}

function nameMark(name: string): string {
  const chars = Array.from(name.trim())
  if (chars.length === 0) return ''
  const cjk = chars.some((char) => /[\u3400-\u9fff]/.test(char))
  return cjk ? chars.slice(-2).join('') : chars.slice(0, 2).join('').toUpperCase()
}

function fieldText(value: unknown): string {
  if (typeof value === 'string') return value
  if (typeof value === 'number') return String(value)
  return ''
}

function detailRows(item: WorkItem, t: (key: string) => string): { label: string; value: string }[] {
  const text = (key: string) => fieldText(item.fields[key])
  if (item.kind === 'approval' && (text('title') || text('detail'))) {
    return [
      { label: t('workbench.applet.field.title'), value: text('title') || item.title },
      { label: t('workbench.applet.field.detail'), value: text('detail') }
    ]
  }
  if (item.kind === 'leave' && text('leave_type')) {
    return [
      { label: t('workbench.applet.leave.type'), value: text('leave_type') },
      { label: t('workbench.applet.leave.start'), value: text('start_date') },
      { label: t('workbench.applet.leave.end'), value: text('end_date') },
      { label: t('work.inbox.field.days'), value: text('days') },
      { label: t('workbench.applet.leave.reason'), value: text('reason') }
    ]
  }
  if (item.kind === 'travel' && text('destination')) {
    return [
      { label: t('workbench.applet.travel.destination'), value: text('destination') },
      { label: t('workbench.applet.leave.start'), value: text('start_date') },
      { label: t('workbench.applet.leave.end'), value: text('end_date') },
      { label: t('work.inbox.field.days'), value: text('days') },
      { label: t('workbench.applet.travel.reason'), value: text('reason') }
    ]
  }
  if (item.kind === 'expense' && text('amount')) {
    return [
      { label: t('workbench.applet.expense.amount'), value: text('amount') },
      { label: t('workbench.applet.expense.category'), value: text('category') },
      { label: t('workbench.applet.expense.date'), value: text('spent_on') },
      { label: t('workbench.applet.expense.reason'), value: text('detail') }
    ]
  }
  if (item.kind === 'meeting' && text('title')) {
    return [
      { label: t('workbench.applet.meeting.subject'), value: text('title') },
      { label: t('workbench.applet.meeting.day'), value: text('day') },
      { label: t('workbench.applet.meeting.start'), value: text('start_time') },
      { label: t('workbench.applet.meeting.end'), value: text('end_time') },
      { label: t('workbench.applet.meeting.place'), value: text('place') },
      { label: t('workbench.applet.meeting.agenda'), value: text('agenda') }
    ]
  }
  if (item.kind === 'seal' && text('document')) {
    return [
      { label: t('workbench.applet.seal.document'), value: text('document') },
      { label: t('workbench.applet.seal.kind'), value: text('seal_type') },
      { label: t('workbench.applet.seal.copies'), value: text('copies') },
      { label: t('workbench.applet.seal.purpose'), value: text('purpose') }
    ]
  }
  return [{ label: t('work.inbox.detail.title'), value: item.summary || item.title }]
}

function explainError(message: string, fallback: string, approver: string, org: string): string {
  if (message.includes('no other approver')) return approver
  if (message.includes('not a member') || message.includes('organization not found')) return org
  if (!message || message === 'runtime' || message === 'unauthorized' || message.startsWith('HTTP ')) return fallback
  return message
}

export function WorkInboxView({
  loadItems = loadWorkItems,
  loadSummary = loadWorkSummary,
  decideItem = decideWorkItem,
  retrySchedule = retryMeetingSchedule,
  onNav
}: WorkInboxViewProps) {
  const { t } = useTranslation()
  const [box, setBox] = useState<WorkBox>(HOME_PLACE.box)
  const [kind, setKind] = useState<WorkKind | ''>(HOME_PLACE.kind)
  const [status, setStatus] = useState<WorkStatus | ''>(HOME_PLACE.status)
  const [days, setDays] = useState<Days>(HOME_PLACE.days)
  const [department, setDepartment] = useState(HOME_PLACE.department)
  const [knownDepartments, setKnownDepartments] = useState<string[]>([])
  const [query, setQuery] = useState(HOME_PLACE.query)
  const [appliedQuery, setAppliedQuery] = useState(HOME_PLACE.query)
  const pastRef = useRef<Place[]>([])
  const futureRef = useRef<Place[]>([])
  const placeRef = useRef<Place>(HOME_PLACE)
  const skipHistory = useRef(false)
  const [trail, setTrail] = useState(0)
  const [summary, setSummary] = useState<WorkSummary>({ todo: 0, done: 0, cc: 0, sent: 0 })
  const [items, setItems] = useState<WorkItem[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [navOpen, setNavOpen] = useState(true)
  const [detailOpen, setDetailOpen] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [comment, setComment] = useState('')
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    const timer = window.setTimeout(() => setAppliedQuery(query.trim()), 300)
    return () => window.clearTimeout(timer)
  }, [query])

  const refreshEpoch = useRef(0)

  const refresh = useCallback(async () => {
    const epoch = ++refreshEpoch.current
    setLoading(true)
    setError('')
    try {
      const [nextSummary, nextItems] = await Promise.all([
        loadSummary(),
        loadItems({
          box,
          kind: kind || undefined,
          status: status || undefined,
          q: appliedQuery,
          days: days || undefined,
          department: department || undefined
        })
      ])
      if (epoch !== refreshEpoch.current) return
      setSummary(nextSummary)
      setItems(nextItems)
      setKnownDepartments((current) => {
        const next = current.slice()
        for (const row of nextItems) {
          const name = typeof row.fields.department === 'string' ? row.fields.department.trim() : ''
          if (name && !next.includes(name)) next.push(name)
        }
        return next.length === current.length ? current : next
      })
      setSelectedId((current) => (current && nextItems.some((item) => item.id === current) ? current : null))
    } catch (cause) {
      if (epoch !== refreshEpoch.current) return
      setItems([])
      setSummary({ todo: 0, done: 0, cc: 0, sent: 0 })
      const message = cause instanceof Error ? cause.message : ''
      setError(explainError(message, t('work.inbox.error'), t('work.inbox.error.approver'), t('work.inbox.form.org')))
    } finally {
      if (epoch === refreshEpoch.current) setLoading(false)
    }
  }, [appliedQuery, box, days, department, kind, loadItems, loadSummary, status, t])

  useEffect(() => {
    void refresh()
    const changed = () => {
      void refresh()
    }
    window.addEventListener(WORK_INBOX_CHANGED, changed)
    return () => {
      refreshEpoch.current += 1
      window.removeEventListener(WORK_INBOX_CHANGED, changed)
    }
  }, [refresh])

  useEffect(() => {
    return subscribeCenter(() => {
      setItems([])
      setSelectedId(null)
      void refresh()
    })
  }, [refresh])

  const applyPlace = useCallback((next: Place) => {
    skipHistory.current = true
    placeRef.current = next
    setBox(next.box)
    setKind(next.kind)
    setStatus(next.status)
    setDays(next.days)
    setDepartment(next.department)
    setQuery(next.query)
    setAppliedQuery(next.query)
    setSelectedId(null)
    setDetailOpen(false)
  }, [])

  const back = useCallback(() => {
    const previous = pastRef.current.at(-1)
    if (!previous) return
    futureRef.current = [placeRef.current, ...futureRef.current]
    pastRef.current = pastRef.current.slice(0, -1)
    applyPlace(previous)
    setTrail((value) => value + 1)
  }, [applyPlace])

  const forward = useCallback(() => {
    const [next, ...rest] = futureRef.current
    if (!next) return
    pastRef.current = [...pastRef.current, placeRef.current]
    futureRef.current = rest
    applyPlace(next)
    setTrail((value) => value + 1)
  }, [applyPlace])

  const home = useCallback(() => {
    setBox(HOME_PLACE.box)
    setKind(HOME_PLACE.kind)
    setStatus(HOME_PLACE.status)
    setDays(HOME_PLACE.days)
    setDepartment(HOME_PLACE.department)
    setQuery(HOME_PLACE.query)
    setAppliedQuery(HOME_PLACE.query)
    setSelectedId(null)
    setDetailOpen(false)
  }, [])

  const reload = useCallback(() => {
    void refresh()
  }, [refresh])

  useEffect(() => {
    if (skipHistory.current) {
      skipHistory.current = false
      return
    }
    const next: Place = { box, kind, status, days, query: appliedQuery, department }
    if (samePlace(placeRef.current, next)) return
    pastRef.current = [...pastRef.current, placeRef.current]
    futureRef.current = []
    placeRef.current = next
    setTrail((value) => value + 1)
  }, [appliedQuery, box, days, department, kind, status])

  useEffect(() => {
    onNav?.({
      canBack: pastRef.current.length > 0,
      canForward: futureRef.current.length > 0,
      back,
      forward,
      home,
      reload
    })
  }, [back, forward, home, onNav, reload, trail])

  const selected = items.find((item) => item.id === selectedId) ?? null
  const detailShown = selected !== null && detailOpen
  const detailPresence = useRestingDialog(detailShown)
  const detailHold = useRef(selected)
  if (selected) detailHold.current = selected
  const detailItem = detailPresence.present ? (selected ?? detailHold.current) : null
  const counts: Record<WorkBox, number> = {
    todo: summary.todo,
    done: summary.done,
    cc: summary.cc,
    sent: summary.sent
  }

  const chooseBox = (next: WorkBox) => {
    setBox(next)
    setSelectedId(null)
    setDetailOpen(false)
  }

  const chooseKind = (next: WorkKind | '') => {
    setKind(next)
    setSelectedId(null)
    setDetailOpen(false)
  }

  const openItem = (id: string) => {
    setSelectedId(id)
    setDetailOpen(true)
    setComment('')
  }

  const commentReady = comment.trim().length > 0

  const decide = async (item: WorkItem, decision: WorkDecision) => {
    if (busy || !commentReady) return
    setBusy(true)
    setError('')
    try {
      await decideItem(item.id, decision, comment.trim())
      notifyWorkInboxChanged()
      await refresh()
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : ''
      setError(explainError(message, t('work.inbox.error'), t('work.inbox.error.approver'), t('work.inbox.form.org')))
    } finally {
      setBusy(false)
    }
  }

  const timeOptions: FilterOption<Days>[] = [
    { value: 0, label: t('work.inbox.filter.time.all') },
    { value: 7, label: t('work.inbox.filter.time.days', { count: 7 }) },
    { value: 30, label: t('work.inbox.filter.time.days', { count: 30 }) }
  ]
  const kindOptions: FilterOption<WorkKind | ''>[] = [
    { value: '', label: t('work.inbox.filter.kind.all') },
    ...KIND_NAV.filter((row) => row.kind !== '').map((row) => ({
      value: row.kind,
      label: t(row.labelKey)
    }))
  ]
  const statusOptions: FilterOption<WorkStatus | ''>[] = [
    { value: '', label: t('work.inbox.filter.status.all') },
    { value: 'pending', label: t('work.inbox.status.pending') },
    { value: 'approved', label: t('work.inbox.status.approved') },
    { value: 'rejected', label: t('work.inbox.status.rejected') },
    { value: 'cancelled', label: t('work.inbox.status.cancelled') },
    { value: 'returned', label: t('work.inbox.status.returned') },
    { value: 'paid', label: t('work.inbox.status.paid') },
    { value: 'fulfillment_exception', label: t('work.inbox.status.fulfillment_exception') }
  ]
  const departmentOptions: FilterOption<string>[] = [
    { value: '', label: t('work.inbox.filter.department.all') },
    ...knownDepartments.map((name) => ({ value: name, label: name }))
  ]
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-white text-[#171717]">
      <div className="relative flex min-h-0 flex-1 overflow-hidden">
        {navOpen ? (
          <aside className="flex w-[220px] shrink-0 flex-col border-r border-[#efefef] bg-white">
            <div className="px-3 pt-3">
              <label className="flex h-[30px] items-center gap-2 rounded-[8px] bg-[#f5f5f5] px-2.5">
                <Search className="size-4 shrink-0 text-[#8a8a8a]" strokeWidth={1.7} aria-hidden />
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={t('work.inbox.search')}
                  aria-label={t('work.inbox.search')}
                  className="h-full min-w-0 flex-1 bg-transparent text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a]"
                />
              </label>
            </div>
            <nav className="mt-2 flex min-h-0 flex-1 flex-col overflow-y-auto px-2 pb-2">
              <div className="flex flex-col gap-0.5">
                {BOXES.map((item) => (
                  <NavRow
                    key={item}
                    icon={BOX_ICON[item]}
                    label={t(`work.inbox.box.${item}`)}
                    count={counts[item]}
                    emphasize={item === 'todo' && counts[item] > 0}
                    active={box === item}
                    ui={`work.inbox.box.${item}`}
                    onClick={() => chooseBox(item)}
                  />
                ))}
              </div>
              <div className="px-2 pt-4 pb-1 text-[12px] text-[#8a8a8a]">{t('work.inbox.nav.kinds')}</div>
              <div className="flex flex-col gap-0.5">
                {KIND_NAV.map((row) => (
                  <NavRow
                    key={row.labelKey}
                    icon={row.icon}
                    label={t(row.labelKey)}
                    active={kind === row.kind}
                    onClick={() => chooseKind(row.kind)}
                  />
                ))}
              </div>
            </nav>
            <div className="border-t border-[#efefef] p-2">
              <button
                type="button"
                onClick={() => setNavOpen(false)}
                className="flex h-9 w-full items-center gap-2.5 rounded-[8px] px-2.5 text-[13px] text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]">
                <PanelLeftClose className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
                {t('work.inbox.nav.collapse')}
              </button>
            </div>
          </aside>
        ) : null}
        <section className="flex min-w-0 flex-1 flex-col bg-white">
          <div className="relative z-10 flex h-12 shrink-0 items-center gap-1 px-3">
            {navOpen ? null : (
              <button
                type="button"
                onClick={() => setNavOpen(true)}
                aria-label={t('work.inbox.nav.expand')}
                className="grid size-9 shrink-0 place-items-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]">
                <PanelLeftOpen className="size-4" strokeWidth={1.7} aria-hidden />
              </button>
            )}
            <FilterMenu
              label={t('work.inbox.filter.kind')}
              value={kind}
              options={kindOptions}
              ui="work.inbox.filter.kind"
              onChange={chooseKind}
            />
            <FilterMenu
              label={t('work.inbox.filter.time')}
              value={days}
              options={timeOptions}
              ui="work.inbox.filter.time"
              onChange={setDays}
            />
            <FilterMenu
              label={t('work.inbox.filter.department')}
              value={department}
              options={departmentOptions}
              ui="work.inbox.filter.department"
              onChange={setDepartment}
            />
            <FilterMenu
              label={t('work.inbox.filter.status')}
              value={status}
              options={statusOptions}
              ui="work.inbox.filter.status"
              onChange={setStatus}
            />
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {error ? <p className="px-4 py-2 text-[13px] text-[#b42318]">{error}</p> : null}
            {loading && items.length === 0 ? (
              <p className="px-4 py-6 text-[13px] text-[#8a8a8a]">{t('common.loading')}</p>
            ) : null}
            {!loading && items.length === 0 ? (
              <p className="px-4 py-6 text-[13px] text-[#8a8a8a]">{t('work.inbox.empty')}</p>
            ) : null}
            <ul className="flex flex-col gap-2 px-4 pb-4">
              {items.map((item) => {
                const on = selectedId === item.id
                return (
                  <li key={item.id}>
                    <button
                      type="button"
                      data-ui="work.inbox.card"
                      data-id={item.id}
                      onClick={() => openItem(item.id)}
                      className={`flex w-full flex-col gap-1 rounded-[10px] border px-4 py-3 text-left ${
                        on ? 'border-[#171717] bg-white' : 'border-[#ececec] bg-white hover:bg-[#f7f7f7]'
                      }`}>
                      <span className="flex items-start justify-between gap-3">
                        <span className="min-w-0 flex-1 truncate text-[15px] font-medium text-[#171717]">
                          {item.title}
                        </span>
                        <span className={statusClass(item.status)}>{t(`work.inbox.status.${item.status}`)}</span>
                      </span>
                      <span className="block truncate text-[13px] text-[#3f3f3f]">
                        {t('work.inbox.detail.title')}
                        {item.summary || item.title}
                      </span>
                      <span className="block truncate text-[13px] text-[#3f3f3f]">
                        {t('work.inbox.detail.submitted')}
                        {formatStamp(item.created_at)}
                      </span>
                      {item.engine ? (
                        <>
                          <span className="block truncate text-[13px] text-[#3f3f3f]">
                            {t('work.inbox.detail.node')}
                            {item.current_node}
                          </span>
                          <ProcessTrail item={item} />
                        </>
                      ) : showsApprovalFlow(item.kind) ? (
                        <ApprovalFlowDiagram
                          status={item.status}
                          layout="row"
                          assigneeNames={item.assignee_names}
                          deciderName={item.decider_name}
                          decidedAt={item.decided_at}
                          comment={item.decision_comment}
                        />
                      ) : (
                        <span className="block truncate text-[13px] text-[#3f3f3f]">
                          {t('work.inbox.detail.node')}
                          {item.current_node}
                        </span>
                      )}
                      <span className="mt-1 flex items-center gap-2 text-[12px] text-[#8a8a8a]">
                        <span
                          aria-hidden
                          className="grid size-6 shrink-0 place-items-center rounded-[8px] bg-[#f2f3f5] text-[11px] font-medium text-[#171717]">
                          {nameMark(item.initiator_name)}
                        </span>
                        {item.initiator_name}
                      </span>
                    </button>
                  </li>
                )
              })}
            </ul>
          </div>
        </section>
        {detailItem ? (
          <aside
            data-ui="work.inbox.detail.drawer"
            data-state={detailPresence.state}
            aria-hidden={detailPresence.state === 'closed' ? true : undefined}
            inert={detailPresence.state === 'closed' ? true : undefined}
            className={`work-inbox-detail-slide absolute inset-y-0 right-0 z-20 flex w-[320px] flex-col border-l border-[#efefef] bg-white shadow-[-12px_0_32px_rgba(23,23,23,0.12)]${
              detailPresence.state === 'closed' ? ' pointer-events-none' : ''
            }`}>
            <div data-ui="work.inbox.detail" className="flex h-full min-h-0 w-full flex-col px-5 py-5">
              <div className="min-h-0 flex-1 overflow-y-auto">
                <div className="flex items-center gap-2">
                  <h3 className="min-w-0 flex-1 text-[15px] font-medium text-[#171717]">{detailItem.title}</h3>
                  <span className={statusClass(detailItem.status)}>{t(`work.inbox.status.${detailItem.status}`)}</span>
                  <button
                    type="button"
                    onClick={() => setDetailOpen(false)}
                    aria-label={t('common.close')}
                    className="inline-flex size-6 shrink-0 items-center justify-center rounded-[6px] p-0 leading-none text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
                    <X size={14} className="block size-3.5 shrink-0" strokeWidth={1.7} aria-hidden />
                  </button>
                </div>
                {detailItem.engine ? (
                  <ProcessTrail item={detailItem} />
                ) : showsApprovalFlow(detailItem.kind) ? (
                  <ApprovalFlowDiagram
                    status={detailItem.status}
                    layout="column"
                    assigneeNames={detailItem.assignee_names}
                    deciderName={detailItem.decider_name}
                    decidedAt={detailItem.decided_at}
                    comment={detailItem.decision_comment}
                  />
                ) : null}
                <dl className="mt-4 space-y-3 text-[13px] text-[#3f3f3f]">
                  {detailRows(detailItem, t).map((row) => (
                    <div key={row.label}>
                      <dt className="text-[12px] text-[#8a8a8a]">{row.label}</dt>
                      <dd className="mt-0.5 break-words whitespace-pre-line">{row.value}</dd>
                    </div>
                  ))}
                  {detailItem.fields.schedule === 'unwritten' || detailItem.schedule_retry ? (
                    <div>
                      <dt className="text-[12px] text-[#8a8a8a]">{t('work.inbox.schedule')}</dt>
                      <dd className="mt-0.5">{t('work.inbox.schedule.unwritten')}</dd>
                    </div>
                  ) : null}
                  {detailItem.decision_comment ? (
                    <div>
                      <dt className="text-[12px] text-[#8a8a8a]">{t('work.inbox.comment')}</dt>
                      <dd className="mt-0.5 break-words whitespace-pre-line">{detailItem.decision_comment}</dd>
                    </div>
                  ) : null}
                  <div>
                    <dt className="text-[12px] text-[#8a8a8a]">{t('work.inbox.detail.submitted')}</dt>
                    <dd className="mt-0.5">{formatStamp(detailItem.created_at)}</dd>
                  </div>
                  {showsApprovalFlow(detailItem.kind) && !detailItem.engine ? null : (
                    <div>
                      <dt className="text-[12px] text-[#8a8a8a]">{t('work.inbox.detail.node')}</dt>
                      <dd className="mt-0.5">{detailItem.current_node}</dd>
                    </div>
                  )}
                  <div className="flex items-center gap-2">
                    <span
                      aria-hidden
                      className="grid size-6 shrink-0 place-items-center rounded-[8px] bg-[#f2f3f5] text-[11px] font-medium text-[#171717]">
                      {nameMark(detailItem.initiator_name)}
                    </span>
                    <span>
                      {detailItem.initiator_name}
                      {detailItem.org_name ? ` · ${detailItem.org_name}` : ''}
                    </span>
                  </div>
                </dl>
                {error ? <p className="mt-3 text-[13px] text-[#b42318]">{error}</p> : null}
              </div>
              {detailItem.approvable || detailItem.rejectable || detailItem.cancellable ? (
                <label className="mt-4 block text-[12px] text-[#8a8a8a]">
                  {t('work.inbox.comment')}
                  <textarea
                    data-ui="work.inbox.comment"
                    aria-label={t('work.inbox.comment')}
                    maxLength={500}
                    value={comment}
                    onChange={(event) => setComment(event.target.value)}
                    className="mt-1 h-20 w-full resize-none rounded-[8px] border-0 bg-[#f5f5f5] px-3 py-2 text-[13px] text-[#171717] shadow-none outline-none placeholder:text-[#8a8a8a] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset"
                  />
                </label>
              ) : null}
              {detailItem.schedule_retry ? (
                <button
                  type="button"
                  data-ui="work.inbox.schedule.retry"
                  disabled={busy}
                  onClick={() => {
                    if (busy) return
                    setBusy(true)
                    setError('')
                    void retrySchedule(detailItem.id)
                      .then(() => refresh())
                      .catch((cause: unknown) => {
                        const message = cause instanceof Error ? cause.message : ''
                        setError(
                          explainError(
                            message,
                            t('work.inbox.error'),
                            t('work.inbox.error.approver'),
                            t('work.inbox.form.org')
                          )
                        )
                      })
                      .finally(() => setBusy(false))
                  }}
                  className="mt-3 h-9 rounded-[8px] border border-[#e5e5e5] bg-white text-[13px] text-[#171717] hover:bg-[#fafafa] disabled:opacity-40">
                  {t('work.inbox.schedule.retry')}
                </button>
              ) : null}
              <div className="mt-auto flex gap-2 pt-6">
                {detailItem.approvable === true || detailItem.rejectable === true ? (
                  <>
                    {detailItem.rejectable === true ? (
                      <PopConfirm
                        className="flex flex-1"
                        title={t('work.inbox.confirm.reject')}
                        confirmLabel={t('work.inbox.action.reject')}
                        cancelLabel={t('common.cancel')}
                        tone="danger"
                        disabled={busy || !commentReady}
                        onConfirm={() => decide(detailItem, 'reject')}>
                        <button
                          type="button"
                          data-ui="work.inbox.reject"
                          disabled={busy || !commentReady}
                          className="h-9 flex-1 rounded-[8px] border border-[#e5e5e5] bg-white text-[13px] text-[#171717] hover:bg-[#fafafa] disabled:opacity-40">
                          {t('work.inbox.action.reject')}
                        </button>
                      </PopConfirm>
                    ) : null}
                    {detailItem.approvable === true ? (
                      <PopConfirm
                        className="flex flex-1"
                        title={t('work.inbox.confirm.approve')}
                        confirmLabel={t('work.inbox.action.approve')}
                        cancelLabel={t('common.cancel')}
                        tone="default"
                        disabled={busy || !commentReady}
                        onConfirm={() => decide(detailItem, 'approve')}>
                        <button
                          type="button"
                          data-ui="work.inbox.approve"
                          disabled={busy || !commentReady}
                          className="h-9 flex-1 rounded-[8px] bg-[#171717] text-[13px] text-white disabled:opacity-40">
                          {t('work.inbox.action.approve')}
                        </button>
                      </PopConfirm>
                    ) : null}
                  </>
                ) : null}
                {detailItem.cancellable ? (
                  <PopConfirm
                    className="flex flex-1"
                    title={t('work.inbox.confirm.cancel')}
                    confirmLabel={t('work.inbox.action.cancel')}
                    cancelLabel={t('common.cancel')}
                    tone="default"
                    disabled={busy || !commentReady}
                    onConfirm={() => decide(detailItem, 'cancel')}>
                    <button
                      type="button"
                      data-ui="work.inbox.cancel"
                      disabled={busy || !commentReady}
                      className="h-9 flex-1 rounded-[8px] border border-[#e5e5e5] bg-white text-[13px] text-[#171717] hover:bg-[#fafafa] disabled:opacity-40">
                      {t('work.inbox.action.cancel')}
                    </button>
                  </PopConfirm>
                ) : null}
              </div>
            </div>
          </aside>
        ) : null}
      </div>
    </div>
  )
}

type FilterOption<T extends string | number> = { value: T; label: string }

function FilterMenu<T extends string | number>({
  label,
  value,
  options,
  ui,
  onChange
}: {
  label: string
  value: T
  options: FilterOption<T>[]
  ui: string
  onChange: (value: T) => void
}) {
  const [open, setOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)
  const current = options.find((option) => option.value === value)?.label ?? label

  useEffect(() => {
    if (!open) return
    const onPointer = (event: PointerEvent) => {
      const target = event.target
      if (target instanceof Node && rootRef.current?.contains(target)) return
      setOpen(false)
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      setOpen(false)
    }
    document.addEventListener('pointerdown', onPointer, true)
    document.addEventListener('keydown', onKey, true)
    return () => {
      document.removeEventListener('pointerdown', onPointer, true)
      document.removeEventListener('keydown', onKey, true)
    }
  }, [open])

  return (
    <div ref={rootRef} className="relative">
      <button
        type="button"
        data-ui={ui}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={current}
        onClick={() => setOpen((next) => !next)}
        className="flex h-[30px] items-center gap-1 rounded-[8px] px-2 text-[13px] text-[#171717] hover:bg-[#f7f7f7]">
        <span>{current}</span>
        <ChevronDown className={`size-3.5 text-[#8a8a8a] ${open ? 'rotate-180' : ''}`} strokeWidth={1.7} aria-hidden />
      </button>
      {open ? (
        <div
          role="listbox"
          aria-label={label}
          data-ui={`${ui}.menu`}
          className="absolute top-full left-0 z-20 mt-1 min-w-[168px] rounded-[8px] border border-[#ececec] bg-white p-1 shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
          {options.map((option) => {
            const selected = option.value === value
            return (
              <button
                key={String(option.value)}
                type="button"
                role="option"
                aria-selected={selected}
                onClick={() => {
                  onChange(option.value)
                  setOpen(false)
                }}
                className={`flex h-9 w-full items-center gap-2 rounded-[8px] px-2 text-left text-[13px] text-[#171717] hover:bg-[#f7f7f7] ${
                  selected ? 'bg-[#f2f3f5] font-medium' : ''
                }`}>
                <Check className={`size-3.5 shrink-0 ${selected ? '' : 'opacity-0'}`} strokeWidth={1.7} aria-hidden />
                <span className="min-w-0 truncate">{option.label}</span>
              </button>
            )
          })}
        </div>
      ) : null}
    </div>
  )
}

function NavRow({
  icon: Icon,
  label,
  count,
  emphasize = false,
  active,
  ui,
  onClick
}: {
  icon: LucideIcon
  label: string
  count?: number
  emphasize?: boolean
  active: boolean
  ui?: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      data-ui={ui}
      aria-current={active ? 'page' : undefined}
      onClick={onClick}
      className={`flex h-9 w-full items-center gap-2 rounded-[8px] px-2 text-[13px] ${
        active ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
      }`}>
      <Icon className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
      <span className="min-w-0 flex-1 truncate text-left">{label}</span>
      {count === undefined ? null : (
        <span className={`text-[12px] tabular-nums ${emphasize ? 'font-medium text-[#171717]' : 'text-[#8a8a8a]'}`}>
          {count}
        </span>
      )}
    </button>
  )
}
