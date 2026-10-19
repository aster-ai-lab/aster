/**
 * Office forms submit a field object. The server owns the title and computed days.
 */

export type LeaveDraft = { type: string; start: string; end: string; reason: string }
export type ApprovalDraft = { title: string; detail: string }
export type TravelDraft = {
  destination: string
  start: string
  end: string
  reason: string
  booking: boolean
  estimate: string
}
export type ExpenseDraft = { amount: string; category: string; date: string; reason: string; travelId: string }
export type CorrectionDraft = { date: string; type: string; time: string; reason: string }
export type ProbationDraft = { result: string; deferTo: string; summary: string }
export type AiResourceDraft = {
  project: string
  reason: string
  owner: string
  rdOwner: string
  days: string
  start: string
  end: string
  types: string
  spec: string
  output: string
  personal: boolean
  budget: string
}
export type MeetingDraft = { subject: string; day: string; start: string; end: string; place: string; agenda: string }
export type SealDraft = { document: string; kind: string; copies: string; reason: string }
export type NoticeDraft = { title: string; body: string }

export type OfficeDrafts = {
  leave: LeaveDraft
  approval: ApprovalDraft
  travel: TravelDraft
  expense: ExpenseDraft
  meeting: MeetingDraft
  seal: SealDraft
  notice: NoticeDraft
  correction: CorrectionDraft
  probation: ProbationDraft
  aiResource: AiResourceDraft
}

export type OfficeFormId =
  | 'approval'
  | 'leave'
  | 'travel'
  | 'expense'
  | 'meeting'
  | 'seal'
  | 'notice'
  | 'correction'
  | 'clock'
  | 'probation'
  | 'ai-resource'

export type FormCheckpoint = {
  drafts: OfficeDrafts
  submitted: boolean
  submitFailed: boolean
  routed: boolean
}

export const LEAVE_TYPES = [
  { code: '1', value: '事假', labelKey: 'workbench.applet.leave.type.personal', color: '#3B78F0' },
  { code: '2', value: '婚假', labelKey: 'workbench.applet.leave.type.marriage', color: '#E0568A' },
  { code: '3', value: '病假', labelKey: 'workbench.applet.leave.type.sick', color: '#E15B5B' },
  { code: '4', value: '年假', labelKey: 'workbench.applet.leave.type.annual', color: '#F08A3A' },
  { code: '5', value: '丧假', labelKey: 'workbench.applet.leave.type.bereavement', color: '#6E5CE6' },
  { code: '6', value: '产假', labelKey: 'workbench.applet.leave.type.maternity', color: '#2A9BC4' },
  { code: '7', value: '陪产假', labelKey: 'workbench.applet.leave.type.paternity', color: '#2F9B73' },
  { code: '8', value: '育儿假', labelKey: 'workbench.applet.leave.type.childcare', color: '#69B9C9' }
] as const

export const EXPENSE_CATEGORIES = [
  { value: '交通', labelKey: 'workbench.applet.expense.category.transport' },
  { value: '餐饮', labelKey: 'workbench.applet.expense.category.meal' },
  { value: '住宿', labelKey: 'workbench.applet.expense.category.lodging' },
  { value: '其他', labelKey: 'workbench.applet.expense.category.other' }
] as const

export const SEAL_KINDS = [
  { value: '公章', labelKey: 'workbench.applet.seal.kind.official' },
  { value: '合同章', labelKey: 'workbench.applet.seal.kind.contract' },
  { value: '财务章', labelKey: 'workbench.applet.seal.kind.finance' },
  { value: '法人章', labelKey: 'workbench.applet.seal.kind.legal' }
] as const

/** Quarter hours from 08:00 through 20:00. */
export const QUARTER_HOURS: readonly string[] = Array.from({ length: ((20 - 8) * 60) / 15 + 1 }, (_, index) => {
  const minutes = 8 * 60 + index * 15
  return `${String(Math.floor(minutes / 60)).padStart(2, '0')}:${String(minutes % 60).padStart(2, '0')}`
})

export function emptyDrafts(): OfficeDrafts {
  return {
    leave: { type: '', start: '', end: '', reason: '' },
    approval: { title: '', detail: '' },
    travel: { destination: '', start: '', end: '', reason: '', booking: false, estimate: '' },
    expense: { amount: '', category: '', date: '', reason: '', travelId: '' },
    meeting: { subject: '', day: '', start: '', end: '', place: '', agenda: '' },
    seal: { document: '', kind: '', copies: '', reason: '' },
    notice: { title: '', body: '' },
    correction: { date: '', type: '', time: '', reason: '' },
    probation: { result: '', deferTo: '', summary: '' },
    aiResource: {
      project: '',
      reason: '',
      owner: '',
      rdOwner: '',
      days: '',
      start: '',
      end: '',
      types: '',
      spec: '',
      output: '',
      personal: false,
      budget: ''
    }
  }
}

export function parseOfficeDay(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value)
  if (!match) return null
  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  const date = new Date(year, month - 1, day)
  if (date.getFullYear() !== year || date.getMonth() !== month - 1 || date.getDate() !== day) return null
  return date
}

export function formatOfficeDay(date: Date | undefined): string {
  if (!date) return ''
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${date.getFullYear()}-${month}-${day}`
}

export function inclusiveDays(start: string, end: string): number | null {
  if (!parseOfficeDay(start) || !parseOfficeDay(end)) return null
  const from = Date.parse(`${start}T00:00:00`)
  const to = Date.parse(`${end}T00:00:00`)
  if (Number.isNaN(from) || Number.isNaN(to) || to < from) return null
  return Math.round((to - from) / 86_400_000) + 1
}

export function expenseAmountOk(value: string): boolean {
  const trimmed = value.trim()
  if (!/^(?:0|[1-9]\d{0,6})(?:\.\d{1,2})?$/.test(trimmed)) return false
  const amount = Number(trimmed)
  return Number.isFinite(amount) && amount > 0
}

export function copiesOk(value: string): boolean {
  return /^[1-9]\d?$/.test(value.trim())
}

export function meetingSpanOk(start: string, end: string): boolean {
  return QUARTER_HOURS.includes(start) && QUARTER_HOURS.includes(end) && end > start
}

/** `order` when the end is before the start. `span` when the inclusive length exceeds 3660 days. */
export function dateSpanIssue(start: string, end: string): 'order' | 'span' | null {
  if (!parseOfficeDay(start) || !parseOfficeDay(end)) return null
  const days = inclusiveDays(start, end)
  if (days == null) return 'order'
  if (days > 3660) return 'span'
  return null
}

export function officeReady(id: OfficeFormId, drafts: OfficeDrafts): boolean {
  if (id === 'approval') return drafts.approval.title.trim() !== '' && drafts.approval.detail.trim() !== ''
  if (id === 'leave') {
    return (
      drafts.leave.type !== '' &&
      dateSpanIssue(drafts.leave.start, drafts.leave.end) == null &&
      parseOfficeDay(drafts.leave.start) != null &&
      parseOfficeDay(drafts.leave.end) != null &&
      drafts.leave.reason.trim() !== ''
    )
  }
  if (id === 'travel') {
    return (
      drafts.travel.destination.trim() !== '' &&
      dateSpanIssue(drafts.travel.start, drafts.travel.end) == null &&
      parseOfficeDay(drafts.travel.start) != null &&
      parseOfficeDay(drafts.travel.end) != null &&
      drafts.travel.reason.trim() !== ''
    )
  }
  if (id === 'expense') {
    return (
      expenseAmountOk(drafts.expense.amount) &&
      EXPENSE_CATEGORIES.some((item) => item.value === drafts.expense.category) &&
      parseOfficeDay(drafts.expense.date) != null &&
      drafts.expense.reason.trim() !== ''
    )
  }
  if (id === 'meeting') {
    return (
      drafts.meeting.subject.trim() !== '' &&
      parseOfficeDay(drafts.meeting.day) != null &&
      meetingSpanOk(drafts.meeting.start, drafts.meeting.end) &&
      drafts.meeting.place.trim() !== '' &&
      drafts.meeting.agenda.trim() !== ''
    )
  }
  if (id === 'seal') {
    return (
      drafts.seal.document.trim() !== '' &&
      SEAL_KINDS.some((item) => item.value === drafts.seal.kind) &&
      copiesOk(drafts.seal.copies) &&
      drafts.seal.reason.trim() !== ''
    )
  }
  if (id === 'clock') {
    return drafts.correction.type === '上班' || drafts.correction.type === '下班'
  }
  if (id === 'correction') {
    return (
      parseOfficeDay(drafts.correction.date) != null &&
      (drafts.correction.type === '上班' || drafts.correction.type === '下班') &&
      /^\d{2}:\d{2}$/.test(drafts.correction.time.trim()) &&
      drafts.correction.reason.trim() !== ''
    )
  }
  if (id === 'probation') {
    const result = drafts.probation.result
    const known = result === '同意转正' || result === '延期' || result === '不予转正'
    return (
      known &&
      drafts.probation.summary.trim() !== '' &&
      (result !== '延期' || parseOfficeDay(drafts.probation.deferTo) != null)
    )
  }
  if (id === 'ai-resource') {
    const draft = drafts.aiResource
    const text = [draft.project, draft.reason, draft.owner, draft.rdOwner, draft.types, draft.spec, draft.output]
    const dates =
      parseOfficeDay(draft.start) != null &&
      parseOfficeDay(draft.end) != null &&
      dateSpanIssue(draft.start, draft.end) == null
    const effort = draft.days.trim() !== '' || expenseAmountOk(draft.budget)
    return text.every((item) => item.trim() !== '') && dates && effort
  }
  return drafts.notice.title.trim() !== '' && drafts.notice.body.trim() !== ''
}

/** Field object for one office form. Days are not included; the server computes them. */
export function officeFields(id: OfficeFormId, drafts: OfficeDrafts): Record<string, string> | null {
  if (!officeReady(id, drafts)) return null
  if (id === 'approval') {
    return { title: drafts.approval.title.trim(), detail: drafts.approval.detail.trim() }
  }
  if (id === 'leave') {
    return {
      leave_type: drafts.leave.type,
      start_date: drafts.leave.start,
      end_date: drafts.leave.end,
      reason: drafts.leave.reason.trim()
    }
  }
  if (id === 'travel') {
    const fields: Record<string, string> = {
      destination: drafts.travel.destination.trim(),
      start_date: drafts.travel.start,
      end_date: drafts.travel.end,
      reason: drafts.travel.reason.trim()
    }
    if (drafts.travel.booking) fields.booking = 'true'
    if (drafts.travel.estimate.trim()) fields.amount = drafts.travel.estimate.trim()
    return fields
  }
  if (id === 'expense') {
    const fields: Record<string, string> = {
      amount: drafts.expense.amount.trim(),
      category: drafts.expense.category,
      spent_on: drafts.expense.date,
      detail: drafts.expense.reason.trim()
    }
    if (drafts.expense.travelId.trim()) fields.travel_id = drafts.expense.travelId.trim()
    return fields
  }
  if (id === 'meeting') {
    return {
      title: drafts.meeting.subject.trim(),
      day: drafts.meeting.day,
      start_time: drafts.meeting.start,
      end_time: drafts.meeting.end,
      place: drafts.meeting.place.trim(),
      agenda: drafts.meeting.agenda.trim()
    }
  }
  if (id === 'seal') {
    return {
      document: drafts.seal.document.trim(),
      seal_type: drafts.seal.kind,
      copies: drafts.seal.copies.trim(),
      purpose: drafts.seal.reason.trim()
    }
  }
  if (id === 'clock') {
    return { punch_type: drafts.correction.type }
  }
  if (id === 'correction') {
    return {
      punch_date: drafts.correction.date,
      punch_type: drafts.correction.type,
      corrected_time: drafts.correction.time.trim(),
      reason: drafts.correction.reason.trim()
    }
  }
  if (id === 'probation') {
    const fields: Record<string, string> = {
      result: drafts.probation.result,
      summary: drafts.probation.summary.trim()
    }
    if (drafts.probation.deferTo) fields.defer_to = drafts.probation.deferTo
    return fields
  }
  if (id === 'ai-resource') {
    const draft = drafts.aiResource
    const fields: Record<string, string> = {
      project_name: draft.project.trim(),
      test_reason: draft.reason.trim(),
      owner: draft.owner.trim(),
      rd_owner: draft.rdOwner.trim(),
      start_date: draft.start,
      end_date: draft.end,
      resource_type: draft.types,
      resource_note: draft.spec.trim(),
      expected_output: draft.output.trim()
    }
    if (draft.days.trim()) fields.days = draft.days.trim()
    if (draft.budget.trim()) fields.budget = draft.budget.trim()
    if (draft.personal) fields.personal_data = 'true'
    return fields
  }
  return { title: drafts.notice.title.trim(), detail: drafts.notice.body.trim() }
}
