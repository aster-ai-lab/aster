import { Calendar, Field, FieldLabel, Input, Popover, PopoverAnchor, PopoverContent, Textarea } from '@a3s-lab/ui'
import {
  useEffect,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type CSSProperties,
  type Dispatch,
  type MouseEvent,
  type MouseEventHandler,
  type ReactNode,
  type SetStateAction
} from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import {
  copiesOk,
  dateSpanIssue,
  EXPENSE_CATEGORIES,
  expenseAmountOk,
  formatOfficeDay,
  inclusiveDays,
  LEAVE_TYPES,
  meetingSpanOk,
  parseOfficeDay,
  QUARTER_HOURS,
  SEAL_KINDS,
  type OfficeDrafts,
  type OfficeFormId
} from './officeAppletDraft'

const fieldLabelClass = 'text-[12px] font-normal leading-4 text-[#8a8a8a]'

type DraftChange = Dispatch<SetStateAction<OfficeDrafts>>

export function OfficeAppletFields({
  appletId,
  drafts,
  onChange,
  travelLinks = [],
  members = []
}: {
  appletId: OfficeFormId
  drafts: OfficeDrafts
  onChange: DraftChange
  travelLinks?: readonly { id: string; title: string }[]
  members?: readonly { id: string; name: string }[]
}) {
  if (appletId === 'approval') return <ApprovalFields draft={drafts.approval} onChange={onChange} />
  if (appletId === 'leave') return <LeaveFields draft={drafts.leave} onChange={onChange} />
  if (appletId === 'travel') return <TravelFields draft={drafts.travel} onChange={onChange} />
  if (appletId === 'expense')
    return <ExpenseFields draft={drafts.expense} onChange={onChange} travelLinks={travelLinks} />
  if (appletId === 'meeting') return <MeetingFields draft={drafts.meeting} onChange={onChange} />
  if (appletId === 'seal') return <SealFields draft={drafts.seal} onChange={onChange} />
  if (appletId === 'clock') return <ClockFields draft={drafts.correction} onChange={onChange} />
  if (appletId === 'correction') return <CorrectionFields draft={drafts.correction} onChange={onChange} />
  if (appletId === 'probation') return <ProbationFields draft={drafts.probation} onChange={onChange} />
  if (appletId === 'ai-resource')
    return <AiResourceFields draft={drafts.aiResource} onChange={onChange} members={members} />
  return <NoticeFields draft={drafts.notice} onChange={onChange} />
}

export function NoticeIcon() {
  return (
    <svg viewBox="0 0 56 56" className="size-full" aria-hidden>
      <circle cx="28" cy="28" r="28" fill="#E7F6F8" />
      <path d="M18 24.5v7l-3 2.2V22.3l3 2.2Z" fill="#2A9BC4" />
      <path d="M20 22.2h8.5l8 4.2v3.2l-8 4.2H20a2 2 0 0 1-2-2v-7.6a2 2 0 0 1 2-2Z" fill="#2A9BC4" />
      <path
        d="M36.5 24.2c1.4 1.1 2.2 2.6 2.2 3.8s-.8 2.7-2.2 3.8"
        fill="none"
        stroke="#2A9BC4"
        strokeWidth="1.7"
        strokeLinecap="round"
      />
    </svg>
  )
}

function ApprovalFields({ draft, onChange }: { draft: OfficeDrafts['approval']; onChange: DraftChange }) {
  const { t } = useTranslation()
  return (
    <>
      <Field className="gap-1">
        <FieldLabel htmlFor="approval-title" className={fieldLabelClass}>
          {t('workbench.applet.field.title')}
        </FieldLabel>
        <Input
          id="approval-title"
          name="title"
          required
          maxLength={80}
          placeholder={t('workbench.applet.field.title.placeholder')}
          value={draft.title}
          onChange={(event) =>
            onChange((current) => ({ ...current, approval: { ...current.approval, title: event.target.value } }))
          }
        />
      </Field>
      <Field className="gap-1">
        <FieldLabel htmlFor="approval-detail" className={fieldLabelClass}>
          {t('workbench.applet.field.detail')}
        </FieldLabel>
        <Textarea.Input
          id="approval-detail"
          name="detail"
          required
          maxLength={2000}
          placeholder={t('workbench.applet.field.detail.placeholder')}
          value={draft.detail}
          onValueChange={(detail) => onChange((current) => ({ ...current, approval: { ...current.approval, detail } }))}
          className="h-40 resize-none"
        />
      </Field>
    </>
  )
}

function LeaveFields({ draft, onChange }: { draft: OfficeDrafts['leave']; onChange: DraftChange }) {
  const { t } = useTranslation()
  return (
    <>
      <Field className="gap-1">
        <FieldLabel htmlFor="leave-type" className={fieldLabelClass}>
          {t('workbench.applet.leave.type')}
        </FieldLabel>
        <OfficeMenu
          id="leave-type"
          label={t('workbench.applet.leave.type')}
          placeholder={t('workbench.applet.leave.type.placeholder')}
          value={draft.type}
          options={LEAVE_TYPES.map((item) => ({
            value: item.value,
            label: t(item.labelKey),
            icon: <LeaveTypeMark code={item.code} color={item.color} />
          }))}
          onChange={(type) => onChange((current) => ({ ...current, leave: { ...current.leave, type } }))}
        />
      </Field>
      <RangeFields
        start={draft.start}
        end={draft.end}
        startId="leave-start"
        endId="leave-end"
        startMarker="workbench.applet.leave.start"
        endMarker="workbench.applet.leave.end"
        rangeMarker="workbench.applet.leave.range"
        onChange={(next) => onChange((current) => ({ ...current, leave: { ...current.leave, ...next } }))}
      />
      <ReasonField
        id="leave-reason"
        label={t('workbench.applet.leave.reason')}
        placeholder={t('workbench.applet.leave.reason.placeholder')}
        value={draft.reason}
        onValueChange={(reason) => onChange((current) => ({ ...current, leave: { ...current.leave, reason } }))}
      />
    </>
  )
}

function TravelFields({ draft, onChange }: { draft: OfficeDrafts['travel']; onChange: DraftChange }) {
  const { t } = useTranslation()
  return (
    <>
      <Field className="gap-1">
        <FieldLabel htmlFor="travel-destination" className={fieldLabelClass}>
          {t('workbench.applet.travel.destination')}
        </FieldLabel>
        <Input
          id="travel-destination"
          name="destination"
          required
          maxLength={80}
          placeholder={t('workbench.applet.travel.destination.placeholder')}
          value={draft.destination}
          onChange={(event) =>
            onChange((current) => ({ ...current, travel: { ...current.travel, destination: event.target.value } }))
          }
        />
      </Field>
      <RangeFields
        start={draft.start}
        end={draft.end}
        startId="travel-start"
        endId="travel-end"
        startMarker="workbench.applet.travel.start"
        endMarker="workbench.applet.travel.end"
        rangeMarker="workbench.applet.travel.range"
        onChange={(next) => onChange((current) => ({ ...current, travel: { ...current.travel, ...next } }))}
      />
      <ReasonField
        id="travel-reason"
        label={t('workbench.applet.travel.reason')}
        placeholder={t('workbench.applet.travel.reason.placeholder')}
        value={draft.reason}
        onValueChange={(reason) => onChange((current) => ({ ...current, travel: { ...current.travel, reason } }))}
      />
      <Field className="gap-1">
        <FieldLabel htmlFor="travel-estimate" className={fieldLabelClass}>
          {t('workbench.applet.travel.estimate')}
        </FieldLabel>
        <Input
          id="travel-estimate"
          name="estimate"
          inputMode="decimal"
          maxLength={10}
          placeholder={t('workbench.applet.expense.amount.placeholder')}
          value={draft.estimate}
          onChange={(event) =>
            onChange((current) => ({ ...current, travel: { ...current.travel, estimate: event.target.value } }))
          }
        />
      </Field>
      <button
        type="button"
        aria-pressed={draft.booking}
        data-ui="workbench.applet.travel.booking"
        className="h-9 rounded-[8px] bg-[#f5f5f5] px-3 text-left text-[13px] text-[#171717]"
        onClick={() =>
          onChange((current) => ({ ...current, travel: { ...current.travel, booking: !current.travel.booking } }))
        }>
        {t('workbench.applet.travel.booking')}
      </button>
    </>
  )
}

function ExpenseFields({
  draft,
  onChange,
  travelLinks
}: {
  draft: OfficeDrafts['expense']
  onChange: DraftChange
  travelLinks: readonly { id: string; title: string }[]
}) {
  const { t } = useTranslation()
  return (
    <>
      <Field className="gap-1">
        <FieldLabel htmlFor="expense-amount" className={fieldLabelClass}>
          {t('workbench.applet.expense.amount')}
        </FieldLabel>
        <Input
          id="expense-amount"
          name="amount"
          required
          inputMode="decimal"
          maxLength={10}
          placeholder={t('workbench.applet.expense.amount.placeholder')}
          aria-invalid={draft.amount.trim() !== '' && !expenseAmountOk(draft.amount)}
          aria-describedby="expense-amount-hint"
          value={draft.amount}
          onChange={(event) =>
            onChange((current) => ({ ...current, expense: { ...current.expense, amount: event.target.value } }))
          }
        />
        <FieldHint id="expense-amount-hint">
          {draft.amount.trim() !== '' && !expenseAmountOk(draft.amount)
            ? t('workbench.applet.expense.amount.invalid')
            : null}
        </FieldHint>
      </Field>
      <ChoiceField
        id="expense-category"
        label={t('workbench.applet.expense.category')}
        placeholder={t('workbench.applet.expense.category.placeholder')}
        value={draft.category}
        options={EXPENSE_CATEGORIES}
        onValueChange={(category) => onChange((current) => ({ ...current, expense: { ...current.expense, category } }))}
      />
      <DayField
        id="expense-date"
        label={t('workbench.applet.expense.date')}
        value={draft.date}
        marker="workbench.applet.expense.date"
        calendarMarker="workbench.applet.expense.calendar"
        onChange={(date) => onChange((current) => ({ ...current, expense: { ...current.expense, date } }))}
      />
      <ReasonField
        id="expense-reason"
        label={t('workbench.applet.expense.reason')}
        placeholder={t('workbench.applet.expense.reason.placeholder')}
        value={draft.reason}
        onValueChange={(reason) => onChange((current) => ({ ...current, expense: { ...current.expense, reason } }))}
      />
      <Field className="gap-1">
        <FieldLabel htmlFor="expense-travel" className={fieldLabelClass}>
          {t('workbench.applet.expense.travel')}
        </FieldLabel>
        <OfficeMenu
          id="expense-travel"
          label={t('workbench.applet.expense.travel')}
          placeholder={t('workbench.applet.expense.travel.placeholder')}
          emptyText={t('workbench.applet.expense.travel.empty')}
          allowClear
          value={draft.travelId}
          options={travelLinks.map((item) => ({ value: item.id, label: item.title }))}
          onChange={(travelId) => onChange((current) => ({ ...current, expense: { ...current.expense, travelId } }))}
        />
      </Field>
    </>
  )
}

const PUNCH_TYPES = [
  { value: '上班', labelKey: 'workbench.applet.clock.type.in' },
  { value: '下班', labelKey: 'workbench.applet.clock.type.out' }
] as const

const PROBATION_RESULTS = [
  { value: '同意转正', labelKey: 'workbench.applet.probation.result.pass' },
  { value: '延期', labelKey: 'workbench.applet.probation.result.defer' },
  { value: '不予转正', labelKey: 'workbench.applet.probation.result.refuse' }
] as const

const AI_TYPES = [
  { value: '模型', labelKey: 'workbench.applet.ai.type.model' },
  { value: '算力', labelKey: 'workbench.applet.ai.type.compute' },
  { value: '数据', labelKey: 'workbench.applet.ai.type.data' }
] as const

function ClockFields({ draft, onChange }: { draft: OfficeDrafts['correction']; onChange: DraftChange }) {
  const { t } = useTranslation()
  return (
    <Field className="gap-1">
      <p className={fieldLabelClass}>{t('workbench.applet.clock.type')}</p>
      <div role="radiogroup" aria-label={t('workbench.applet.clock.type')} className="grid grid-cols-2 gap-2">
        {PUNCH_TYPES.map((item) => {
          const selected = draft.type === item.value
          return (
            <button
              key={item.value}
              type="button"
              role="radio"
              aria-checked={selected}
              onClick={() =>
                onChange((current) => ({ ...current, correction: { ...current.correction, type: item.value } }))
              }
              className={`h-9 rounded-[8px] text-[13px] font-medium ${
                selected ? 'bg-[#171717] text-white' : 'bg-[#f5f5f5] text-[#171717] hover:bg-[#ececec]'
              }`}>
              {t(item.labelKey)}
            </button>
          )
        })}
      </div>
    </Field>
  )
}

function CorrectionFields({ draft, onChange }: { draft: OfficeDrafts['correction']; onChange: DraftChange }) {
  const { t } = useTranslation()
  return (
    <>
      <DayField
        id="correction-date"
        label={t('workbench.applet.clock.date')}
        value={draft.date}
        marker="workbench.applet.clock.date"
        calendarMarker="workbench.applet.clock.calendar"
        onChange={(date) => onChange((current) => ({ ...current, correction: { ...current.correction, date } }))}
      />
      <ChoiceField
        id="correction-type"
        label={t('workbench.applet.clock.type')}
        placeholder={t('workbench.applet.clock.type.placeholder')}
        value={draft.type}
        options={PUNCH_TYPES}
        onValueChange={(type) => onChange((current) => ({ ...current, correction: { ...current.correction, type } }))}
      />
      <Field className="gap-1">
        <FieldLabel htmlFor="correction-time" className={fieldLabelClass}>
          {t('workbench.applet.clock.time')}
        </FieldLabel>
        <Input
          id="correction-time"
          name="time"
          inputMode="numeric"
          maxLength={5}
          placeholder="09:00"
          value={draft.time}
          onChange={(event) =>
            onChange((current) => ({ ...current, correction: { ...current.correction, time: event.target.value } }))
          }
        />
      </Field>
      <ReasonField
        id="correction-reason"
        label={t('workbench.applet.clock.reason')}
        placeholder={t('workbench.applet.clock.reason.placeholder')}
        value={draft.reason}
        onValueChange={(reason) =>
          onChange((current) => ({ ...current, correction: { ...current.correction, reason } }))
        }
      />
    </>
  )
}

function ProbationFields({ draft, onChange }: { draft: OfficeDrafts['probation']; onChange: DraftChange }) {
  const { t } = useTranslation()
  return (
    <>
      <ChoiceField
        id="probation-result"
        label={t('workbench.applet.probation.result')}
        placeholder={t('workbench.applet.probation.result.placeholder')}
        value={draft.result}
        options={PROBATION_RESULTS}
        onValueChange={(result) => onChange((current) => ({ ...current, probation: { ...current.probation, result } }))}
      />
      {draft.result === '延期' ? (
        <DayField
          id="probation-defer"
          label={t('workbench.applet.probation.defer')}
          value={draft.deferTo}
          marker="workbench.applet.probation.defer"
          calendarMarker="workbench.applet.probation.calendar"
          onChange={(deferTo) => onChange((current) => ({ ...current, probation: { ...current.probation, deferTo } }))}
        />
      ) : null}
      <ReasonField
        id="probation-summary"
        name="summary"
        label={t('workbench.applet.probation.summary')}
        placeholder={t('workbench.applet.probation.summary.placeholder')}
        value={draft.summary}
        onValueChange={(summary) =>
          onChange((current) => ({ ...current, probation: { ...current.probation, summary } }))
        }
      />
    </>
  )
}

function AiResourceFields({
  draft,
  onChange,
  members
}: {
  draft: OfficeDrafts['aiResource']
  onChange: DraftChange
  members: readonly { id: string; name: string }[]
}) {
  const { t } = useTranslation()
  const set = (patch: Partial<OfficeDrafts['aiResource']>) =>
    onChange((current) => ({ ...current, aiResource: { ...current.aiResource, ...patch } }))
  return (
    <>
      <Field className="gap-1">
        <FieldLabel htmlFor="ai-project" className={fieldLabelClass}>
          {t('workbench.applet.ai.project')}
        </FieldLabel>
        <Input
          id="ai-project"
          name="project"
          maxLength={80}
          value={draft.project}
          onChange={(event) => set({ project: event.target.value })}
        />
      </Field>
      <ReasonField
        id="ai-reason"
        label={t('workbench.applet.ai.reason')}
        placeholder={t('workbench.applet.ai.reason.placeholder')}
        value={draft.reason}
        onValueChange={(reason) => set({ reason })}
      />
      <Field className="gap-1">
        <FieldLabel htmlFor="ai-owner" className={fieldLabelClass}>
          {t('workbench.applet.ai.owner')}
        </FieldLabel>
        <Input
          id="ai-owner"
          name="owner"
          maxLength={40}
          value={draft.owner}
          onChange={(event) => set({ owner: event.target.value })}
        />
      </Field>
      <Field className="gap-1">
        <FieldLabel htmlFor="ai-rd-owner" className={fieldLabelClass}>
          {t('workbench.applet.ai.rd_owner')}
        </FieldLabel>
        <OfficeMenu
          id="ai-rd-owner"
          label={t('workbench.applet.ai.rd_owner')}
          placeholder={t('workbench.applet.ai.rd_owner')}
          value={draft.rdOwner}
          options={members.map((person) => ({ value: person.id, label: person.name }))}
          onChange={(rdOwner) => set({ rdOwner })}
        />
      </Field>
      <Field className="gap-1">
        <FieldLabel htmlFor="ai-days" className={fieldLabelClass}>
          {t('workbench.applet.ai.days')}
        </FieldLabel>
        <Input
          id="ai-days"
          name="days"
          maxLength={8}
          value={draft.days}
          onChange={(event) => set({ days: event.target.value })}
        />
      </Field>
      <Field className="gap-1">
        <FieldLabel htmlFor="ai-budget" className={fieldLabelClass}>
          {t('workbench.applet.ai.budget')}
        </FieldLabel>
        <Input
          id="ai-budget"
          name="budget"
          inputMode="decimal"
          maxLength={10}
          value={draft.budget}
          onChange={(event) => set({ budget: event.target.value })}
        />
      </Field>
      <RangeFields
        start={draft.start}
        end={draft.end}
        startId="ai-start"
        endId="ai-end"
        startMarker="workbench.applet.ai.start"
        endMarker="workbench.applet.ai.end"
        rangeMarker="workbench.applet.ai.range"
        onChange={(next) => set(next)}
      />
      <ChoiceField
        id="ai-type"
        label={t('workbench.applet.ai.type')}
        placeholder={t('workbench.applet.ai.type.placeholder')}
        value={draft.types}
        options={AI_TYPES}
        onValueChange={(types) => set({ types })}
      />
      <ReasonField
        id="ai-spec"
        name="spec"
        label={t('workbench.applet.ai.spec')}
        placeholder={t('workbench.applet.ai.spec.placeholder')}
        value={draft.spec}
        onValueChange={(spec) => set({ spec })}
      />
      <ReasonField
        id="ai-output"
        name="output"
        label={t('workbench.applet.ai.output')}
        placeholder={t('workbench.applet.ai.output.placeholder')}
        value={draft.output}
        onValueChange={(output) => set({ output })}
      />
      <button
        type="button"
        aria-pressed={draft.personal}
        data-ui="workbench.applet.ai.personal"
        className="h-9 rounded-[8px] bg-[#f5f5f5] px-3 text-left text-[13px] text-[#171717]"
        onClick={() => set({ personal: !draft.personal })}>
        {t('workbench.applet.ai.personal')}
      </button>
    </>
  )
}

function MeetingFields({ draft, onChange }: { draft: OfficeDrafts['meeting']; onChange: DraftChange }) {
  const { t } = useTranslation()
  const starts = QUARTER_HOURS.slice(0, -1)
  const ends = QUARTER_HOURS.filter((hour) => !draft.start || hour > draft.start)
  return (
    <>
      <Field className="gap-1">
        <FieldLabel htmlFor="meeting-subject" className={fieldLabelClass}>
          {t('workbench.applet.meeting.subject')}
        </FieldLabel>
        <Input
          id="meeting-subject"
          name="subject"
          required
          maxLength={80}
          placeholder={t('workbench.applet.meeting.subject.placeholder')}
          value={draft.subject}
          onChange={(event) =>
            onChange((current) => ({ ...current, meeting: { ...current.meeting, subject: event.target.value } }))
          }
        />
      </Field>
      <DayField
        id="meeting-day"
        label={t('workbench.applet.meeting.day')}
        value={draft.day}
        marker="workbench.applet.meeting.day"
        calendarMarker="workbench.applet.meeting.calendar"
        onChange={(day) => onChange((current) => ({ ...current, meeting: { ...current.meeting, day } }))}
      />
      <div className="grid grid-cols-2 gap-3">
        <ChoiceField
          id="meeting-start"
          label={t('workbench.applet.meeting.start')}
          placeholder={t('workbench.applet.meeting.time.placeholder')}
          value={draft.start}
          options={starts.map((hour) => ({ value: hour, labelKey: hour }))}
          onValueChange={(start) =>
            onChange((current) => ({
              ...current,
              meeting: {
                ...current.meeting,
                start,
                end: current.meeting.end > start ? current.meeting.end : ''
              }
            }))
          }
        />
        <ChoiceField
          id="meeting-end"
          label={t('workbench.applet.meeting.end')}
          placeholder={t('workbench.applet.meeting.time.placeholder')}
          value={draft.end}
          options={ends.map((hour) => ({ value: hour, labelKey: hour }))}
          onValueChange={(end) => onChange((current) => ({ ...current, meeting: { ...current.meeting, end } }))}
        />
      </div>
      <FieldHint id="meeting-time-hint">
        {draft.start && draft.end && !meetingSpanOk(draft.start, draft.end)
          ? t('workbench.applet.meeting.order')
          : null}
      </FieldHint>
      <Field className="gap-1">
        <FieldLabel htmlFor="meeting-place" className={fieldLabelClass}>
          {t('workbench.applet.meeting.place')}
        </FieldLabel>
        <Input
          id="meeting-place"
          name="place"
          required
          maxLength={80}
          placeholder={t('workbench.applet.meeting.place.placeholder')}
          value={draft.place}
          onChange={(event) =>
            onChange((current) => ({ ...current, meeting: { ...current.meeting, place: event.target.value } }))
          }
        />
      </Field>
      <ReasonField
        id="meeting-agenda"
        name="agenda"
        label={t('workbench.applet.meeting.agenda')}
        placeholder={t('workbench.applet.meeting.agenda.placeholder')}
        value={draft.agenda}
        onValueChange={(agenda) => onChange((current) => ({ ...current, meeting: { ...current.meeting, agenda } }))}
      />
    </>
  )
}

function SealFields({ draft, onChange }: { draft: OfficeDrafts['seal']; onChange: DraftChange }) {
  const { t } = useTranslation()
  return (
    <>
      <Field className="gap-1">
        <FieldLabel htmlFor="seal-document" className={fieldLabelClass}>
          {t('workbench.applet.seal.document')}
        </FieldLabel>
        <Input
          id="seal-document"
          name="document"
          required
          maxLength={80}
          placeholder={t('workbench.applet.seal.document.placeholder')}
          value={draft.document}
          onChange={(event) =>
            onChange((current) => ({ ...current, seal: { ...current.seal, document: event.target.value } }))
          }
        />
      </Field>
      <ChoiceField
        id="seal-kind"
        label={t('workbench.applet.seal.kind')}
        placeholder={t('workbench.applet.seal.kind.placeholder')}
        value={draft.kind}
        options={SEAL_KINDS}
        onValueChange={(kind) => onChange((current) => ({ ...current, seal: { ...current.seal, kind } }))}
      />
      <Field className="gap-1">
        <FieldLabel htmlFor="seal-copies" className={fieldLabelClass}>
          {t('workbench.applet.seal.copies')}
        </FieldLabel>
        <Input
          id="seal-copies"
          name="copies"
          required
          inputMode="numeric"
          maxLength={2}
          placeholder={t('workbench.applet.seal.copies.placeholder')}
          aria-invalid={draft.copies.trim() !== '' && !copiesOk(draft.copies)}
          aria-describedby="seal-copies-hint"
          value={draft.copies}
          onChange={(event) =>
            onChange((current) => ({ ...current, seal: { ...current.seal, copies: event.target.value } }))
          }
        />
        <FieldHint id="seal-copies-hint">
          {draft.copies.trim() !== '' && !copiesOk(draft.copies) ? t('workbench.applet.seal.copies.invalid') : null}
        </FieldHint>
      </Field>
      <ReasonField
        id="seal-reason"
        label={t('workbench.applet.seal.reason')}
        placeholder={t('workbench.applet.seal.reason.placeholder')}
        value={draft.reason}
        onValueChange={(reason) => onChange((current) => ({ ...current, seal: { ...current.seal, reason } }))}
      />
    </>
  )
}

function NoticeFields({ draft, onChange }: { draft: OfficeDrafts['notice']; onChange: DraftChange }) {
  const { t } = useTranslation()
  return (
    <>
      <Field className="gap-1">
        <FieldLabel htmlFor="notice-title" className={fieldLabelClass}>
          {t('workbench.applet.notice.title')}
        </FieldLabel>
        <Input
          id="notice-title"
          name="title"
          required
          maxLength={80}
          placeholder={t('workbench.applet.notice.title.placeholder')}
          value={draft.title}
          onChange={(event) =>
            onChange((current) => ({ ...current, notice: { ...current.notice, title: event.target.value } }))
          }
        />
      </Field>
      <ReasonField
        id="notice-body"
        name="body"
        label={t('workbench.applet.notice.body')}
        placeholder={t('workbench.applet.notice.body.placeholder')}
        value={draft.body}
        onValueChange={(body) => onChange((current) => ({ ...current, notice: { ...current.notice, body } }))}
      />
    </>
  )
}

function ReasonField({
  id,
  name = 'reason',
  label,
  placeholder,
  value,
  onValueChange
}: {
  id: string
  name?: string
  label: string
  placeholder: string
  value: string
  onValueChange: (value: string) => void
}) {
  return (
    <Field className="gap-1">
      <FieldLabel htmlFor={id} className={fieldLabelClass}>
        {label}
      </FieldLabel>
      <Textarea.Input
        id={id}
        name={name}
        required
        maxLength={2000}
        placeholder={placeholder}
        value={value}
        onValueChange={onValueChange}
        className="h-40 resize-none"
      />
    </Field>
  )
}

function FieldHint({ id, children }: { id: string; children: string | null }) {
  if (!children) return null
  return (
    <p id={id} className="m-0 text-[12px] leading-4 text-[#b42318]">
      {children}
    </p>
  )
}

function OfficeMenu({
  id,
  label,
  placeholder,
  value,
  options,
  onChange,
  allowClear = false,
  emptyText
}: {
  id: string
  label: string
  placeholder: string
  value: string
  options: readonly { value: string; label: string; icon?: ReactNode }[]
  onChange: (value: string) => void
  allowClear?: boolean
  emptyText?: string
}) {
  const triggerRef = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const [box, setBox] = useState<{ top: number; left: number; width: number } | null>(null)
  const selected = options.find((item) => item.value === value)
  const listId = `${id}-list`

  useEffect(() => {
    if (!open) return
    const place = () => {
      const node = triggerRef.current
      if (!node) return
      const rect = node.getBoundingClientRect()
      const height = Math.min(240, options.length === 0 ? 40 : options.length * 36 + 8)
      const below = rect.bottom + 4
      const top = below + height > window.innerHeight - 8 ? Math.max(8, rect.top - 4 - height) : below
      setBox({ top, left: rect.left, width: rect.width })
    }
    place()
    const close = (event: MouseEvent) => {
      const target = event.target
      if (!(target instanceof Node)) return
      if (triggerRef.current?.contains(target)) return
      if (document.getElementById(listId)?.contains(target)) return
      setOpen(false)
    }
    window.addEventListener('resize', place)
    window.addEventListener('scroll', place, true)
    document.addEventListener('mousedown', close)
    return () => {
      window.removeEventListener('resize', place)
      window.removeEventListener('scroll', place, true)
      document.removeEventListener('mousedown', close)
    }
  }, [listId, open, options.length])

  return (
    <>
      <button
        ref={triggerRef}
        id={id}
        type="button"
        role="combobox"
        aria-label={selected ? `${label} ${selected.label}` : label}
        aria-expanded={open}
        aria-controls={listId}
        onClick={() => setOpen((current) => !current)}
        className="inline-flex h-9 w-full items-center justify-between gap-2 rounded-[8px] bg-[#f5f5f5] px-3 text-left text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
        <span className={`flex min-w-0 items-center gap-2 ${selected ? 'text-[#171717]' : 'text-[#8a8a8a]'}`}>
          {selected?.icon}
          <span className="truncate">{selected ? selected.label : placeholder}</span>
        </span>
        <span className="inline-flex shrink-0 items-center gap-1 text-[#8a8a8a]">
          {allowClear && selected ? (
            <span
              role="button"
              tabIndex={-1}
              aria-label={placeholder}
              className="inline-flex size-5 items-center justify-center rounded-[4px] hover:bg-white"
              onMouseDown={(event) => event.stopPropagation()}
              onClick={(event) => {
                event.preventDefault()
                event.stopPropagation()
                onChange('')
                setOpen(false)
              }}>
              ×
            </span>
          ) : null}
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.75"
            aria-hidden>
            <path d="m6 9 6 6 6-6" />
          </svg>
        </span>
      </button>
      {open && box
        ? createPortal(
            <div
              id={listId}
              role="listbox"
              style={{ top: box.top, left: box.left, width: box.width }}
              className="fixed z-[80] max-h-60 overflow-y-auto rounded-[12px] border border-[#efefef] bg-white py-1 text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
              {options.length === 0 ? (
                <p className="m-0 px-3 py-2 text-[13px] text-[#8a8a8a]">{emptyText}</p>
              ) : (
                options.map((item) => (
                  <button
                    key={item.value}
                    type="button"
                    role="option"
                    aria-selected={item.value === value}
                    className={`flex w-full items-center gap-2 px-3 py-2 text-left text-[13px] ${
                      item.value === value ? 'bg-[#f5f5f5]' : 'bg-white hover:bg-[#f5f5f5]'
                    }`}
                    onMouseDown={() => {
                      onChange(item.value)
                      setOpen(false)
                    }}
                    onClick={() => {
                      onChange(item.value)
                      setOpen(false)
                    }}>
                    {item.icon}
                    <span>{item.label}</span>
                  </button>
                ))
              )}
            </div>,
            document.body
          )
        : null}
    </>
  )
}

function ChoiceField({
  id,
  label,
  placeholder,
  value,
  options,
  onValueChange
}: {
  id: string
  label: string
  placeholder: string
  value: string
  options: readonly { value: string; labelKey: string }[]
  onValueChange: (value: string) => void
}) {
  const { t } = useTranslation()
  return (
    <Field className="gap-1">
      <FieldLabel htmlFor={id} className={fieldLabelClass}>
        {label}
      </FieldLabel>
      <OfficeMenu
        id={id}
        label={label}
        placeholder={placeholder}
        value={value}
        options={options.map((item) => ({
          value: item.value,
          label: item.labelKey.startsWith('workbench.') ? t(item.labelKey) : item.labelKey
        }))}
        onChange={onValueChange}
      />
    </Field>
  )
}

function dayActivationEvent(event: { key: string; code?: string }): boolean {
  return (
    event.key === 'Enter' ||
    event.key === ' ' ||
    event.key === 'Spacebar' ||
    event.key === 'Space' ||
    event.code === 'Space' ||
    event.code === 'Enter' ||
    event.code === 'NumpadEnter'
  )
}

function DateField({
  id,
  label,
  value,
  marker,
  expanded,
  onToggle
}: {
  id: string
  label: string
  value: string
  marker: string
  expanded: boolean
  onToggle: () => void
}) {
  const { t } = useTranslation()
  return (
    <Field className="gap-1">
      <FieldLabel htmlFor={id} className={fieldLabelClass}>
        {label}
      </FieldLabel>
      <button
        id={id}
        type="button"
        data-ui={marker}
        aria-expanded={expanded}
        aria-label={value ? `${label} ${value}` : undefined}
        onClick={onToggle}
        className={`h-9 w-full rounded-[8px] bg-[#f5f5f5] px-3 text-left text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset ${
          value ? 'text-[#171717]' : 'text-[#8a8a8a]'
        }`}>
        {value || t('workbench.applet.leave.date')}
      </button>
    </Field>
  )
}

function RangeFields({
  start,
  end,
  startId,
  endId,
  startMarker,
  endMarker,
  rangeMarker,
  onChange
}: {
  start: string
  end: string
  startId: string
  endId: string
  startMarker: string
  endMarker: string
  rangeMarker: string
  onChange: (next: { start: string; end: string }) => void
}) {
  const { t } = useTranslation()
  const [openOn, setOpenOn] = useState<'start' | 'end' | null>(null)
  const from = parseOfficeDay(start)
  const to = parseOfficeDay(end)
  const days = inclusiveDays(start, end)
  const issue = dateSpanIssue(start, end)
  return (
    <Popover
      open={openOn != null}
      onOpenChange={(next) => {
        if (!next) setOpenOn(null)
      }}>
      <PopoverAnchor asChild>
        <div className="grid grid-cols-2 gap-3">
          <DateField
            id={startId}
            label={t('workbench.applet.leave.start')}
            value={start}
            marker={startMarker}
            expanded={openOn != null}
            onToggle={() => setOpenOn((current) => (current === 'start' ? null : 'start'))}
          />
          <DateField
            id={endId}
            label={t('workbench.applet.leave.end')}
            value={end}
            marker={endMarker}
            expanded={openOn != null}
            onToggle={() => setOpenOn((current) => (current === 'end' ? null : 'end'))}
          />
        </div>
      </PopoverAnchor>
      <OfficeCalendarPopover marker={rangeMarker}>
        <Calendar
          mode="range"
          weekStartsOn={1}
          captionLayout="label"
          navLayout="around"
          lang="zh-CN"
          showOutsideDays
          defaultMonth={from ?? new Date()}
          selected={from ? { from, to: to ?? undefined } : undefined}
          onSelect={(range) => {
            const next = { start: formatOfficeDay(range?.from), end: formatOfficeDay(range?.to) }
            onChange(next)
            if (next.start && next.end && next.start !== next.end) setOpenOn(null)
          }}
          {...calendarChrome(t)}
        />
      </OfficeCalendarPopover>
      <p data-ui="workbench.applet.leave.days" className="m-0 h-5 text-[13px] leading-5 text-[#3f3f3f]">
        {days != null && issue == null ? t('workbench.applet.leave.days', { days }) : null}
      </p>
      <FieldHint id={`${startId}-span`}>
        {issue === 'order'
          ? t('workbench.applet.leave.order')
          : issue === 'span'
            ? t('workbench.applet.leave.span')
            : null}
      </FieldHint>
    </Popover>
  )
}

function DayField({
  id,
  label,
  value,
  marker,
  calendarMarker,
  onChange
}: {
  id: string
  label: string
  value: string
  marker: string
  calendarMarker: string
  onChange: (value: string) => void
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const selected = parseOfficeDay(value)
  const chooseToday = () => {
    const next = formatOfficeDay(new Date())
    if (!next) return
    onChange(next)
    setOpen(false)
  }
  useEffect(() => {
    if (!open) return
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target
      if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return
      if (target instanceof HTMLElement && (target.isContentEditable || target.closest('[data-day]'))) return
      // A key that misses the day button still lands on the webview. Select today.
      if (!dayActivationEvent(event)) return
      event.preventDefault()
      event.stopPropagation()
      chooseToday()
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [open, onChange])
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverAnchor asChild>
        <div className="flex items-end gap-2">
          <div className="min-w-0 flex-1">
            <DateField
              id={id}
              label={label}
              value={value}
              marker={marker}
              expanded={open}
              onToggle={() => setOpen((current) => !current)}
            />
          </div>
          <button
            type="button"
            data-ui={`${marker}.today`}
            aria-label={t('workbench.applet.leave.calendar.today')}
            onClick={chooseToday}
            className="h-9 shrink-0 rounded-[8px] bg-[#f5f5f5] px-3 text-[13px] text-[#171717] outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
            {t('workbench.applet.leave.calendar.today')}
          </button>
        </div>
      </PopoverAnchor>
      <OfficeCalendarPopover marker={calendarMarker}>
        <Calendar
          mode="single"
          required
          weekStartsOn={1}
          captionLayout="label"
          navLayout="around"
          lang="zh-CN"
          showOutsideDays
          defaultMonth={selected ?? new Date()}
          selected={selected ?? undefined}
          onSelect={(day) => {
            // A second activation of the same day must not clear it. The popover
            // closes only after a real day is stored.
            const next = formatOfficeDay(day)
            if (!next) return
            onChange(next)
            setOpen(false)
          }}
          onDayKeyDown={(date, _modifiers, event) => {
            // The day grid selects from a click. Enter and Space do not. A
            // background key can reach this button when a pointer press does not.
            if (!dayActivationEvent(event)) return
            event.preventDefault()
            const next = formatOfficeDay(date)
            if (!next) return
            onChange(next)
            setOpen(false)
          }}
          {...calendarChrome(t)}
        />
      </OfficeCalendarPopover>
    </Popover>
  )
}

function OfficeCalendarPopover({ marker, children }: { marker: string; children: ReactNode }) {
  return (
    <PopoverContent
      align="start"
      side="bottom"
      sideOffset={8}
      collisionPadding={12}
      data-ui={marker}
      onOpenAutoFocus={(event) => event.preventDefault()}
      onCloseAutoFocus={(event) => event.preventDefault()}
      style={{ width: 'var(--radix-popover-trigger-width)' }}
      className="z-[80] rounded-[12px] border border-[#efefef] bg-white p-3 text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
      {children}
    </PopoverContent>
  )
}

function OfficeDayButton({
  day: _day,
  modifiers,
  onMouseDown,
  onClick,
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  day?: unknown
  modifiers?: { focused?: boolean }
  onMouseDown?: MouseEventHandler<HTMLButtonElement>
  onClick?: MouseEventHandler<HTMLButtonElement>
}) {
  const ref = useRef<HTMLButtonElement>(null)
  const pressed = useRef(false)
  const label = typeof props['aria-label'] === 'string' ? props['aria-label'] : undefined
  const rest = { ...props }
  delete rest['aria-label']
  useEffect(() => {
    if (modifiers?.focused) ref.current?.focus()
  }, [modifiers?.focused])
  const activate = (event: MouseEvent<HTMLElement>) => {
    if (event.defaultPrevented || event.button !== 0) return
    if (pressed.current) return
    // A background press reaches the inner text and does not finish a click.
    pressed.current = true
    onClick?.(event as MouseEvent<HTMLButtonElement>)
  }
  return (
    <button
      ref={ref}
      {...rest}
      onMouseDown={(event) => {
        onMouseDown?.(event)
        activate(event)
      }}
      onClick={(event) => {
        if (pressed.current) {
          pressed.current = false
          return
        }
        onClick?.(event)
      }}>
      <span aria-label={label} onMouseDown={activate}>
        {children}
      </span>
    </button>
  )
}

function calendarChrome(t: (key: string, values?: Record<string, unknown>) => string) {
  return {
    formatters: {
      formatCaption: (month: Date) =>
        t('workbench.applet.leave.calendar.caption', {
          year: month.getFullYear(),
          month: month.getMonth() + 1
        }),
      formatWeekdayName: (weekday: Date) => weekday.toLocaleDateString('zh-CN', { weekday: 'narrow' })
    },
    labels: {
      labelPrevious: () => t('workbench.applet.leave.calendar.previous'),
      labelNext: () => t('workbench.applet.leave.calendar.next'),
      labelDayButton: (date: Date) =>
        date.toLocaleDateString('zh-CN', { year: 'numeric', month: 'long', day: 'numeric', weekday: 'long' })
    },
    className: 'w-full p-0',
    classNames: officeCalendarClassNames,
    components: { MonthCaption: OfficeMonthCaption, DayButton: OfficeDayButton }
  }
}

function OfficeMonthCaption({
  className,
  style,
  children
}: {
  className?: string
  style?: CSSProperties
  children?: ReactNode
}) {
  return (
    <div className={className} style={style}>
      {children}
    </div>
  )
}

const officeCalendarClassNames = {
  root: 'rdp-root w-full',
  months: 'rdp-months relative flex w-full max-w-none flex-col',
  month: 'rdp-month relative flex w-full flex-col gap-2',
  month_caption:
    'rdp-month_caption flex h-10 w-full items-center justify-center px-10 text-[15px] font-medium text-[#171717]',
  caption_label: 'rdp-caption_label text-[15px] font-medium text-[#171717]',
  button_previous:
    'rdp-button_previous absolute top-1 left-0 z-10 inline-flex size-8 items-center justify-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f5f5f5] disabled:pointer-events-none disabled:opacity-40',
  button_next:
    'rdp-button_next absolute top-1 right-0 z-10 inline-flex size-8 items-center justify-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f5f5f5] disabled:pointer-events-none disabled:opacity-40',
  today: 'font-medium',
  month_grid: 'w-full table-fixed border-collapse',
  weekdays: 'w-full',
  weekday: 'w-[14.28%] pb-1 text-center text-[13px] font-normal text-[#8a8a8a]',
  week: 'w-full',
  day: 'group/day relative w-[14.28%] min-w-0 p-0.5 text-center text-[15px] text-[#171717]',
  day_button:
    'flex aspect-square w-full items-center justify-center rounded-[10px] text-[15px] font-normal text-inherit hover:bg-black/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10 disabled:pointer-events-none disabled:opacity-40 group-data-[selected=true]/day:hover:bg-white/15',
  selected: 'bg-[#171717] text-white',
  range_start: 'rounded-l-[10px] bg-[#171717] text-white',
  range_end: 'rounded-r-[10px] bg-[#171717] text-white',
  range_middle: 'bg-[#f2f3f5]! text-[#171717]!'
}

function LeaveTypeMark({ code, color }: { code: string; color: string }) {
  return (
    <svg data-leave-type={code} data-leave-swatch={color} viewBox="0 0 16 16" className="size-4 shrink-0" aria-hidden>
      <rect width="16" height="16" rx="4" fill={color} />
      <LeaveTypeGlyph code={code} />
    </svg>
  )
}

function LeaveTypeGlyph({ code }: { code: string }) {
  const ink = {
    fill: 'none',
    stroke: '#fff',
    strokeWidth: 1.4,
    strokeLinecap: 'round' as const,
    strokeLinejoin: 'round' as const
  }
  if (code === '1') {
    return (
      <>
        <circle cx="8" cy="5.6" r="1.5" fill="#fff" />
        <path d="M5.1 11.3c.5-1.6 1.6-2.3 2.9-2.3s2.4.7 2.9 2.3" {...ink} />
      </>
    )
  }
  if (code === '2') {
    return (
      <>
        <circle cx="6.2" cy="8" r="2.1" {...ink} />
        <circle cx="9.8" cy="8" r="2.1" {...ink} />
      </>
    )
  }
  if (code === '3') {
    return <path d="M8 4.2v7.6M4.2 8h7.6" {...ink} />
  }
  if (code === '4') {
    return (
      <>
        <circle cx="8" cy="8" r="2" {...ink} />
        <path
          d="M8 3.4v1.3M8 11.3v1.3M3.4 8h1.3M11.3 8h1.3M4.7 4.7l.9.9M10.4 10.4l.9.9M11.3 4.7l-.9.9M5.6 10.4l-.9.9"
          {...ink}
        />
      </>
    )
  }
  if (code === '5') return <path d="M5 4.2h6l-1.2 7.6H6.2L5 4.2Z" {...ink} />
  if (code === '6')
    return <path d="M8 11.6 5.4 9.1a1.8 1.8 0 0 1 2.6-2.5L8 6.7l.1-.1a1.8 1.8 0 0 1 2.5 2.5L8 11.6Z" {...ink} />
  if (code === '7') {
    return (
      <>
        <circle cx="6" cy="6.2" r="1.3" fill="#fff" />
        <circle cx="10.2" cy="6.4" r="1.1" fill="#fff" />
        <path
          d="M3.8 11.4c.4-1.5 1.3-2.2 2.4-2.2 1 0 1.8.6 2.2 1.6M8.2 11.4c.3-1.2 1-1.8 2-1.8 1 0 1.6.6 2 1.8"
          {...ink}
        />
      </>
    )
  }
  return (
    <>
      <circle cx="8" cy="6.2" r="1.4" fill="#fff" />
      <path d="M5.4 11.4c.4-1.4 1.3-2 2.6-2s2.2.6 2.6 2" {...ink} />
      <path d="M11.2 5.2 12 4.4M11.6 7.2h1.1" {...ink} />
    </>
  )
}
