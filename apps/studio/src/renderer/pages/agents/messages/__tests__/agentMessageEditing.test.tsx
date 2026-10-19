import { renderHook } from '@testing-library/react'
import { expect, it, vi } from 'vitest'

import type { Topic } from '@renderer/types/topic'
import type { CherryUIMessage } from '@shared/data/types/message'

import { useAgentMessageListProviderValue } from '../agentMessageListAdapter'

vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }))

it('offers editing on earlier user messages and disables it while the session is busy', () => {
  const messages: CherryUIMessage[] = [
    { id: 'user-a', role: 'user', parts: [{ type: 'text', text: 'Question A' }] },
    { id: 'assistant-a', role: 'assistant', parts: [{ type: 'text', text: 'Answer A' }] },
    { id: 'user-b', role: 'user', parts: [{ type: 'text', text: 'Question B' }] }
  ]
  const { result, rerender } = renderHook(
    ({ editBusy }) =>
      useAgentMessageListProviderValue({
        topic: { id: 'agent-session:source', assistantId: 'agent-1', name: 'Source', messages: [] } as unknown as Topic,
        messages,
        partsByMessageId: {},
        isLoading: false,
        messageNavigation: 'anchor',
        startEditing: async () => {},
        editBusy
      }),
    { initialProps: { editBusy: false } }
  )
  const [firstUser, assistant, lastUser] = result.current.state.messages
  expect(result.current.actions.canEditMessage!(firstUser)).toBe(true)
  expect(result.current.actions.canEditMessage!(lastUser)).toBe(true)
  expect(result.current.actions.canEditMessage!(assistant)).toBe(false)
  rerender({ editBusy: true })
  expect(result.current.actions.canEditMessage!(firstUser)).toBe(false)
})

it.each(['answered', 'closed'] as const)('blocks edit while a form is unanswered and restores it after %s', (state) => {
  const pending = [
    { id: 'user-a', role: 'user', parts: [{ type: 'text', text: '口径用哪一种？' }] },
    {
      id: 'assistant-a',
      role: 'assistant',
      parts: [{ type: 'data-form', data: { state: 'form-requested', spec: 'root = Input("caliber", "口径")' } }]
    }
  ] as CherryUIMessage[]
  const answered = [
    pending[0],
    {
      id: 'assistant-a',
      role: 'assistant',
      parts: [
        {
          type: 'data-form',
          data: { state, spec: 'root = Input("caliber", "口径")', values: { caliber: '口径用含税' } }
        }
      ]
    }
  ] as CherryUIMessage[]
  const { result, rerender } = renderHook(
    ({ messages }) =>
      useAgentMessageListProviderValue({
        topic: { id: 'agent-session:source', assistantId: 'agent-1', name: 'Source', messages: [] } as unknown as Topic,
        messages,
        partsByMessageId: {},
        isLoading: false,
        messageNavigation: 'anchor',
        startEditing: async () => {},
        editBusy: false
      }),
    { initialProps: { messages: pending } }
  )
  const user = result.current.state.messages[0]
  expect(result.current.actions.canEditMessage!(user)).toBe(false)
  rerender({ messages: answered })
  expect(result.current.actions.canEditMessage!(result.current.state.messages[0])).toBe(true)
})
