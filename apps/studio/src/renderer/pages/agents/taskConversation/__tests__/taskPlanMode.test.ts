import { describe, expect, it } from 'vitest'

import type { CherryUIMessage } from '@shared/data/types/message'

import { taskPlanMode, supportsTaskPlanning, planReviewIsCurrent } from '../taskPlanMode'

const request: CherryUIMessage = {
  id: 'u',
  role: 'user',
  parts: [
    { type: 'text', text: 'Update note.txt' },
    { type: 'data-task-plan', data: { mode: 'plan' } }
  ]
}
const plan: CherryUIMessage = {
  id: 'p',
  role: 'assistant',
  parts: [{ type: 'text', text: '1. Read note.txt\n2. Update the first line' }],
  metadata: { status: 'success' }
}

describe('taskPlanMode', () => {
  it('refuses explicitly non-local runtime and composer actions without guessing from the prompt', () => {
    expect(supportsTaskPlanning({ text: 'Use a cloud model and Paperloom', tokens: [] }, true)).toBe(true)
    expect(supportsTaskPlanning({ text: 'Keep this draft', tokens: [] }, false)).toBe(false)
    expect(
      supportsTaskPlanning(
        {
          text: 'Keep this draft',
          tokens: [
            {
              id: 'agent',
              index: 0,
              textOffset: 0,
              kind: 'reference',
              label: 'Cloud agent',
              payload: { agentRuntime: { runtime: 'cloud', harness: 'builtin' } }
            }
          ]
        },
        true
      )
    ).toBe(false)
  })

  it('changes the review revision when a successful plan is revised under the same message identity', () => {
    const before = taskPlanMode([request, plan])
    const after = taskPlanMode([request, { ...plan, parts: [{ type: 'text', text: 'Revised plan' }] }])
    expect(after.readyMessageId).toBe(before.readyMessageId)
    expect(after.readyRevision).not.toBe(before.readyRevision)
  })

  it('withdraws review callbacks after member, session, message or content revision changes', () => {
    const owner = { sessionId: 's1', memberId: 'member-a', planMessageId: 'plan', planRevision: 'content-1' }
    expect(planReviewIsCurrent(owner, owner)).toBe(true)
    for (const changed of [
      { ...owner, sessionId: 's2' },
      { ...owner, memberId: 'member-b' },
      { ...owner, planMessageId: 'new-plan' },
      { ...owner, planRevision: 'content-2' },
      { ...owner, planMessageId: undefined }
    ])
      expect(planReviewIsCurrent(owner, changed)).toBe(false)
    expect(planReviewIsCurrent({ ...owner, planRevision: undefined }, owner)).toBe(false)
  })

  it('offers only a completed plan from an explicitly planned turn', () => {
    expect(taskPlanMode([request, plan]).readyMessageId).toBe('p')
    expect(taskPlanMode([request, { ...plan, metadata: { status: 'pending' } }]).readyMessageId).toBeUndefined()
    expect(
      taskPlanMode([{ ...request, parts: [{ type: 'text', text: 'Plan mode: plan this task' }] }, plan]).messageIds.size
    ).toBe(0)
  })

  it('offers a successful nonempty canonical plan without terminal prose', () => {
    const structured = {
      type: 'dynamic-tool' as const,
      toolName: 'update_plan',
      toolCallId: 'plan-only',
      state: 'output-available' as const,
      input: { variant: 'TodoWrite', plan: [{ step: 'Read note.txt', status: 'pending' }] },
      output: {}
    }
    expect(taskPlanMode([request, { ...plan, parts: [structured] }]).readyMessageId).toBe('p')
  })

  it('refuses an empty canonical plan even when successful text accompanies it', () => {
    const empty = {
      type: 'dynamic-tool' as const,
      toolName: 'update_plan',
      toolCallId: 'empty-plan',
      state: 'output-available' as const,
      input: { variant: 'TodoWrite', plan: [] },
      output: {}
    }
    for (const parts of [[empty], [...plan.parts, empty]])
      expect(taskPlanMode([request, { ...plan, parts }]).readyMessageId).toBeUndefined()
  })

  it('refuses unknown, errored and unfinished canonical plan-only results', () => {
    const input = { variant: 'TodoWrite', plan: [{ step: 'Read note.txt', status: 'pending' }] }
    const cases = [
      {
        type: 'dynamic-tool' as const,
        toolName: 'update_plan',
        toolCallId: 'unknown',
        state: 'output-available' as const,
        input: { variant: 'TodoWrite', plan: [{ step: 'Read', status: 'invented' }] },
        output: {}
      },
      {
        type: 'dynamic-tool' as const,
        toolName: 'update_plan',
        toolCallId: 'failed',
        state: 'output-error' as const,
        input,
        errorText: 'failed'
      },
      {
        type: 'dynamic-tool' as const,
        toolName: 'update_plan',
        toolCallId: 'unfinished',
        state: 'input-available' as const,
        input
      }
    ]
    for (const part of cases) expect(taskPlanMode([request, { ...plan, parts: [part] }]).readyMessageId).toBeUndefined()
  })

  it('refuses an unknown structured plan state even when the assistant row succeeded', () => {
    const malformed = {
      type: 'dynamic-tool',
      toolName: 'TodoWrite',
      toolCallId: 'plan-call',
      state: 'output-available',
      input: { variant: 'TodoWrite', plan: [{ step: 'Unverified', status: 'invented' }] },
      output: {}
    } as const
    expect(taskPlanMode([request, { ...plan, parts: [...plan.parts, malformed] }]).readyMessageId).toBeUndefined()
  })

  it('withdraws the old execute action after a follow-up or an interrupted plan', () => {
    expect(
      taskPlanMode([request, plan, { id: 'next', role: 'user', parts: [{ type: 'text', text: 'Revise the plan' }] }])
        .readyMessageId
    ).toBeUndefined()
    expect(taskPlanMode([request, { ...plan, metadata: { status: 'error' } }]).readyMessageId).toBeUndefined()
    expect(taskPlanMode([request, { ...plan, parts: [] }]).readyMessageId).toBeUndefined()
  })

  it('keeps plan identity across execution, refinement, and a later planning turn', () => {
    const chat: CherryUIMessage = { id: 'chat', role: 'user', parts: [{ type: 'text', text: 'Summarize the task' }] }
    const response = { ...plan, id: 'answer' }
    const messages = [request, plan, chat, response]
    expect(taskPlanMode(messages).readyMessageId).toBeUndefined()
    expect([...taskPlanMode(messages).messageIds]).toEqual(['p'])
    const revised = { ...plan, id: 'revised' }
    const next = taskPlanMode([...messages, { ...request, id: 'revise' }, revised])
    expect(next.readyMessageId).toBe('revised')
    expect([...next.messageIds]).toEqual(['p', 'revised'])
  })
})
