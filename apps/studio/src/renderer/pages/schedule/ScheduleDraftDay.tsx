import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'

import { cn } from '@renderer/utils/style'

import { quietButtonClassName } from '../org/orgUi'
import type { ScheduleEvent } from './scheduleApi'
import {
  addDays,
  composeLocal,
  dateInputValue,
  gmtLabel,
  placeOnDay,
  sameDay,
  slotFromOffset,
  startOfDay,
  timeInputValue,
  type LocalSpan
} from './scheduleTime'

const HOUR_PX = 32

export function ScheduleDraftDay({
  value,
  title,
  events,
  editingId,
  onChange
}: {
  value: LocalSpan
  title: string
  events: readonly ScheduleEvent[]
  editingId: string | null
  onChange: (next: LocalSpan) => void
}) {
  const { t, i18n } = useTranslation()
  const language = i18n.resolvedLanguage || i18n.language
  const scroller = useRef<HTMLDivElement>(null)
  const start = composeLocal(value.startDate, value.startTime)
  const end = composeLocal(value.endDate, value.endTime)
  const day = start ? startOfDay(start) : startOfDay(new Date())
  const duration = start && end && end > start ? end.getTime() - start.getTime() : 60 * 60 * 1000
  const others = events.flatMap((event) => {
    if (event.id === editingId) return []
    const eventStart = new Date(event.starts_at)
    const eventEnd = new Date(event.ends_at)
    if (!Number.isFinite(eventStart.getTime()) || !Number.isFinite(eventEnd.getTime())) return []
    return [{ id: event.id, title: event.title, start: eventStart, end: eventEnd }]
  })
  const draft = {
    id: 'draft',
    title: title.trim() || t('schedule.form.titlePlaceholder'),
    start: start ?? day,
    end: new Date((start ?? day).getTime() + duration)
  }
  const placed = placeOnDay([...others, draft], day, HOUR_PX)
  const byId = new Map([...others, draft].map((item) => [item.id, item]))
  const now = new Date()
  const showNow = sameDay(day, now)
  const zh = language.toLowerCase().startsWith('zh')
  const heading = zh
    ? `${day.getMonth() + 1}月${day.getDate()}日`
    : new Intl.DateTimeFormat(language, { month: 'short', day: 'numeric' }).format(day)

  useEffect(() => {
    const node = scroller.current
    if (!node || !start) return
    const top = ((start.getHours() * 60 + start.getMinutes()) / 60) * HOUR_PX
    node.scrollTop = Math.max(0, top - HOUR_PX)
  }, [value.startDate, value.startTime])

  function shift(delta: number) {
    if (!start || !end) return
    const nextStart = addDays(start, delta)
    const nextEnd = addDays(end, delta)
    onChange({
      startDate: dateInputValue(nextStart),
      startTime: timeInputValue(nextStart),
      endDate: dateInputValue(nextEnd),
      endTime: timeInputValue(nextEnd)
    })
  }

  function place(offset: number) {
    const slot = slotFromOffset(day, offset, HOUR_PX)
    const nextEnd = new Date(slot.start.getTime() + duration)
    onChange({
      startDate: dateInputValue(slot.start),
      startTime: timeInputValue(slot.start),
      endDate: dateInputValue(nextEnd),
      endTime: timeInputValue(nextEnd)
    })
  }

  return (
    <div
      data-ui="schedule.draft.day"
      className="hidden h-[420px] min-w-0 flex-col overflow-hidden rounded-[12px] border border-[#efefef] bg-white min-[720px]:flex">
      <div className="flex shrink-0 items-center gap-1 border-b border-[#efefef] px-2 py-2">
        <button
          type="button"
          className={cn(quietButtonClassName(30), 'px-2')}
          onClick={() =>
            shift(start ? Math.round((startOfDay(now).getTime() - startOfDay(start).getTime()) / 86400000) : 0)
          }>
          {t('schedule.today')}
        </button>
        <button
          type="button"
          className={cn(quietButtonClassName(30), 'size-[30px] px-0')}
          aria-label={t('schedule.prev')}
          data-ui="schedule.draft.prev"
          onClick={() => shift(-1)}>
          <ChevronLeft size={16} strokeWidth={1.7} />
        </button>
        <button
          type="button"
          className={cn(quietButtonClassName(30), 'size-[30px] px-0')}
          aria-label={t('schedule.next')}
          data-ui="schedule.draft.next"
          onClick={() => shift(1)}>
          <ChevronRight size={16} strokeWidth={1.7} />
        </button>
        <p className="m-0 ml-auto truncate text-[13px] text-[#171717]">
          {heading}
          {showNow ? ` ${t('schedule.today')}` : ''}
        </p>
      </div>
      <div ref={scroller} className="min-h-0 flex-1 overflow-y-auto">
        <div className="flex" style={{ height: 24 * HOUR_PX }}>
          <div className="relative w-10 shrink-0">
            <span className="absolute top-1 left-1 text-[10px] text-[#8a8a8a]">{gmtLabel(now)}</span>
            {Array.from({ length: 24 }, (_, hour) => (
              <span
                key={hour}
                className="absolute right-1 -translate-y-1/2 text-[10px] text-[#8a8a8a]"
                style={{ top: hour * HOUR_PX }}>
                {String(hour).padStart(2, '0')}
              </span>
            ))}
          </div>
          <div
            className="relative min-w-0 flex-1 border-l border-[#efefef]"
            onClick={(pointer) => {
              const bounds = pointer.currentTarget.getBoundingClientRect()
              place(pointer.clientY - bounds.top)
            }}>
            {Array.from({ length: 24 }, (_, hour) => (
              <div key={hour} className="border-t border-[#efefef]" style={{ height: HOUR_PX }} />
            ))}
            {placed.map((slice) => {
              const item = byId.get(slice.id)
              if (!item) return null
              const mine = slice.id === 'draft'
              return (
                <div
                  key={slice.id}
                  data-ui={mine ? 'schedule.draft.block' : undefined}
                  className={cn(
                    'pointer-events-none absolute overflow-hidden rounded-[6px] px-1.5 py-0.5',
                    mine ? 'bg-[#171717] text-white' : 'border-l-[3px] border-[#171717] bg-[#f2f3f5] text-[#171717]'
                  )}
                  style={{
                    top: slice.top,
                    height: slice.height,
                    left: `calc(${(slice.lane / slice.lanes) * 100}% + 2px)`,
                    width: `calc(${100 / slice.lanes}% - 4px)`
                  }}>
                  <span className="block truncate text-[12px] font-medium">{item.title}</span>
                  <span className={cn('block truncate text-[11px]', mine ? 'text-white' : 'text-[#3f3f3f]')}>
                    {timeInputValue(item.start)}–{timeInputValue(item.end)}
                  </span>
                </div>
              )
            })}
            {showNow ? (
              <div
                className="pointer-events-none absolute right-0 left-0 z-[1]"
                style={{ top: ((now.getHours() * 60 + now.getMinutes()) / 60) * HOUR_PX }}>
                <div className="h-px bg-[#171717]" />
              </div>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  )
}
