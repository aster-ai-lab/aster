import { isToolUIPart } from 'ai'
import { createContext } from 'react'

import type { CherryUIMessage } from '@shared/data/types/message'

import { paperloomDraftQuestion } from '../paperloom/paperloomHandoff'
import { taskPlanSteps } from './taskPlanProjection'

export const TaskPlanningTurnsContext = createContext<ReadonlySet<string>>(new Set())

/** The submitted turn owns its mode, even when the composer switch changes later. */
export function taskPlanMode(messages: readonly CherryUIMessage[]) {
  const messageIds = new Set<string>()
  let planning = false
  for (const message of messages) {
    if (message.role === 'user') {
      planning = message.parts.some((part) => part.type === 'data-task-plan' && part.data?.mode === 'plan')
    } else if (message.role === 'assistant' && planning) {
      messageIds.add(message.id)
    }
  }
  const latest = messages.at(-1)
  const readyMessageId =
    latest?.role === 'assistant' &&
    messageIds.has(latest.id) &&
    latest.metadata?.status === 'success' &&
    !latest.parts.some((part) => {
      if (!isToolUIPart(part)) return false
      if (part.state === 'output-error' || part.state === 'approval-requested') return true
      const input = part.input
      return (
        !!input &&
        typeof input === 'object' &&
        'variant' in input &&
        input.variant === 'TodoWrite' &&
        !taskPlanSteps(input)?.length
      )
    }) &&
    latest.parts.some(
      (part) =>
        (part.type === 'text' && part.text.trim()) ||
        (isToolUIPart(part) && part.state === 'output-available' && Boolean(taskPlanSteps(part.input)?.length))
    )
      ? latest.id
      : undefined
  return { messageIds, readyMessageId, readyRevision: readyMessageId ? JSON.stringify(latest?.parts) : undefined }
}

/** Non-ACP actions cannot promise the local engine's read-only planning policy. */
export function supportsTaskPlanning(
  draft: Pick<import('@renderer/components/composer/tokens').ComposerSerializedDraft, 'text' | 'tokens'>,
  localMode: boolean
): boolean {
  if (!localMode || paperloomDraftQuestion(draft) !== null) return false
  return !draft.tokens.some(
    (token) =>
      token.payload &&
      typeof token.payload === 'object' &&
      'agentRuntime' in token.payload &&
      token.payload.agentRuntime !== undefined
  )
}

export type PlanReviewBinding = {
  sessionId: string
  memberId: string | null
  planMessageId?: string
  planRevision?: string
}

/** Review actions own exact message content and member/session, including across asynchronous preflight. */
export function planReviewIsCurrent(expected: PlanReviewBinding, current: PlanReviewBinding): boolean {
  return Boolean(
    expected.planMessageId &&
    expected.planRevision &&
    expected.sessionId === current.sessionId &&
    expected.memberId === current.memberId &&
    expected.planMessageId === current.planMessageId &&
    expected.planRevision === current.planRevision
  )
}
