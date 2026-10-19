import { MockUseCacheUtils } from '@test-mocks/renderer/useCache'
import { render } from '@testing-library/react'
import type { PropsWithChildren } from 'react'
import { use } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { cacheService } from '@data/CacheService'
import { buildAgentSessionTopicId } from '@renderer/utils/agentSession'

import { WorkbenchTurnStreamContext } from '../../workbenchComposerHost'
import AgentSessionMessages from '../AgentSessionMessages'

const useAgentMessageListProviderValueMock = vi.hoisted(() => vi.fn(() => ({ state: {}, actions: {}, meta: {} })))
const ipcRequestMock = vi.hoisted(() => vi.fn(() => Promise.resolve(undefined)))

vi.mock('@renderer/components/chat/messages/MessageList', () => ({
  default: () => {
    const status = use(WorkbenchTurnStreamContext)
    return <div data-testid="message-list" data-turn-stream={status ?? 'none'} />
  }
}))

vi.mock('@renderer/components/chat/messages/MessageListProvider', () => ({
  MessageListProvider: ({ children }: PropsWithChildren) => <div data-testid="message-list-provider">{children}</div>
}))

vi.mock('@renderer/data/hooks/usePreference', () => ({
  usePreference: () => ['anchor']
}))

vi.mock('@renderer/hooks/agent/useSession', () => ({
  useSession: () => ({
    session: {
      id: 'session-1',
      agentId: 'agent-1',
      name: 'Agent session',
      createdAt: '2026-01-01T00:00:00.000Z',
      updatedAt: '2026-01-01T00:00:00.000Z'
    }
  })
}))

vi.mock('../../messages/agentMessageListAdapter', () => ({
  useAgentMessageListProviderValue: useAgentMessageListProviderValueMock
}))

// The mount effect fires ipcApi.request('ai.agent.session.prewarm' / 'ai.agent.session.close_warm');
// the lease test below asserts on those calls.
vi.mock('@renderer/ipc', () => ({
  ipcApi: { request: ipcRequestMock, on: vi.fn(() => () => {}) }
}))

describe('AgentSessionMessages', () => {
  beforeEach(() => {
    useAgentMessageListProviderValueMock.mockClear()
    ipcRequestMock.mockClear()
  })

  it('normalizes blank agent avatars before passing the assistant profile to the message list', () => {
    render(
      <AgentSessionMessages
        agentId="agent-1"
        sessionId="session-1"
        messages={[]}
        activeAgent={{ id: 'agent-1', name: 'Blank avatar agent', configuration: { avatar: '   ' } } as any}
        partsByMessageId={{}}
        isLoading={false}
      />
    )

    expect(document.querySelector('[data-ui="agent.harness.bar"]')).toBeNull()
    expect(document.querySelector('[data-ui="agent.harness"]')).toBeNull()
    expect(document.querySelector('[data-ui="agent.traces"]')).toBeNull()

    expect(useAgentMessageListProviderValueMock).toHaveBeenCalledWith(
      expect.objectContaining({
        assistantProfile: {
          name: 'Blank avatar agent',
          avatar: '🤖'
        }
      })
    )
  })

  it('acquires the session warm lease on mount and releases it only on unmount', () => {
    const view = render(
      <AgentSessionMessages
        agentId="agent-1"
        sessionId="session-1"
        messages={[]}
        partsByMessageId={{}}
        isLoading={false}
      />
    )

    expect(ipcRequestMock).toHaveBeenCalledWith('ai.agent.session.prewarm', { sessionId: 'session-1' })
    expect(ipcRequestMock).not.toHaveBeenCalledWith('ai.agent.session.close_warm', { sessionId: 'session-1' })

    view.unmount()
    expect(ipcRequestMock).toHaveBeenCalledWith('ai.agent.session.close_warm', { sessionId: 'session-1' })
  })

  it('anchors background status to the latest assistant with content instead of an empty pending placeholder', () => {
    const settledAssistant = {
      id: 'assistant-settled',
      role: 'assistant',
      parts: [{ type: 'text', text: 'Main answer' }]
    }
    const pendingAssistant = {
      id: 'assistant-pending',
      role: 'assistant',
      parts: []
    }

    render(
      <AgentSessionMessages
        agentId="agent-1"
        sessionId="session-1"
        messages={[settledAssistant, { id: 'user-follow-up', role: 'user', parts: [] }, pendingAssistant] as any}
        partsByMessageId={{
          'assistant-settled': [{ type: 'text', text: 'Main answer' }] as any
        }}
        isLoading={false}
      />
    )

    expect(useAgentMessageListProviderValueMock).toHaveBeenCalledWith(
      expect.objectContaining({
        messageTail: expect.objectContaining({
          messageId: 'assistant-settled'
        })
      })
    )
  })

  it('publishes the session stream so the latest bot pose can follow it', () => {
    const key = `topic.stream.statuses.${buildAgentSessionTopicId('session-1')}` as const
    const entry = {
      status: 'error' as const,
      activeExecutions: [],
      awaitingApprovalAnchors: []
    }
    cacheService.setShared(key, entry)
    MockUseCacheUtils.setSharedCacheValue(key, entry)

    const view = render(
      <AgentSessionMessages
        agentId="agent-1"
        sessionId="session-1"
        messages={[]}
        partsByMessageId={{}}
        isLoading={false}
      />
    )

    expect(view.getByTestId('message-list')).toHaveAttribute('data-turn-stream', 'error')
    view.unmount()
    cacheService.deleteShared(key)
  })
})
