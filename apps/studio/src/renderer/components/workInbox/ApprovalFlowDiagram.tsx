import { Check } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import type { WorkStatus } from '@renderer/workInbox/client'

import { approvalFlow, type ApprovalStep, type ApprovalStepId, type ApprovalStepState } from './approvalFlow'

type Layout = 'row' | 'column'

function traveled(state: ApprovalStepState): boolean {
  return state === 'done'
}

function lineClass(state: ApprovalStepState): string {
  return traveled(state) ? 'bg-[#171717]' : 'bg-[#e5e5e5]'
}

function nameClass(state: ApprovalStepState): string {
  if (state === 'current') return 'text-[12px] font-medium text-[#171717]'
  if (state === 'done') return 'text-[12px] text-[#3f3f3f]'
  return 'text-[12px] text-[#8a8a8a]'
}

function Marker({ state }: { state: ApprovalStepState }) {
  const base = 'relative z-10 grid size-4 shrink-0 place-items-center rounded-full'
  if (state === 'current') {
    return (
      <span aria-hidden className={`${base} bg-[#171717]`}>
        <span className="size-1.5 rounded-full bg-white" />
      </span>
    )
  }
  if (state === 'done') {
    return (
      <span aria-hidden className={`${base} bg-[#f2f3f5] text-[#171717]`}>
        <Check className="size-2.5" strokeWidth={2.4} />
      </span>
    )
  }
  if (state === 'skipped') {
    return <span aria-hidden className={`${base} border border-dashed border-[#c5c5c5] bg-white`} />
  }
  return <span aria-hidden className={`${base} border border-[#e5e5e5] bg-white`} />
}

function stepName(id: ApprovalStepId, t: (key: string) => string): string {
  if (id === 'start') return t('work.inbox.flow.start')
  if (id === 'review') return t('work.inbox.flow.review')
  return t('work.inbox.flow.end')
}

function stateName(state: ApprovalStepState, t: (key: string) => string): string {
  if (state === 'done') return t('work.inbox.flow.state.done')
  if (state === 'current') return t('work.inbox.flow.state.current')
  if (state === 'waiting') return t('work.inbox.flow.state.waiting')
  return t('work.inbox.flow.state.skipped')
}

function outcome(status: WorkStatus, t: (key: string) => string): string {
  if (status === 'approved') return t('work.inbox.status.approved')
  if (status === 'rejected') return t('work.inbox.status.rejected')
  if (status === 'cancelled') return t('work.inbox.status.cancelled')
  return t('work.inbox.status.pending')
}

function caption(step: ApprovalStep, status: WorkStatus, t: (key: string) => string): string {
  const state = stateName(step.state, t)
  if (step.state === 'current' && step.id === 'end') return `${state} · ${outcome(status, t)}`
  if (step.state === 'current' && step.id === 'review') {
    return `${state} · ${t('work.inbox.flow.review.mode')}`
  }
  return state
}

function visibleNames(names: string[]): string[] {
  return names.map((name) => name.trim()).filter((name) => name.length > 0)
}

function People({ names, singleLine }: { names: string[]; singleLine: boolean }) {
  if (names.length === 0) return null
  if (singleLine) {
    return (
      <span
        data-ui="work.inbox.flow.people"
        className="mt-1 block w-full truncate text-center text-[12px] leading-4 text-[#3f3f3f]">
        {names.map((person, index) => (
          <span key={`${person}-${index}`}>
            {index > 0 ? (
              <span aria-hidden className="text-[#8a8a8a]">
                、
              </span>
            ) : null}
            <span data-ui="work.inbox.flow.person">{person}</span>
          </span>
        ))}
      </span>
    )
  }
  return (
    <span data-ui="work.inbox.flow.people" className="mt-1 flex max-w-full flex-wrap gap-1">
      {names.map((person, index) => (
        <span
          key={`${person}-${index}`}
          data-ui="work.inbox.flow.person"
          className="max-w-full truncate rounded-[4px] bg-[#f2f3f5] px-1.5 text-[12px] leading-5 text-[#3f3f3f]">
          {person}
        </span>
      ))}
    </span>
  )
}

function formatStamp(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  const pad = (part: number) => String(part).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}

export function ApprovalFlowDiagram({
  status,
  layout,
  assigneeNames = [],
  deciderName = '',
  decidedAt = '',
  comment = ''
}: {
  status: WorkStatus
  layout: Layout
  assigneeNames?: string[]
  deciderName?: string
  decidedAt?: string
  comment?: string
}) {
  const { t } = useTranslation()
  const steps = approvalFlow(status)
  const row = layout === 'row'
  const people = visibleNames(assigneeNames)
  const decider = deciderName.trim()
  const note = comment.trim()
  return (
    <span className={row ? 'mt-1 block w-full' : 'mt-4 block'}>
      {row ? null : <span className="mb-2 block text-[12px] text-[#8a8a8a]">{t('work.inbox.flow')}</span>}
      <span
        role="list"
        aria-label={t('work.inbox.flow')}
        data-ui="work.inbox.flow"
        className={row ? 'flex w-full items-start' : 'flex flex-col'}>
        {steps.map((step, index) => {
          const previous = index > 0 ? steps[index - 1] : null
          const text = caption(step, status, t)
          const name = stepName(step.id, t)
          return (
            <span
              key={step.id}
              role="listitem"
              data-ui="work.inbox.flow.step"
              data-step={step.id}
              data-state={step.state}
              aria-current={step.state === 'current' ? 'step' : undefined}
              aria-label={`${name}, ${text}`}
              className={
                row ? 'relative flex min-w-0 flex-1 flex-col items-center px-1 text-center' : 'flex items-stretch gap-3'
              }>
              {row && previous ? (
                <span aria-hidden className={`absolute top-2 right-1/2 left-0 h-px ${lineClass(previous.state)}`} />
              ) : null}
              {row ? (
                <Marker state={step.state} />
              ) : (
                <span className="flex w-4 flex-col items-center">
                  <Marker state={step.state} />
                  {index < steps.length - 1 ? (
                    <span aria-hidden className={`my-1 w-px flex-1 ${lineClass(step.state)}`} />
                  ) : null}
                </span>
              )}
              <span
                className={
                  row
                    ? 'mt-1 flex w-full min-w-0 flex-col items-center'
                    : `flex min-w-0 flex-col ${index < steps.length - 1 ? 'pb-3' : ''}`
                }>
                <span className={nameClass(step.state)}>{name}</span>
                <span className="text-[11px] leading-4 text-[#8a8a8a]">{text}</span>
                {step.id === 'review' && step.state === 'current' ? <People names={people} singleLine={row} /> : null}
                {step.id === 'review' && step.state === 'done' && !row ? (
                  <>
                    {decider ? (
                      <span data-ui="work.inbox.flow.decider" className="text-[12px] text-[#3f3f3f]">
                        {decider}
                      </span>
                    ) : null}
                    {decidedAt.trim() ? (
                      <time dateTime={decidedAt} className="text-[11px] leading-4 text-[#8a8a8a]">
                        {formatStamp(decidedAt)}
                      </time>
                    ) : null}
                    {note ? (
                      <span data-ui="work.inbox.flow.comment" className="text-[12px] break-words text-[#3f3f3f]">
                        {note}
                      </span>
                    ) : null}
                  </>
                ) : null}
              </span>
            </span>
          )
        })}
      </span>
    </span>
  )
}
