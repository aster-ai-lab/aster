import type { WorkKind, WorkStatus } from '@renderer/workInbox/client'

export type ApprovalStepId = 'start' | 'review' | 'end'
export type ApprovalStepState = 'done' | 'current' | 'waiting' | 'skipped'

export type ApprovalStep = {
  id: ApprovalStepId
  state: ApprovalStepState
}

/** Notice is not an approval, so it has no approval graph. */
export function showsApprovalFlow(kind: WorkKind): boolean {
  return kind !== 'notice'
}

/** Submit is done. One assignee decision ends the review. Withdrawal skips it. */
export function approvalFlow(status: WorkStatus): ApprovalStep[] {
  if (status === 'pending') {
    return [
      { id: 'start', state: 'done' },
      { id: 'review', state: 'current' },
      { id: 'end', state: 'waiting' }
    ]
  }
  if (status === 'cancelled') {
    return [
      { id: 'start', state: 'done' },
      { id: 'review', state: 'skipped' },
      { id: 'end', state: 'current' }
    ]
  }
  return [
    { id: 'start', state: 'done' },
    { id: 'review', state: 'done' },
    { id: 'end', state: 'current' }
  ]
}
