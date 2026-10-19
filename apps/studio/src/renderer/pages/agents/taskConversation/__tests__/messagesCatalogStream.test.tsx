import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { TaskConversationTurn } from '@renderer/pages/agents/taskConversation/TaskConversationMessage'
import type { CherryMessagePart } from '@shared/data/types/message'

describe('messages catalog task stream', () => {
  it('groups read, meeting-minutes, and calendar, then keeps prose structure', () => {
    const { container } = render(
      <TaskConversationTurn
        parts={
          [
            {
              type: 'tool-read',
              toolCallId: 'call-read',
              state: 'output-available',
              input: { path: 'notes/meeting.md' },
              output: 'ok'
            },
            {
              type: 'dynamic-tool',
              toolName: 'meeting-minutes',
              toolCallId: 'call-minutes',
              state: 'output-available',
              input: {},
              output: 'ok'
            },
            {
              type: 'dynamic-tool',
              toolName: 'calendar',
              toolCallId: 'call-calendar',
              state: 'output-available',
              input: {},
              output: 'ok'
            },
            { type: 'text', text: '# 结论\n\n- 待办\n\n```\ncode\n```' }
          ] as CherryMessagePart[]
        }
        streaming={false}
      />
    )

    const group = container.querySelector('[data-task-tool-group]')
    expect(group).not.toBeNull()
    expect(group?.querySelector('[data-task-tool-name="read"][data-tool-call="true"]')).not.toBeNull()
    expect(group?.querySelector('[data-task-tool-name="meeting-minutes"][data-tool-call="true"]')).not.toBeNull()
    expect(group?.querySelector('[data-task-tool-name="calendar"][data-tool-call="true"]')).not.toBeNull()
    const prose = container.querySelector('[data-task-prose]')
    expect(prose?.querySelector('h1, h2, h3')).not.toBeNull()
    expect(prose?.querySelector('ul, ol')).not.toBeNull()
    expect(prose?.querySelector('pre, code')).not.toBeNull()
  })
})
