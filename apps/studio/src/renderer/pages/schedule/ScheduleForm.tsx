import { Calendar } from '@a3s-lab/ui'
import { AlignLeft, Bell, Check, ChevronDown, MapPin, Users } from 'lucide-react'
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { readMemberAccount } from '@renderer/data/memberOrgs'
import { cn } from '@renderer/utils/style'

import { PersonAvatar } from '../messages/messageAvatars'
import { PeoplePickerDialog } from '../messages/PeopleDialogs'
import { listOrgDepartments, listOrgMembers, type OrgDepartment, type OrgMemberRow } from '../org/orgApi'
import { filledButtonClassName, orgFieldClassName, quietButtonClassName } from '../org/orgUi'
import type { ScheduleEvent } from './scheduleApi'
import { ScheduleDraftDay } from './ScheduleDraftDay'
import {
  decodeRule,
  defaultCustom,
  emailOk,
  encodeCustom,
  monthDay,
  monthNumber,
  nthOfMonth,
  REMINDER_PRESETS,
  REPEAT_PRESETS,
  shiftDate,
  TIME_ZONES,
  weekdayIndex,
  type CustomRepeat,
  type RepeatPreset
} from './scheduleRule'
import { scheduleCalendarClassNames, ScheduleWhenFields } from './ScheduleWhenFields'

export type DraftGuest = {
  member_id?: string
  email?: string
  label: string
  avatar?: string
  phone?: string
}

export type ScheduleDraftState = {
  id: string | null
  title: string
  location: string
  note: string
  minutes: string
  startDate: string
  startTime: string
  endDate: string
  endTime: string
  allDay: boolean
  timeZone: string
  repeatRule: string
  guestPermission: 'invite' | 'invite_notes'
  reminders: number[]
  guests: DraftGuest[]
  visibility: 'default' | 'public' | 'private'
  availability: 'busy' | 'free'
  error: string
}

const ZH_WEEK = ['日', '一', '二', '三', '四', '五', '六']
const rowClass =
  'flex h-9 w-full items-center gap-2 rounded-[8px] px-1 text-left text-[13px] text-[#171717] hover:bg-[#f7f7f7] dark:text-[#f5f5f5] dark:hover:bg-[#2c2c2c]'
const menuClass =
  'pointer-events-auto fixed z-[200] overflow-y-auto overscroll-contain rounded-[12px] border border-[#efefef] bg-white p-1 shadow-[0_12px_32px_rgba(0,0,0,0.12)] outline-none dark:border-[#3a3a3a] dark:bg-[#1c1c1c]'

type OverlayPlace = {
  top?: number
  bottom?: number
  left: number
  width: number
  maxHeight: number
}

/** Overlay the trigger. A list that fits does not scroll, and it never grows the dialog. */
function placeOverlay(trigger: HTMLElement, menu: HTMLElement | null, minWidth: number): OverlayPlace {
  const rect = trigger.getBoundingClientRect()
  const gap = 4
  const margin = 8
  const content = menu?.scrollHeight ?? 0
  const below = window.innerHeight - rect.bottom - gap - margin
  const above = rect.top - gap - margin
  const lift = content > below && above > below
  const width = Math.min(Math.max(rect.width, minWidth), window.innerWidth - margin * 2)
  let left = rect.left
  if (left + width > window.innerWidth - margin) left = window.innerWidth - margin - width
  return {
    top: lift ? undefined : rect.bottom + gap,
    bottom: lift ? window.innerHeight - rect.top + gap : undefined,
    left: Math.max(margin, left),
    width,
    maxHeight: Math.max(36, Math.floor(lift ? above : below))
  }
}

function memberGuest(person: OrgMemberRow): DraftGuest {
  return {
    member_id: person.member_id ?? undefined,
    label: person.name || person.email,
    avatar: person.avatar,
    phone: person.phone ?? ''
  }
}

function shortWeekday(date: string, language: string): string {
  const index = weekdayIndex(date)
  if (language.toLowerCase().startsWith('zh')) return ZH_WEEK[index] ?? ''
  const [year, month, day] = date.split('-').map(Number)
  return new Intl.DateTimeFormat(language, { weekday: 'short' }).format(new Date(year, month - 1, day))
}

export function ScheduleForm({
  draft,
  orgId,
  events,
  saving,
  canSave,
  canDelete,
  onChange,
  onSubmit,
  onCancel,
  onDelete
}: {
  draft: ScheduleDraftState
  orgId: string
  events: readonly ScheduleEvent[]
  saving: boolean
  canSave: boolean
  canDelete: boolean
  onChange: (next: ScheduleDraftState) => void
  onSubmit: () => void
  onCancel: () => void
  onDelete: () => void
}) {
  const { t, i18n } = useTranslation()
  const language = i18n.resolvedLanguage || i18n.language
  const zh = language.toLowerCase().startsWith('zh')
  const [panel, setPanel] = useState<string | null>(null)
  const [members, setMembers] = useState<OrgMemberRow[] | null>(null)
  const [departments, setDepartments] = useState<OrgDepartment[]>([])
  const [calendarName, setCalendarName] = useState('')
  const [untilOpen, setUntilOpen] = useState(false)
  const rootRef = useRef<HTMLDivElement>(null)
  const directoryEpoch = useRef(0)
  const draftRef = useRef(draft)
  draftRef.current = draft

  useEffect(() => {
    if (typeof readMemberAccount !== 'function') return
    let live = true
    readMemberAccount()
      .then((account) => {
        if (live && account.displayName) setCalendarName(account.displayName)
      })
      .catch(() => undefined)
    return () => {
      live = false
    }
  }, [])

  useEffect(() => {
    if (!panel) return
    function onPointerDown(event: PointerEvent) {
      const target = event.target
      if (!(target instanceof Node) || rootRef.current?.contains(target)) return
      if (
        target instanceof Element &&
        target.closest(
          '[data-ui="schedule.form.menu"], [data-ui="schedule.form.until"], [data-ui="messages.people-dialog"]'
        )
      )
        return
      setPanel(null)
      setUntilOpen(false)
    }
    document.addEventListener('pointerdown', onPointerDown)
    return () => document.removeEventListener('pointerdown', onPointerDown)
  }, [panel])

  function toggle(next: string) {
    setUntilOpen(false)
    setPanel((current) => (current === next ? null : next))
  }

  async function loadDirectory() {
    const epoch = directoryEpoch.current + 1
    directoryEpoch.current = epoch
    try {
      const [people, groups] = await Promise.all([listOrgMembers(orgId), listOrgDepartments(orgId)])
      if (directoryEpoch.current !== epoch) return
      setMembers(people.filter((person) => person.status === 'active' && Boolean(person.member_id)))
      setDepartments(groups)
    } catch {
      if (directoryEpoch.current !== epoch) return
      setMembers([])
      setDepartments([])
    }
  }

  useEffect(() => {
    if (!draftRef.current.guests.some((guest) => guest.member_id)) return
    void loadDirectory()
  }, [orgId])

  useEffect(() => {
    if (!members) return
    const current = draftRef.current
    const byId = new Map(members.map((person) => [person.member_id, person]))
    let changed = false
    const guests = current.guests.map((guest) => {
      if (!guest.member_id) return guest
      const person = byId.get(guest.member_id)
      if (!person) return guest
      const next = memberGuest(person)
      if (guest.label === next.label && guest.avatar === next.avatar && guest.phone === next.phone) return guest
      changed = true
      return { ...guest, ...next }
    })
    if (changed) onChange({ ...current, guests })
  }, [members, onChange])

  function openGuests() {
    setUntilOpen(false)
    setPanel('guests')
    void loadDirectory()
  }

  function applyGuests(userIds: string[], emails: string[] = []) {
    const byId = new Map((members ?? []).map((person) => [person.id, person]))
    const guests: DraftGuest[] = []
    for (const id of userIds) {
      const person = byId.get(id)
      if (person) guests.push(memberGuest(person))
      if (guests.length >= 32) break
    }
    for (const email of emails) {
      if (guests.length >= 32) break
      guests.push({ email, label: email })
    }
    onChange({ ...draft, guests, error: '' })
  }

  const decoded = decodeRule(draft.repeatRule)
  const custom = decoded?.preset === 'custom' ? decoded.custom : null
  const zones = (TIME_ZONES as readonly string[]).includes(draft.timeZone)
    ? TIME_ZONES
    : [draft.timeZone, ...TIME_ZONES]
  const pickerPeople = (members ?? []).map((person) => ({
    userId: person.id,
    displayName: person.name || person.email || person.phone || person.id,
    email: person.email,
    departmentId: person.department_id,
    position: person.position_name,
    phone: person.phone ?? '',
    avatar: person.avatar
  }))
  const pickerDepartments = departments.map((row) => ({
    id: row.id,
    parentId: row.parent_id,
    name: row.name,
    sortOrder: row.sort_order
  }))

  function presetText(preset: RepeatPreset): string {
    const weekday = shortWeekday(draft.startDate, language)
    const nth = t(`schedule.form.nth.${nthOfMonth(monthDay(draft.startDate))}`)
    if (preset === 'none') return t('schedule.form.repeat.none')
    if (preset === 'daily') return t('schedule.form.repeat.daily')
    if (preset === 'weekly') return t('schedule.form.repeat.weekly', { weekday })
    if (preset === 'monthly_nth') return t('schedule.form.repeat.monthlyNth', { nth, weekday })
    if (preset === 'monthly_day') return t('schedule.form.repeat.monthlyDay', { day: monthDay(draft.startDate) })
    if (preset === 'yearly')
      return t('schedule.form.repeat.yearly', { month: monthNumber(draft.startDate), day: monthDay(draft.startDate) })
    return t('schedule.form.repeat.weekdays')
  }

  function repeatText(): string {
    if (decoded?.preset && decoded.preset !== 'custom') return presetText(decoded.preset)
    if (decoded?.preset === 'custom') return t('schedule.form.repeat.custom')
    return draft.repeatRule
  }

  function choosePreset(preset: RepeatPreset) {
    onChange({ ...draft, repeatRule: preset, error: '' })
    setPanel(null)
  }

  function chooseCustom() {
    if (custom) {
      setPanel(null)
      return
    }
    onChange({ ...draft, repeatRule: encodeCustom(defaultCustom(draft.startDate)), error: '' })
    setPanel(null)
  }

  function writeCustom(next: CustomRepeat) {
    onChange({ ...draft, repeatRule: encodeCustom(next), error: '' })
  }

  function reminderText(minutes: number): string {
    const key = `schedule.form.reminder.${minutes}`
    return i18n.exists(key) ? t(key) : t('schedule.form.reminder.5')
  }

  function setAllDay(allDay: boolean) {
    if (allDay) {
      const endDate =
        draft.endTime === '00:00' && draft.endDate > draft.startDate ? shiftDate(draft.endDate, -1) : draft.endDate
      onChange({ ...draft, allDay: true, startTime: '00:00', endTime: '00:00', endDate, error: '' })
      return
    }
    onChange({
      ...draft,
      allDay: false,
      startTime: '09:00',
      endTime: draft.endDate === draft.startDate ? '09:30' : '09:00',
      error: ''
    })
  }

  const span = {
    startDate: draft.startDate,
    startTime: draft.startTime,
    endDate: draft.endDate,
    endTime: draft.endTime
  }

  return (
    <form
      className="flex max-h-[min(760px,calc(100dvh-80px))] flex-col"
      onSubmit={(event) => {
        event.preventDefault()
        if (canSave) onSubmit()
      }}>
      <div
        ref={rootRef}
        data-ui="schedule.form.body"
        className="grid min-h-0 flex-1 items-stretch gap-4 overflow-y-auto min-[720px]:grid-cols-[minmax(0,1fr)_232px]">
        <div className="flex min-w-0 flex-col gap-1">
          <input
            aria-label={t('schedule.form.title')}
            className="h-10 w-full border-0 bg-transparent px-1 text-[16px] font-medium shadow-none outline-none placeholder:font-normal placeholder:text-[#8a8a8a]"
            value={draft.title}
            maxLength={80}
            placeholder={t('schedule.form.titlePlaceholder')}
            onChange={(event) => onChange({ ...draft, title: event.target.value, error: '' })}
          />

          <div className="relative">
            <button type="button" data-ui="schedule.form.guests" className={rowClass} onClick={() => openGuests()}>
              {draft.guests.length === 0 ? (
                <Users size={16} strokeWidth={1.7} className="shrink-0 text-[#8a8a8a]" />
              ) : (
                <span aria-hidden className="flex shrink-0 items-center gap-0.5">
                  {draft.guests.slice(0, 3).map((guest) => (
                    <PersonAvatar
                      key={guest.member_id || guest.email || guest.label}
                      name={guest.label}
                      phone={guest.phone ?? ''}
                      avatar={guest.avatar}
                      size={20}
                    />
                  ))}
                </span>
              )}
              <span className={cn('min-w-0 flex-1 truncate', draft.guests.length === 0 && 'text-[#8a8a8a]')}>
                {draft.guests.length === 0
                  ? t('schedule.form.guest.add')
                  : draft.guests.map((guest) => guest.label).join('、')}
              </span>
            </button>
            <PeoplePickerDialog
              open={panel === 'guests'}
              onOpenChange={(open) => {
                if (!open) setPanel((current) => (current === 'guests' ? null : current))
              }}
              title={t('schedule.form.guest.add')}
              confirmLabel={t('common.confirm')}
              people={pickerPeople}
              departments={pickerDepartments}
              withName={false}
              allowEmail
              parseEmail={(value) => (emailOk(value) ? value.trim().toLowerCase() : '')}
              emailHint={(email) => t('schedule.form.guest.email', { email })}
              maxPicked={32}
              initialPicked={draft.guests.flatMap((guest) => {
                const person = members?.find((member) => member.member_id === guest.member_id)
                return person ? [person.id] : []
              })}
              initialEmails={draft.guests.flatMap((guest) => (guest.email && !guest.member_id ? [guest.email] : []))}
              overlayClassName="z-[200]"
              contentClassName="z-[200]"
              onConfirm={async (userIds, _name, pickedEmails) => {
                applyGuests(userIds, pickedEmails ?? [])
              }}
            />
          </div>

          <FieldMenu
            open={panel === 'permission'}
            label={`${t('schedule.form.guest.permission')}：${t(`schedule.form.guestPermission.${draft.guestPermission}`)}`}
            onToggle={() => toggle('permission')}>
            {(['invite', 'invite_notes'] as const).map((permission) => (
              <MenuOption
                key={permission}
                selected={draft.guestPermission === permission}
                onPick={() => {
                  onChange({ ...draft, guestPermission: permission })
                  setPanel(null)
                }}>
                {t(`schedule.form.guestPermission.${permission}`)}
              </MenuOption>
            ))}
          </FieldMenu>

          <div className="py-1">
            <ScheduleWhenFields
              allDay={draft.allDay}
              value={span}
              onChange={(next) => onChange({ ...draft, ...next, error: '' })}
            />
          </div>

          <div className="flex items-center gap-3 px-1">
            <label className="flex h-9 items-center gap-2 text-[13px]">
              <input
                type="checkbox"
                className="size-3.5 accent-[#171717]"
                checked={draft.allDay}
                onChange={(event) => setAllDay(event.target.checked)}
              />
              {t('schedule.form.allDay')}
            </label>
            <span className="text-[#e5e5e5]" aria-hidden>
              |
            </span>
            <FieldMenu
              open={panel === 'zone'}
              label={t('schedule.form.timezone')}
              onToggle={() => toggle('zone')}
              dataUi="schedule.form.timezone">
              {zones.map((zone) => (
                <MenuOption
                  key={zone}
                  selected={draft.timeZone === zone}
                  onPick={() => {
                    onChange({ ...draft, timeZone: zone, error: '' })
                    setPanel(null)
                  }}>
                  {zoneLabel(zone, language)}
                </MenuOption>
              ))}
            </FieldMenu>
          </div>

          <FieldMenu
            open={panel === 'repeat'}
            label={repeatText()}
            onToggle={() => toggle('repeat')}
            dataUi="schedule.form.repeat">
            {REPEAT_PRESETS.map((preset) => (
              <MenuOption key={preset} selected={decoded?.preset === preset} onPick={() => choosePreset(preset)}>
                {presetText(preset)}
              </MenuOption>
            ))}
            <MenuOption selected={decoded?.preset === 'custom'} onPick={chooseCustom}>
              {t('schedule.form.repeat.custom')}
            </MenuOption>
          </FieldMenu>
          {custom ? (
            <CustomRepeatEditor
              custom={custom}
              language={language}
              zh={zh}
              untilOpen={untilOpen}
              onUntilOpen={setUntilOpen}
              onChange={writeCustom}
            />
          ) : null}
          {draft.repeatRule !== 'none' ? (
            <p className="m-0 px-1 text-[12px] text-[#8a8a8a]">{t('schedule.form.repeat.series')}</p>
          ) : null}

          <button type="button" className={rowClass} onClick={() => toggle('minutes')}>
            <AlignLeft size={16} strokeWidth={1.7} className="shrink-0 text-[#8a8a8a]" />
            <span className="text-[#3f3f3f] dark:text-[#b0b4ba]">{t('schedule.form.minutes')}</span>
          </button>
          {panel === 'minutes' || draft.minutes ? (
            <textarea
              aria-label={t('schedule.form.minutes')}
              className={cn(orgFieldClassName, 'h-20 resize-none py-2')}
              value={draft.minutes}
              maxLength={4000}
              placeholder={t('schedule.form.minutesPlaceholder')}
              onChange={(event) => onChange({ ...draft, minutes: event.target.value })}
            />
          ) : null}

          <button type="button" className={rowClass} onClick={() => toggle('note')}>
            <AlignLeft size={16} strokeWidth={1.7} className="shrink-0 text-[#8a8a8a]" />
            <span className="text-[#8a8a8a]">{t('schedule.form.notePlaceholder')}</span>
          </button>
          {panel === 'note' || draft.note ? (
            <textarea
              aria-label={t('schedule.form.note')}
              className={cn(orgFieldClassName, 'h-20 resize-none py-2')}
              value={draft.note}
              maxLength={2000}
              placeholder={t('schedule.form.notePlaceholder')}
              onChange={(event) => onChange({ ...draft, note: event.target.value })}
            />
          ) : null}

          <label className="flex h-9 items-center gap-2 px-1">
            <MapPin size={16} strokeWidth={1.7} className="shrink-0 text-[#8a8a8a]" />
            <input
              aria-label={t('schedule.form.location')}
              className="h-9 min-w-0 flex-1 border-0 bg-transparent px-0 text-[13px] shadow-none outline-none placeholder:text-[#8a8a8a]"
              value={draft.location}
              maxLength={120}
              placeholder={t('schedule.form.locationPlaceholder')}
              onChange={(event) => onChange({ ...draft, location: event.target.value })}
            />
          </label>

          {draft.reminders.map((minutes, index) => (
            <div key={`${minutes}-${index}`} className="flex items-center gap-2">
              <FieldMenu
                open={panel === `reminder-${index}`}
                label={
                  <span className="flex items-center gap-2">
                    <Bell size={16} strokeWidth={1.7} className="shrink-0 text-[#8a8a8a]" />
                    {reminderText(minutes)}
                  </span>
                }
                dataUi="schedule.form.reminder"
                onToggle={() => toggle(`reminder-${index}`)}>
                {REMINDER_PRESETS.map((preset) => (
                  <MenuOption
                    key={preset}
                    selected={minutes === preset}
                    onPick={() => {
                      const reminders = draft.reminders.map((value, at) => (at === index ? preset : value))
                      onChange({ ...draft, reminders })
                      setPanel(null)
                    }}>
                    {reminderText(preset)}
                  </MenuOption>
                ))}
              </FieldMenu>
              <span className="shrink-0 text-[13px] text-[#8a8a8a]">{t('schedule.form.reminder.me')}</span>
              <button
                type="button"
                className="shrink-0 px-1 text-[12px] text-[#8a8a8a]"
                onClick={() => onChange({ ...draft, reminders: draft.reminders.filter((_, at) => at !== index) })}>
                ×
              </button>
            </div>
          ))}
          {draft.reminders.length < 5 ? (
            <button
              type="button"
              className={cn(rowClass, 'text-[#3f3f3f]')}
              onClick={() => {
                const preset = REMINDER_PRESETS.find((item) => !draft.reminders.includes(item)) ?? 15
                onChange({ ...draft, reminders: [...draft.reminders, preset] })
              }}>
              {t('schedule.form.reminder.add')}
            </button>
          ) : null}

          <div className="flex h-9 items-center gap-2 px-1 text-[13px]">
            <span className="size-2.5 shrink-0 rounded-[3px] bg-[#171717] dark:bg-[#f5f5f5]" />
            <span className="min-w-0 truncate">{calendarName || t('schedule.mine')}</span>
          </div>

          <FieldMenu
            open={panel === 'visibility'}
            label={t(`schedule.form.visibility.${draft.visibility}`)}
            onToggle={() => toggle('visibility')}>
            {(['default', 'public', 'private'] as const).map((visibility) => (
              <MenuOption
                key={visibility}
                selected={draft.visibility === visibility}
                onPick={() => {
                  onChange({ ...draft, visibility })
                  setPanel(null)
                }}>
                {t(`schedule.form.visibility.${visibility}`)}
              </MenuOption>
            ))}
          </FieldMenu>

          <FieldMenu
            open={panel === 'availability'}
            label={t(`schedule.form.availability.${draft.availability}`)}
            onToggle={() => toggle('availability')}>
            {(['busy', 'free'] as const).map((availability) => (
              <MenuOption
                key={availability}
                selected={draft.availability === availability}
                onPick={() => {
                  onChange({ ...draft, availability })
                  setPanel(null)
                }}>
                {t(`schedule.form.availability.${availability}`)}
              </MenuOption>
            ))}
          </FieldMenu>

          {draft.error ? <p className="m-0 px-1 text-[12px] text-[#171717]">{draft.error}</p> : null}
        </div>
        <ScheduleDraftDay
          value={span}
          title={draft.title}
          events={events}
          editingId={draft.id}
          onChange={(next) => onChange({ ...draft, ...next, error: '' })}
        />
      </div>
      <div className="mt-3 flex flex-wrap items-center justify-end gap-2">
        {draft.id && canDelete ? (
          <button type="button" className={cn(quietButtonClassName(36), 'mr-auto')} onClick={onDelete}>
            {t('schedule.form.delete')}
          </button>
        ) : null}
        <button type="button" className={quietButtonClassName(36)} onClick={onCancel}>
          {t('schedule.form.cancel')}
        </button>
        {canSave ? (
          <button type="submit" className={filledButtonClassName(36)} disabled={saving}>
            {t('schedule.form.save')}
          </button>
        ) : null}
      </div>
    </form>
  )
}

function FieldMenu({
  open,
  label,
  onToggle,
  dataUi,
  children
}: {
  open: boolean
  label: ReactNode
  onToggle: () => void
  dataUi?: string
  children: ReactNode
}) {
  const anchorRef = useRef<HTMLButtonElement>(null)
  return (
    <div className="relative">
      <button
        type="button"
        ref={anchorRef}
        data-ui={dataUi}
        aria-expanded={open}
        className={rowClass}
        onClick={onToggle}>
        <span className="min-w-0 flex-1 truncate">{label}</span>
        <ChevronDown
          size={14}
          strokeWidth={1.7}
          className={cn('shrink-0 text-[#8a8a8a] transition-transform', open && 'rotate-180')}
        />
      </button>
      <OverlayMenu
        open={open}
        anchor={anchorRef}
        minWidth={240}
        dataUi="schedule.form.menu"
        className={menuClass}
        onClose={onToggle}>
        {children}
      </OverlayMenu>
    </div>
  )
}

function OverlayMenu({
  open,
  anchor,
  minWidth,
  dataUi,
  className,
  onClose,
  role = 'listbox',
  children
}: {
  open: boolean
  anchor: RefObject<HTMLElement | null>
  minWidth: number
  dataUi: string
  className: string
  onClose: () => void
  role?: 'listbox'
  children: ReactNode
}) {
  const menuRef = useRef<HTMLDivElement>(null)
  const [place, setPlace] = useState<OverlayPlace | null>(null)

  useLayoutEffect(() => {
    if (!open) return
    const apply = () => {
      const node = anchor.current
      if (!node) return
      setPlace(placeOverlay(node, menuRef.current, minWidth))
    }
    apply()
    menuRef.current?.focus({ preventScroll: true })
    window.addEventListener('resize', apply)
    window.addEventListener('scroll', apply, true)
    return () => {
      window.removeEventListener('resize', apply)
      window.removeEventListener('scroll', apply, true)
    }
  }, [open, anchor, minWidth])

  if (!open || typeof document === 'undefined') return null
  return createPortal(
    <div
      ref={menuRef}
      role={role}
      tabIndex={-1}
      data-ui={dataUi}
      className={className}
      style={
        place
          ? { top: place.top, bottom: place.bottom, left: place.left, width: place.width, maxHeight: place.maxHeight }
          : { top: 0, left: 0, visibility: 'hidden' }
      }
      onKeyDown={(event) => {
        if (event.key !== 'Escape') return
        event.preventDefault()
        event.stopPropagation()
        onClose()
      }}>
      {children}
    </div>,
    document.body
  )
}

function MenuOption({ selected, onPick, children }: { selected: boolean; onPick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      className={cn(
        'flex h-9 w-full items-center gap-2 rounded-[8px] px-2 text-left text-[13px] text-[#171717] hover:bg-[#f7f7f7] dark:text-[#f5f5f5]',
        selected && 'bg-[#f2f3f5] font-medium dark:bg-[#2c2c2c]'
      )}
      onClick={onPick}>
      <Check size={14} strokeWidth={1.7} className={selected ? 'shrink-0' : 'shrink-0 opacity-0'} />
      <span className="min-w-0 truncate">{children}</span>
    </button>
  )
}

function CustomRepeatEditor({
  custom,
  language,
  zh,
  untilOpen,
  onUntilOpen,
  onChange
}: {
  custom: CustomRepeat
  language: string
  zh: boolean
  untilOpen: boolean
  onUntilOpen: (open: boolean) => void
  onChange: (next: CustomRepeat) => void
}) {
  const { t } = useTranslation()
  const units = ['day', 'week', 'month', 'year'] as const
  return (
    <div className="mb-1 flex flex-col gap-2 rounded-[8px] bg-[#f7f7f7] p-2 dark:bg-[#1c1c1c]">
      <div className="flex flex-wrap items-center gap-2 text-[13px]">
        <span>{t('schedule.form.repeat.every')}</span>
        <input
          aria-label={t('schedule.form.repeat.every')}
          type="number"
          min={1}
          max={30}
          className="h-8 w-14 rounded-[8px] border-0 bg-white px-2 text-[13px] dark:bg-[#111112]"
          value={custom.interval}
          onChange={(event) => onChange({ ...custom, interval: Number(event.target.value) })}
        />
        {units.map((unit) => (
          <button
            key={unit}
            type="button"
            className={cn(
              quietButtonClassName(30),
              custom.unit === unit && 'bg-[#171717] text-white hover:bg-[#2a2a2a]'
            )}
            onClick={() => onChange({ ...custom, unit })}>
            {t(`schedule.form.repeat.unit.${unit}`)}
          </button>
        ))}
      </div>
      {custom.unit === 'week' ? (
        <div className="flex gap-1">
          {ZH_WEEK.map((label, index) => {
            const selected = custom.weekdays.includes(index)
            return (
              <button
                key={label}
                type="button"
                aria-pressed={selected}
                className={cn(
                  'flex size-7 items-center justify-center rounded-full text-[12px]',
                  selected ? 'bg-[#171717] text-white' : 'bg-white text-[#171717] dark:bg-[#111112] dark:text-[#f5f5f5]'
                )}
                onClick={() => {
                  const weekdays = selected
                    ? custom.weekdays.filter((day) => day !== index)
                    : [...custom.weekdays, index]
                  onChange({ ...custom, weekdays: weekdays.length > 0 ? weekdays : [index] })
                }}>
                {zh ? label : shortWeekday(`2026-10-${4 + index}`, language)}
              </button>
            )
          })}
        </div>
      ) : null}
      {custom.unit === 'month' ? (
        <div className="flex gap-2">
          {(['day', 'nth'] as const).map((mode) => (
            <button
              key={mode}
              type="button"
              className={cn(
                quietButtonClassName(30),
                custom.monthMode === mode && 'bg-[#171717] text-white hover:bg-[#2a2a2a]'
              )}
              onClick={() => onChange({ ...custom, monthMode: mode })}>
              {t(mode === 'day' ? 'schedule.form.repeat.monthDay' : 'schedule.form.repeat.monthNth')}
            </button>
          ))}
        </div>
      ) : null}
      <div className="flex flex-wrap gap-2">
        {(['never', 'until', 'count'] as const).map((end) => (
          <button
            key={end}
            type="button"
            className={cn(quietButtonClassName(30), custom.end === end && 'bg-[#171717] text-white hover:bg-[#2a2a2a]')}
            onClick={() => onChange({ ...custom, end })}>
            {t(
              end === 'never'
                ? 'schedule.form.repeat.endNever'
                : end === 'until'
                  ? 'schedule.form.repeat.endUntil'
                  : 'schedule.form.repeat.endCount'
            )}
          </button>
        ))}
      </div>
      {custom.end === 'until' ? (
        <UntilField
          custom={custom}
          language={language}
          zh={zh}
          open={untilOpen}
          onOpen={onUntilOpen}
          onChange={onChange}
        />
      ) : null}
      {custom.end === 'count' ? (
        <input
          aria-label={t('schedule.form.repeat.endCount')}
          type="number"
          min={1}
          max={366}
          className="h-8 w-20 rounded-[8px] border-0 bg-white px-2 text-[13px] dark:bg-[#111112]"
          value={custom.count}
          onChange={(event) => onChange({ ...custom, count: Number(event.target.value) })}
        />
      ) : null}
    </div>
  )
}

function UntilField({
  custom,
  language,
  zh,
  open,
  onOpen,
  onChange
}: {
  custom: CustomRepeat
  language: string
  zh: boolean
  open: boolean
  onOpen: (open: boolean) => void
  onChange: (next: CustomRepeat) => void
}) {
  const anchorRef = useRef<HTMLButtonElement>(null)
  return (
    <div className="relative">
      <button type="button" ref={anchorRef} className={quietButtonClassName(30)} onClick={() => onOpen(!open)}>
        {custom.until}
      </button>
      <OverlayMenu
        open={open}
        anchor={anchorRef}
        minWidth={280}
        dataUi="schedule.form.until"
        role={undefined}
        className="pointer-events-auto fixed z-[200] overflow-y-auto overscroll-contain rounded-[12px] border border-[#efefef] bg-white p-2 shadow-[0_12px_32px_rgba(0,0,0,0.12)] outline-none"
        onClose={() => onOpen(false)}>
        <Calendar
          mode="single"
          weekStartsOn={0}
          captionLayout="label"
          navLayout="around"
          lang={zh ? 'zh-CN' : language}
          showOutsideDays
          selected={custom.until ? new Date(`${custom.until}T00:00:00`) : undefined}
          onSelect={(day) => {
            if (!day) return
            const month = String(day.getMonth() + 1).padStart(2, '0')
            const date = String(day.getDate()).padStart(2, '0')
            onChange({ ...custom, until: `${day.getFullYear()}-${month}-${date}` })
            onOpen(false)
          }}
          className="w-full bg-white p-0"
          classNames={scheduleCalendarClassNames}
        />
      </OverlayMenu>
    </div>
  )
}

function zoneLabel(zone: string, language: string): string {
  try {
    const name = new Intl.DateTimeFormat(language, { timeZone: zone, timeZoneName: 'long' })
      .formatToParts(new Date())
      .find((part) => part.type === 'timeZoneName')?.value
    return name && name !== zone ? `${name}` : zone
  } catch {
    return zone
  }
}
