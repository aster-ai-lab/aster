import {
  ConfirmDialog,
  Dialog,
  DialogContent,
  DialogPortalContainerProvider,
  DialogTitle,
  SegmentedControl
} from '@a3s-lab/ui'
import { ChevronLeft, ChevronRight, Plus } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import {
  isSessionDenied,
  listMemberOrgs,
  orgAllows,
  PERSONAL_CENTER,
  readCenterId,
  returnToLogin,
  subscribeCenter
} from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import { filledButtonClassName, quietButtonClassName } from '../org/orgUi'
import {
  createScheduleEvent,
  deleteScheduleEvent,
  listScheduleEvents,
  updateScheduleEvent,
  type ScheduleGuest,
  type ScheduleEvent
} from './scheduleApi'
import { ScheduleForm, type ScheduleDraftState } from './ScheduleForm'
import { instantsFromDraft, localTimeZone, spanFromEvent } from './scheduleRule'
import {
  addMonths,
  dateInputValue,
  daysInRange,
  defaultSlot,
  gmtLabel,
  HOUR_PX,
  placeOnDay,
  rangeLabel,
  sameDay,
  shiftAnchor,
  slotFromOffset,
  startOfDay,
  startOfMonth,
  startOfWeek,
  timeInputValue,
  visibleRange,
  type ScheduleMode
} from './scheduleTime'

const WEEKDAYS = [0, 1, 2, 3, 4, 5, 6]

function draftFromRange(start: Date, end: Date, event?: ScheduleEvent): ScheduleDraftState {
  const zone = event?.time_zone || localTimeZone()
  const fromEvent = event
    ? spanFromEvent({
        starts_at: event.starts_at,
        ends_at: event.ends_at,
        all_day: event.all_day === true,
        time_zone: zone
      })
    : null
  return {
    id: event?.id ?? null,
    title: event?.title ?? '',
    location: event?.location ?? '',
    note: event?.note ?? '',
    minutes: event?.minutes ?? '',
    startDate: fromEvent?.startDate ?? dateInputValue(start),
    startTime: fromEvent?.startTime ?? timeInputValue(start),
    endDate: fromEvent?.endDate ?? dateInputValue(end),
    endTime: fromEvent?.endTime ?? timeInputValue(end),
    allDay: event?.all_day === true,
    timeZone: zone,
    repeatRule: event?.repeat_rule || 'none',
    guestPermission: event?.guest_permission === 'invite' ? 'invite' : 'invite_notes',
    reminders: event?.reminders ? [...event.reminders] : [5],
    guests: (event?.guests ?? []).map((guest) => ({
      member_id: guest.member_id,
      email: guest.email,
      label: guest.email || guest.member_id || ''
    })),
    visibility: event?.visibility === 'public' || event?.visibility === 'private' ? event.visibility : 'default',
    availability: event?.availability === 'free' ? 'free' : 'busy',
    error: ''
  }
}

function MonthDots({
  cursor,
  selected,
  onCursor,
  onPick
}: {
  cursor: Date
  selected: Date
  onCursor: (next: Date) => void
  onPick: (day: Date) => void
}) {
  const { t, i18n } = useTranslation()
  const language = i18n.resolvedLanguage || i18n.language
  const cells = daysInRange(startOfWeek(startOfMonth(cursor)), visibleRange('month', cursor).end)
  const title = new Intl.DateTimeFormat(language, { year: 'numeric', month: 'long' }).format(cursor)
  const weekday = new Intl.DateTimeFormat(language, { weekday: 'narrow' })
  return (
    <div data-ui="schedule.mini">
      <div className="mb-2 flex items-center justify-between">
        <p className="m-0 text-[13px] font-medium text-[#171717] dark:text-[#f5f5f5]">{title}</p>
        <div className="flex items-center">
          <button
            type="button"
            className={quietButtonClassName(30)}
            aria-label={t('schedule.prev')}
            onClick={() => onCursor(addMonths(cursor, -1))}>
            <ChevronLeft size={16} strokeWidth={1.7} />
          </button>
          <button
            type="button"
            className={cn(quietButtonClassName(30), 'ml-1')}
            aria-label={t('schedule.next')}
            onClick={() => onCursor(addMonths(cursor, 1))}>
            <ChevronRight size={16} strokeWidth={1.7} />
          </button>
        </div>
      </div>
      <div className="grid grid-cols-7 text-center text-[11px] text-[#8a8a8a]">
        {WEEKDAYS.map((offset) => (
          <span key={offset}>{weekday.format(new Date(2026, 9, 4 + offset))}</span>
        ))}
      </div>
      <div className="mt-1 grid grid-cols-7 gap-y-0.5">
        {cells.map((day) => {
          const outside = day.getMonth() !== cursor.getMonth()
          const today = sameDay(day, new Date())
          const active = sameDay(day, selected)
          return (
            <button
              key={day.toDateString()}
              type="button"
              onClick={() => onPick(day)}
              className={cn(
                'mx-auto flex size-7 items-center justify-center rounded-full text-[12px]',
                outside ? 'text-[#8a8a8a]' : 'text-[#171717] dark:text-[#f5f5f5]',
                active && 'bg-[#f2f3f5] font-medium dark:bg-[#2c2c2c]',
                today && !active && 'font-medium ring-1 ring-[#171717] ring-inset dark:ring-[#f5f5f5]'
              )}>
              {day.getDate()}
            </button>
          )
        })}
      </div>
    </div>
  )
}

function EventBlock({
  event,
  top,
  height,
  lane,
  lanes,
  onOpen
}: {
  event: ScheduleEvent
  top: number
  height: number
  lane: number
  lanes: number
  onOpen: (event: ScheduleEvent) => void
}) {
  const start = new Date(event.starts_at)
  const end = new Date(event.ends_at)
  const when = `${timeInputValue(start)}–${timeInputValue(end)}`
  return (
    <button
      type="button"
      data-ui="schedule.event"
      data-event-id={event.id}
      onClick={(pointer) => {
        pointer.stopPropagation()
        onOpen(event)
      }}
      className={cn(
        'absolute overflow-hidden rounded-[6px] border-l-[3px] px-1.5 py-1 text-left',
        event.availability === 'free'
          ? 'border-dashed border-[#cfcfcf] bg-transparent'
          : 'border-[#171717] bg-[#f2f3f5] dark:border-[#f5f5f5] dark:bg-[#2c2c2c]'
      )}
      style={{
        top,
        height,
        left: `calc(${(lane / lanes) * 100}% + 2px)`,
        width: `calc(${100 / lanes}% - 4px)`
      }}>
      <span className="block truncate text-[12px] font-medium text-[#171717] dark:text-[#f5f5f5]">{event.title}</span>
      <span className="block truncate text-[11px] text-[#3f3f3f] dark:text-[#b0b4ba]">
        {when}
        {event.location ? ` ${event.location}` : ''}
      </span>
    </button>
  )
}

export default function SchedulePage() {
  const { t, i18n } = useTranslation()
  const language = i18n.resolvedLanguage || i18n.language
  const [orgId, setOrgId] = useState(readCenterId)
  const { data: orgs, error: orgsError } = useSWR('studio/member-orgs', () => listMemberOrgs())
  const org = orgsError ? undefined : orgs?.find((item) => item.id === orgId)
  const canRead = org?.canReadSchedule === true && orgAllows(org, 'read', 'ScheduleEvent')
  const canCreate = org?.canCreateSchedule === true && orgAllows(org, 'create', 'ScheduleEvent')
  const canUpdate = org?.canUpdateSchedule === true && orgAllows(org, 'update', 'ScheduleEvent')
  const canDelete = org?.canDeleteSchedule === true && orgAllows(org, 'delete', 'ScheduleEvent')
  const [mode, setMode] = useState<ScheduleMode>('week')
  const [anchor, setAnchor] = useState(() => startOfDay(new Date()))
  const [mini, setMini] = useState(() => startOfMonth(new Date()))
  const [events, setEvents] = useState<ScheduleEvent[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [reloadKey, setReloadKey] = useState(0)
  const readyRef = useRef(false)
  const [trackedOrg, setTrackedOrg] = useState(orgId)
  if (trackedOrg !== orgId) {
    setTrackedOrg(orgId)
    readyRef.current = false
    setEvents([])
    setError('')
  }
  const [draft, setDraft] = useState<ScheduleDraftState | null>(null)
  const [saving, setSaving] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [now, setNow] = useState(() => new Date())
  const scroller = useRef<HTMLDivElement>(null)
  const eventsRef = useRef(events)
  const requestRef = useRef(0)
  const writeEpoch = useRef(0)
  const openedAuthority = useRef('')
  const policyKey = [orgId, canRead, canCreate, canUpdate, canDelete].join('|')
  const livePolicy = useRef(policyKey)
  livePolicy.current = policyKey
  const draftEvent = events.find((event) => event.id === draft?.id)
  const draftAuthority = `${policyKey}|${draftEvent?.can_update === true}|${draftEvent?.can_delete === true}`
  eventsRef.current = events

  useEffect(() => subscribeCenter(() => setOrgId(readCenterId())), [])
  useEffect(() => {
    setDraft(null)
    setConfirmDelete(false)
    writeEpoch.current += 1
    setSaving(false)
  }, [orgId, canRead, canCreate, canUpdate, canDelete])
  useEffect(() => {
    if (draft && openedAuthority.current !== draftAuthority) {
      setDraft(null)
      setConfirmDelete(false)
      writeEpoch.current += 1
      setSaving(false)
    }
  }, [draft, draftAuthority])
  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 60_000)
    return () => window.clearInterval(timer)
  }, [])
  useEffect(() => {
    setMini(startOfMonth(anchor))
  }, [anchor])

  const range = useMemo(() => visibleRange(mode, anchor), [mode, anchor])
  const columns = useMemo(() => (mode === 'month' ? [] : daysInRange(range.start, range.end)), [mode, range])

  const load = useCallback(async () => {
    const ticket = ++requestRef.current
    if (orgId === PERSONAL_CENTER || !canRead) {
      setEvents([])
      setLoading(false)
      return
    }
    // The first paint can say it is loading. A date change keeps the grid and stays quiet.
    if (!readyRef.current) setLoading(true)
    try {
      const rows = await listScheduleEvents(orgId, range.start, range.end)
      if (ticket !== requestRef.current) return
      setEvents(rows)
      setError('')
      readyRef.current = true
    } catch (caught) {
      if (ticket !== requestRef.current) return
      if (isSessionDenied(caught)) {
        returnToLogin()
        return
      }
      const message = visibleScheduleError(caught, t('schedule.error'))
      setError(message)
      setEvents([])
      if (eventsRef.current.length > 0) toast.error(message)
    } finally {
      if (ticket === requestRef.current) setLoading(false)
    }
  }, [orgId, canRead, range.start, range.end, reloadKey, t])

  useEffect(() => {
    void load()
    return () => {
      requestRef.current += 1
    }
  }, [load])

  useEffect(() => {
    if (mode === 'month' || orgId === PERSONAL_CENTER) return
    const node = scroller.current
    if (!node) return
    node.scrollTop = 8 * HOUR_PX
  }, [mode, orgId])

  const timed = useMemo(
    () =>
      events.flatMap((event) => {
        const start = new Date(event.starts_at)
        const end = new Date(event.ends_at)
        if (!Number.isFinite(start.getTime()) || !Number.isFinite(end.getTime())) return []
        return [{ id: `${event.id}:${event.starts_at}`, start, end, event }]
      }),
    [events]
  )

  function openDraft(start: Date, end: Date, event?: ScheduleEvent) {
    if (!canRead || (!event && !canCreate)) return
    setConfirmDelete(false)
    writeEpoch.current += 1
    openedAuthority.current = `${policyKey}|${event?.can_update === true}|${event?.can_delete === true}`
    setDraft(draftFromRange(start, end, event))
  }

  function openEvent(event: ScheduleEvent) {
    openDraft(new Date(event.starts_at), new Date(event.ends_at), event)
  }

  async function saveDraft() {
    if (
      !draft ||
      !canRead ||
      (draft.id ? !canUpdate || events.find((event) => event.id === draft.id)?.can_update !== true : !canCreate)
    )
      return
    const title = draft.title.trim()
    if (!title) {
      setDraft({ ...draft, error: t('schedule.form.required') })
      return
    }
    const span = instantsFromDraft(draft)
    if (!span) {
      setDraft({ ...draft, error: t('schedule.form.range') })
      return
    }
    const start = new Date(span.starts_at)
    const body = {
      title,
      location: draft.location.trim(),
      note: draft.note.trim(),
      minutes: draft.minutes.trim(),
      starts_at: span.starts_at,
      ends_at: span.ends_at,
      all_day: draft.allDay,
      time_zone: draft.timeZone,
      utc_offset_minutes: span.utc_offset_minutes,
      repeat_rule: draft.repeatRule || 'none',
      visibility: draft.visibility,
      availability: draft.availability,
      guest_permission: draft.guestPermission,
      reminders: draft.reminders,
      guests: draft.guests.flatMap<ScheduleGuest>((guest) => {
        if (guest.member_id) return [{ member_id: guest.member_id }]
        if (guest.email) return [{ email: guest.email }]
        return []
      })
    }
    const epoch = writeEpoch.current
    const policy = policyKey
    setSaving(true)
    try {
      if (draft.id) await updateScheduleEvent(orgId, draft.id, body)
      else await createScheduleEvent(orgId, body)
      if (epoch !== writeEpoch.current || policy !== livePolicy.current) return
      toast.success(t('schedule.saved'))
      setDraft(null)
      setAnchor(startOfDay(start))
      setReloadKey((value) => value + 1)
    } catch (caught) {
      if (epoch !== writeEpoch.current || policy !== livePolicy.current) return
      if (isSessionDenied(caught)) {
        returnToLogin()
        return
      }
      setDraft({ ...draft, error: visibleScheduleError(caught, t('schedule.error')) })
    } finally {
      if (epoch === writeEpoch.current) setSaving(false)
    }
  }

  async function removeDraft() {
    if (!draft?.id || !canRead || !canDelete || events.find((event) => event.id === draft.id)?.can_delete !== true)
      return false
    const epoch = writeEpoch.current
    const policy = policyKey
    setSaving(true)
    try {
      await deleteScheduleEvent(orgId, draft.id)
      if (epoch !== writeEpoch.current || policy !== livePolicy.current) return false
      toast.success(t('schedule.deleted'))
      setDraft(null)
      setReloadKey((value) => value + 1)
      return true
    } catch (caught) {
      if (epoch !== writeEpoch.current || policy !== livePolicy.current) return false
      if (isSessionDenied(caught)) {
        returnToLogin()
        return false
      }
      setDraft({ ...draft, error: visibleScheduleError(caught, t('schedule.error')) })
      return false
    } finally {
      if (epoch === writeEpoch.current) setSaving(false)
    }
  }

  const portal = typeof document === 'undefined' ? null : document.body
  const weekday = new Intl.DateTimeFormat(language, { weekday: 'short' })

  let body: ReactNode
  if (orgId === PERSONAL_CENTER) {
    body = (
      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-16">
        <p data-ui="schedule.empty" className="m-0 text-center text-[13px] text-[#8a8a8a]">
          {t('schedule.empty.org')}
        </p>
      </div>
    )
  } else if (!canRead) {
    body = (
      <p role="status" className="m-0 px-6 py-16 text-center text-[13px] text-[#8a8a8a]">
        {t('schedule.noAccess')}
      </p>
    )
  } else if (error && events.length === 0) {
    body = (
      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-16 text-center">
        <p className="m-0 text-[13px] text-[#171717] dark:text-[#f5f5f5]">{error}</p>
        <button
          type="button"
          className={cn(quietButtonClassName(36), 'mt-4')}
          onClick={() => setReloadKey((value) => value + 1)}>
          {t('schedule.retry')}
        </button>
      </div>
    )
  } else if (mode === 'month') {
    const cells = daysInRange(range.start, range.end)
    body = (
      <div data-ui="schedule.grid" className="min-h-0 flex-1 overflow-auto">
        <div className="grid grid-cols-7 border-b border-[#efefef] text-center text-[12px] text-[#8a8a8a] dark:border-[#3a3a3a]">
          {WEEKDAYS.map((offset) => (
            <span key={offset} className="py-2">
              {weekday.format(new Date(2026, 9, 4 + offset))}
            </span>
          ))}
        </div>
        <div className="grid grid-cols-7">
          {cells.map((day) => {
            const onDay = timed.filter((item) => item.start < addDayEnd(day) && item.end > startOfDay(day))
            return (
              <div
                key={day.toDateString()}
                className="min-h-[88px] border-r border-b border-[#efefef] p-1 min-[900px]:min-h-[120px] dark:border-[#3a3a3a]">
                <button
                  type="button"
                  onClick={() => {
                    setMode('day')
                    setAnchor(startOfDay(day))
                  }}
                  className={cn(
                    'mb-1 flex size-6 items-center justify-center rounded-full text-[12px]',
                    day.getMonth() === anchor.getMonth() ? 'text-[#171717] dark:text-[#f5f5f5]' : 'text-[#8a8a8a]',
                    sameDay(day, now) && 'bg-[#f2f3f5] font-medium dark:bg-[#2c2c2c]'
                  )}>
                  {day.getDate()}
                </button>
                {onDay.slice(0, 3).map((item) => (
                  <button
                    key={`${item.event.id}:${item.event.starts_at}`}
                    type="button"
                    data-ui="schedule.event"
                    onClick={() => openEvent(item.event)}
                    className="mb-0.5 block w-full truncate rounded-[4px] bg-[#f2f3f5] px-1 text-left text-[11px] text-[#171717] dark:bg-[#2c2c2c] dark:text-[#f5f5f5]">
                    {item.event.title}
                  </button>
                ))}
                {onDay.length > 3 ? <span className="px-1 text-[11px] text-[#8a8a8a]">+{onDay.length - 3}</span> : null}
              </div>
            )
          })}
        </div>
      </div>
    )
  } else {
    const todayIndex = columns.findIndex((day) => sameDay(day, now))
    const minutes = now.getHours() * 60 + now.getMinutes()
    body = (
      <div ref={scroller} data-ui="schedule.grid" className="min-h-0 min-w-0 flex-1 overflow-auto">
        <div className="sticky top-0 z-10 flex border-b border-[#efefef] bg-white dark:border-[#3a3a3a] dark:bg-[#111112]">
          <div className="w-14 shrink-0 px-2 py-2 text-[11px] text-[#8a8a8a]">{gmtLabel(now)}</div>
          {columns.map((day) => (
            <button
              key={day.toDateString()}
              type="button"
              onClick={() => {
                setMode('day')
                setAnchor(startOfDay(day))
              }}
              className="min-w-0 flex-1 py-2 text-center">
              <span className="block text-[12px] text-[#8a8a8a]">{weekday.format(day)}</span>
              <span
                className={cn(
                  'mx-auto mt-0.5 flex size-7 items-center justify-center rounded-full text-[14px]',
                  sameDay(day, now)
                    ? 'bg-[#f2f3f5] font-medium text-[#171717] dark:bg-[#2c2c2c] dark:text-[#f5f5f5]'
                    : 'text-[#171717] dark:text-[#f5f5f5]'
                )}>
                {day.getDate()}
              </span>
            </button>
          ))}
        </div>
        {events.some((event) => event.all_day) ? (
          <div className="flex border-t border-[#efefef] dark:border-[#3a3a3a]">
            <div className="w-14 shrink-0" />
            {columns.map((day) => (
              <div key={day.toDateString()} className="flex min-w-0 flex-1 flex-col gap-0.5 px-0.5 py-0.5">
                {events
                  .filter((event) => event.all_day && coversDay(event, day))
                  .map((event) => (
                    <button
                      key={`${event.id}:${event.starts_at}`}
                      type="button"
                      data-ui="schedule.event"
                      onClick={() => openEvent(event)}
                      className="truncate rounded-[4px] bg-[#f2f3f5] px-1 text-left text-[11px] text-[#171717] dark:bg-[#2c2c2c] dark:text-[#f5f5f5]">
                      {event.title}
                    </button>
                  ))}
              </div>
            ))}
          </div>
        ) : null}
        <div className="flex" style={{ height: 24 * HOUR_PX }}>
          <div className="relative w-14 shrink-0">
            {Array.from({ length: 24 }, (_, hour) => (
              <span
                key={hour}
                className="absolute right-2 -translate-y-1/2 text-[11px] text-[#8a8a8a]"
                style={{ top: hour * HOUR_PX }}>
                {String(hour).padStart(2, '0')}:00
              </span>
            ))}
          </div>
          <div className="relative flex min-w-0 flex-1">
            {columns.map((day) => {
              const placed = placeOnDay(
                timed.filter((item) => !item.event.all_day),
                day
              )
              return (
                <div
                  key={day.toDateString()}
                  className="relative min-w-0 flex-1 border-l border-[#efefef] dark:border-[#3a3a3a]"
                  onClick={(pointer) => {
                    if ((pointer.target as HTMLElement).closest('[data-ui="schedule.event"]')) return
                    const bounds = pointer.currentTarget.getBoundingClientRect()
                    const slot = slotFromOffset(day, pointer.clientY - bounds.top)
                    openDraft(slot.start, slot.end)
                  }}>
                  {Array.from({ length: 24 }, (_, hour) => (
                    <div
                      key={hour}
                      className="border-t border-[#efefef] dark:border-[#3a3a3a]"
                      style={{ height: HOUR_PX }}
                    />
                  ))}
                  {placed.map((slice) => {
                    const event = timed.find((item) => item.id === slice.id)?.event
                    if (!event) return null
                    return (
                      <EventBlock
                        key={`${event.id}:${event.starts_at}`}
                        event={event}
                        top={slice.top}
                        height={slice.height}
                        lane={slice.lane}
                        lanes={slice.lanes}
                        onOpen={openEvent}
                      />
                    )
                  })}
                </div>
              )
            })}
            {todayIndex >= 0 ? (
              <div
                className="pointer-events-none absolute right-0 left-0 z-[1]"
                style={{ top: (minutes / 60) * HOUR_PX }}>
                <div className="h-px bg-[#171717] dark:bg-[#f5f5f5]" />
                <span
                  className="absolute size-2 -translate-y-1/2 rounded-full bg-[#171717] dark:bg-[#f5f5f5]"
                  style={{ left: `calc(${(todayIndex / columns.length) * 100}% - 4px)` }}
                />
              </div>
            ) : null}
          </div>
        </div>
        {!loading && events.length === 0 ? (
          <p className="m-0 px-4 py-3 text-[12px] text-[#8a8a8a]">{t('schedule.empty.range')}</p>
        ) : null}
      </div>
    )
  }

  return (
    <div
      data-ui="schedule.view"
      className="flex h-full min-h-0 w-full flex-1 flex-col overflow-hidden bg-white text-[#171717] dark:bg-[#111112] dark:text-[#f5f5f5]">
      <header className="flex shrink-0 flex-wrap items-center gap-2 border-b border-[#efefef] px-4 py-3 dark:border-[#3a3a3a]">
        <h1 className="m-0 mr-1 text-[16px] font-medium">{t('schedule.title')}</h1>
        {orgId === PERSONAL_CENTER ? null : (
          <>
            <button
              type="button"
              className={quietButtonClassName(30)}
              onClick={() => setAnchor(startOfDay(new Date()))}>
              {t('schedule.today')}
            </button>
            <button
              type="button"
              className={quietButtonClassName(30)}
              aria-label={t('schedule.prev')}
              onClick={() => setAnchor((value) => shiftAnchor(mode, value, -1))}>
              <ChevronLeft size={16} strokeWidth={1.7} />
            </button>
            <button
              type="button"
              className={quietButtonClassName(30)}
              aria-label={t('schedule.next')}
              onClick={() => setAnchor((value) => shiftAnchor(mode, value, 1))}>
              <ChevronRight size={16} strokeWidth={1.7} />
            </button>
            <p className="m-0 min-w-0 text-[13px] text-[#171717] dark:text-[#f5f5f5]">
              {rangeLabel(range.start, range.end, language)}
            </p>
            <div className="ml-auto flex flex-wrap items-center gap-2">
              <SegmentedControl<ScheduleMode>
                aria-label={t('schedule.view.label')}
                data-ui="schedule.mode"
                size="sm"
                value={mode}
                options={[
                  { value: 'day', label: t('schedule.view.day') },
                  { value: 'week', label: t('schedule.view.week') },
                  { value: 'month', label: t('schedule.view.month') }
                ]}
                onValueChange={setMode}
              />
              {canCreate ? (
                <button
                  type="button"
                  data-ui="schedule.create"
                  className={cn(filledButtonClassName(36), '[&_svg]:text-white')}
                  onClick={() => {
                    const slot = defaultSlot(anchor, new Date())
                    openDraft(slot.start, slot.end)
                  }}>
                  <Plus aria-hidden className="shrink-0" color="#ffffff" size={16} strokeWidth={2.5} />
                  {t('schedule.create')}
                </button>
              ) : null}
            </div>
          </>
        )}
      </header>
      {orgId === PERSONAL_CENTER || !canRead ? (
        body
      ) : (
        <div className="flex min-h-0 flex-1 flex-col min-[900px]:flex-row">
          <aside className="max-h-[240px] shrink-0 overflow-y-auto border-b border-[#efefef] px-4 py-3 min-[900px]:max-h-none min-[900px]:w-[260px] min-[900px]:border-r min-[900px]:border-b-0 dark:border-[#3a3a3a]">
            <MonthDots
              cursor={mini}
              selected={anchor}
              onCursor={setMini}
              onPick={(day) => setAnchor(startOfDay(day))}
            />
            <p className="mt-4 mb-1 text-[12px] text-[#8a8a8a]">{t('schedule.mine')}</p>
            <p className="m-0 flex h-9 items-center gap-2 rounded-[8px] bg-[#f2f3f5] px-2.5 text-[13px] dark:bg-[#2c2c2c]">
              <span className="size-2.5 rounded-[3px] bg-[#171717] dark:bg-[#f5f5f5]" />
              {t('schedule.mine')}
            </p>
            {loading ? <p className="mt-3 mb-0 text-[12px] text-[#8a8a8a]">{t('schedule.loading')}</p> : null}
          </aside>
          {body}
        </div>
      )}
      <DialogPortalContainerProvider container={portal}>
        <Dialog
          open={canRead && draft !== null && openedAuthority.current === draftAuthority}
          onOpenChange={(open) => !open && setDraft(null)}>
          <DialogContent
            data-ui="schedule.dialog"
            closeLabel={t('common.close')}
            overlayClassName="z-[120]"
            size="xl"
            className="z-[120] overflow-visible bg-white text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)]"
            onEscapeKeyDown={(event) => {
              if (
                document.querySelector(
                  '[data-ui="schedule.form.menu"], [data-ui="schedule.form.until"], [data-ui="schedule.form.calendar"], [data-ui="schedule.form.times"]'
                )
              ) {
                event.preventDefault()
              }
            }}>
            {draft ? (
              <>
                <DialogTitle className="sr-only">
                  {draft.id ? t('schedule.form.editTitle') : t('schedule.form.createTitle')}
                </DialogTitle>
                <ScheduleForm
                  draft={draft}
                  orgId={orgId}
                  events={events}
                  saving={saving}
                  canSave={
                    draft.id
                      ? canUpdate && events.find((event) => event.id === draft.id)?.can_update === true
                      : canCreate
                  }
                  canDelete={canDelete && events.find((event) => event.id === draft.id)?.can_delete === true}
                  onChange={setDraft}
                  onSubmit={() => void saveDraft()}
                  onCancel={() => setDraft(null)}
                  onDelete={() => setConfirmDelete(true)}
                />
              </>
            ) : null}
          </DialogContent>
        </Dialog>
        <ConfirmDialog
          open={canDelete && confirmDelete && events.find((event) => event.id === draft?.id)?.can_delete === true}
          onOpenChange={setConfirmDelete}
          title={t('schedule.form.deleteTitle')}
          description={t('schedule.form.deleteBody')}
          confirmText={t('schedule.form.delete')}
          cancelText={t('schedule.form.cancel')}
          destructive
          confirmLoading={saving}
          overlayClassName="z-[140]"
          contentClassName="z-[140] bg-white text-[#171717]"
          onConfirm={removeDraft}
        />
      </DialogPortalContainerProvider>
    </div>
  )
}

function addDayEnd(day: Date): Date {
  return new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1)
}

function coversDay(event: ScheduleEvent, day: Date): boolean {
  const start = new Date(event.starts_at).getTime()
  const end = new Date(event.ends_at).getTime()
  const dayStart = startOfDay(day).getTime()
  return start < dayStart + 86_400_000 && end > dayStart
}

function visibleScheduleError(caught: unknown, fallback: string): string {
  const message = caught instanceof Error ? caught.message.trim() : ''
  if (!message || /^(GET|POST|PUT|DELETE) \//.test(message)) return fallback
  return message
}
