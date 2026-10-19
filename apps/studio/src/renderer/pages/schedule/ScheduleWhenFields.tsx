import { Calendar } from '@a3s-lab/ui'
import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { cn } from '@renderer/utils/style'

import { composeLocal, dateInputValue, moveSpan, timeChoices, type LocalSpan } from './scheduleTime'

type Edge = 'start' | 'end'
type Picker = 'start-date' | 'end-date' | 'start-time' | 'end-time' | null

const fieldButtonClassName =
  'flex h-9 min-w-0 items-center rounded-[8px] bg-[#f5f5f5] px-2.5 text-left text-[13px] text-[#171717] outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10'

function parseDay(value: string): Date | undefined {
  return composeLocal(value, '00:00') ?? undefined
}

function dayLabel(value: string, language: string): string {
  const day = parseDay(value)
  if (!day) return value
  if (language.toLowerCase().startsWith('zh')) {
    return `${day.getMonth() + 1}月${day.getDate()}日`
  }
  return new Intl.DateTimeFormat(language, { month: 'short', day: 'numeric' }).format(day)
}

function TimeList({ value, onPick }: { value: string; onPick: (time: string) => void }) {
  const scroller = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const parent = scroller.current
    const node = parent?.querySelector('[data-selected="true"]')
    if (!parent || !(node instanceof HTMLElement)) return
    const top = node.getBoundingClientRect().top - parent.getBoundingClientRect().top + parent.scrollTop
    parent.scrollTop = Math.max(0, top - parent.clientHeight / 2 + node.offsetHeight / 2)
  }, [value])
  return (
    <div
      ref={scroller}
      data-ui="schedule.form.times"
      className="max-h-[216px] overflow-y-auto overscroll-contain rounded-[8px] bg-[#f5f5f5] p-1">
      {timeChoices(value).map((time) => {
        const selected = time === value
        return (
          <button
            key={time}
            type="button"
            data-selected={selected ? 'true' : undefined}
            className={cn(
              'flex h-9 w-full items-center rounded-[8px] px-2.5 text-left text-[13px] text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]',
              selected && 'bg-white font-medium text-[#171717]'
            )}
            onClick={() => onPick(time)}>
            {time}
          </button>
        )
      })}
    </div>
  )
}

const panelClassName =
  'pointer-events-auto fixed z-[200] rounded-[12px] border border-[#efefef] bg-white shadow-[0_12px_32px_rgba(0,0,0,0.12)]'

export function ScheduleWhenFields({
  value,
  onChange,
  allDay = false
}: {
  value: LocalSpan
  onChange: (next: LocalSpan) => void
  allDay?: boolean
}) {
  const { t, i18n } = useTranslation()
  const language = i18n.resolvedLanguage || i18n.language
  const zh = language.toLowerCase().startsWith('zh')
  const rootRef = useRef<HTMLDivElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  const [picker, setPicker] = useState<Picker>(null)
  const [month, setMonth] = useState<Date | undefined>(undefined)
  const [lift, setLift] = useState(false)
  const [box, setBox] = useState<{ top: number; bottom: number; left: number; right: number } | null>(null)

  useEffect(() => {
    if (!picker) return
    function onPointerDown(event: PointerEvent) {
      const target = event.target
      if (!(target instanceof Node) || rootRef.current?.contains(target) || panelRef.current?.contains(target)) return
      setPicker(null)
    }
    document.addEventListener('pointerdown', onPointerDown)
    return () => document.removeEventListener('pointerdown', onPointerDown)
  }, [picker])

  function toggle(next: Picker, anchor: HTMLElement) {
    const rect = anchor.getBoundingClientRect()
    const below = window.innerHeight - rect.bottom
    setLift(below < 340 && rect.top > below)
    setBox({ top: rect.top, bottom: rect.bottom, left: rect.left, right: rect.right })
    setPicker((current) => {
      const opening = current === next ? null : next
      if (opening === 'start-date') setMonth(parseDay(value.startDate))
      if (opening === 'end-date') setMonth(parseDay(value.endDate))
      return opening
    })
  }

  function apply(edge: Edge, patch: Partial<LocalSpan>) {
    onChange(moveSpan(value, patch, edge))
  }

  const dateEdge: Edge | null = picker === 'start-date' ? 'start' : picker === 'end-date' ? 'end' : null
  const timeEdge: Edge | null = picker === 'start-time' ? 'start' : picker === 'end-time' ? 'end' : null
  const selectedDay = dateEdge ? parseDay(dateEdge === 'start' ? value.startDate : value.endDate) : undefined

  function edgeFields(edge: Edge) {
    const date = edge === 'start' ? value.startDate : value.endDate
    const time = edge === 'start' ? value.startTime : value.endTime
    const datePicker = edge === 'start' ? 'start-date' : 'end-date'
    const timePicker = edge === 'start' ? 'start-time' : 'end-time'
    const dateButton = (
      <button
        type="button"
        data-ui={`schedule.form.${datePicker}`}
        aria-expanded={picker === datePicker}
        aria-label={t(edge === 'start' ? 'schedule.form.start' : 'schedule.form.end')}
        className={cn(fieldButtonClassName, 'flex-1', picker === datePicker && 'bg-[#ececec]')}
        onClick={(event) => toggle(datePicker, event.currentTarget)}>
        {dayLabel(date, language)}
      </button>
    )
    const timeButton = (
      <button
        type="button"
        data-ui={`schedule.form.${timePicker}`}
        aria-expanded={picker === timePicker}
        className={cn(fieldButtonClassName, 'w-[76px] shrink-0', picker === timePicker && 'bg-[#ececec]')}
        onClick={(event) => toggle(timePicker, event.currentTarget)}>
        {time}
      </button>
    )
    return (
      <div key={edge} className="relative flex min-w-[148px] flex-1 gap-2 min-[720px]:min-w-0">
        {edge === 'end' && !allDay ? timeButton : null}
        {dateButton}
        {edge === 'start' && !allDay ? timeButton : null}
      </div>
    )
  }

  const alignEnd = picker === 'start-time' || picker === 'end-date'
  const panel =
    picker && box && typeof document !== 'undefined'
      ? createPortal(
          <div
            ref={panelRef}
            data-ui={dateEdge ? 'schedule.form.calendar' : undefined}
            className={cn(panelClassName, dateEdge ? 'w-[280px] p-2' : 'w-[132px] p-1')}
            style={{
              top: lift ? undefined : box.bottom + 4,
              bottom: lift ? window.innerHeight - box.top + 4 : undefined,
              left: alignEnd ? undefined : box.left,
              right: alignEnd ? window.innerWidth - box.right : undefined
            }}>
            {dateEdge ? (
              <Calendar
                mode="single"
                weekStartsOn={0}
                captionLayout="label"
                navLayout="around"
                lang={zh ? 'zh-CN' : language}
                showOutsideDays
                month={month ?? selectedDay}
                onMonthChange={setMonth}
                selected={selectedDay}
                onSelect={(day) => {
                  if (!day) return
                  apply(
                    dateEdge,
                    dateEdge === 'start' ? { startDate: dateInputValue(day) } : { endDate: dateInputValue(day) }
                  )
                  setPicker(null)
                }}
                formatters={{
                  formatCaption: (monthDate) =>
                    zh
                      ? `${monthDate.getFullYear()}年${monthDate.getMonth() + 1}月`
                      : new Intl.DateTimeFormat(language, { year: 'numeric', month: 'long' }).format(monthDate),
                  formatWeekdayName: (weekday) =>
                    weekday.toLocaleDateString(zh ? 'zh-CN' : language, { weekday: 'narrow' })
                }}
                labels={{
                  labelPrevious: () => t('schedule.prev'),
                  labelNext: () => t('schedule.next')
                }}
                className="w-full bg-white p-0"
                classNames={scheduleCalendarClassNames}
              />
            ) : timeEdge ? (
              <TimeList
                value={timeEdge === 'start' ? value.startTime : value.endTime}
                onPick={(picked) => {
                  apply(timeEdge, timeEdge === 'start' ? { startTime: picked } : { endTime: picked })
                  setPicker(null)
                }}
              />
            ) : null}
          </div>,
          document.body
        )
      : null

  return (
    <div
      ref={rootRef}
      className="flex flex-col gap-2 min-[720px]:flex-row min-[720px]:flex-nowrap min-[720px]:items-center">
      {edgeFields('start')}
      <span className="hidden shrink-0 text-[13px] text-[#8a8a8a] min-[720px]:inline" aria-hidden>
        —
      </span>
      {edgeFields('end')}
      {panel}
    </div>
  )
}

export const scheduleCalendarClassNames = {
  root: 'rdp-root w-full',
  months: 'rdp-months flex w-full max-w-none flex-col',
  month: 'rdp-month grid w-full grid-cols-[2rem_minmax(0,1fr)_2rem] items-center gap-y-1',
  month_caption:
    'rdp-month_caption col-start-2 row-start-1 flex h-8 items-center justify-center text-[13px] font-medium text-[#171717]',
  caption_label: 'rdp-caption_label text-[13px] font-medium text-[#171717]',
  button_previous:
    'rdp-button_previous col-start-1 row-start-1 inline-flex size-8 items-center justify-center justify-self-start rounded-[8px] text-[#171717] hover:bg-[#f5f5f5]',
  button_next:
    'rdp-button_next col-start-3 row-start-1 inline-flex size-8 items-center justify-center justify-self-end rounded-[8px] text-[#171717] hover:bg-[#f5f5f5]',
  today: 'font-medium',
  month_grid: 'col-span-3 row-start-2 w-full table-fixed border-collapse',
  weekdays: 'w-full',
  weekday: 'w-[14.28%] pb-1 text-center text-[12px] font-normal text-[#8a8a8a]',
  week: 'w-full',
  day: 'group/day relative w-[14.28%] min-w-0 p-0.5 text-center text-[13px] text-[#171717]',
  day_button:
    'flex aspect-square w-full items-center justify-center rounded-[8px] text-[13px] font-normal text-inherit hover:bg-[#f7f7f7] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10',
  selected: 'bg-[#171717] text-white',
  outside: 'text-[#8a8a8a]'
}
