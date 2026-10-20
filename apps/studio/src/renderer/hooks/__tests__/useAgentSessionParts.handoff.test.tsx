import { MockCacheUtils } from '@test-mocks/renderer/CacheService'
import { MockDataApiUtils } from '@test-mocks/renderer/DataApiService'
import { act, cleanup, renderHook, waitFor } from '@testing-library/react'
import { type ReactNode, startTransition, Suspense } from 'react'
import { SWRConfig } from 'swr'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { cacheService } from '@data/CacheService'
import { dataApiService } from '@data/DataApiService'
import { createOverlayRefreshHandoff } from '@renderer/components/chat/messages/stream/useMessageStreamingLayers'
import type { AiStreamOpenResponse } from '@shared/ai/transport'
import type { AgentSessionMessageEntity } from '@shared/data/types/agent'

import { useAgentSessionParts } from '../useAgentSessionParts'
import { useConversationTurnController } from '../useConversationTurnController'
import { useTopicOverlayHandoffOnTerminal } from '../useTopicStreamStatus'

// A mocked mutate cannot reproduce SWR discarding a fetch overlapped by a reservation write.
vi.mock('@data/hooks/useDataApi', (importOriginal) => importOriginal())
vi.mock('@data/hooks/useCache', (importOriginal) => importOriginal())

type SessionId = 'session-1' | 'session-2'
type HistoryPage = { items: AgentSessionMessageEntity[] }

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: Error) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

function reservedAck(sessionId: SessionId) {
  return {
    mode: 'started',
    reservedMessages: [
      { id: `user-${sessionId}`, role: 'user', parts: [{ type: 'text', text: 'request' }] },
      { id: `assistant-${sessionId}`, role: 'assistant', parts: [], metadata: { status: 'pending' } }
    ]
  } satisfies AiStreamOpenResponse
}

function finalHistory(sessionId: SessionId): HistoryPage {
  const row: AgentSessionMessageEntity = {
    id: `assistant-${sessionId}`,
    sessionId,
    role: 'assistant',
    data: { parts: [{ type: 'text', text: `reply-${sessionId}`, state: 'done' }] },
    searchableText: `reply-${sessionId}`,
    status: 'success',
    modelId: null,
    messageSnapshot: null,
    stats: null,
    runtimeResumeToken: null,
    createdAt: '2026-10-20T00:00:00.000Z',
    updatedAt: '2026-10-20T00:00:00.100Z'
  }
  return {
    items: [
      row,
      { ...row, id: `user-${sessionId}`, role: 'user', data: { parts: [{ type: 'text', text: 'request' }] } }
    ]
  }
}

function terminal(sessionId: SessionId) {
  cacheService.setShared(`topic.stream.statuses.agent-session:${sessionId}`, {
    status: 'done',
    turnId: `assistant-${sessionId}`,
    activeExecutions: [],
    awaitingApprovalAnchors: [],
    lastCompletedAt: 1
  })
}

function harness() {
  // Isolate reservation races from the status hook's terminal-observation policy.
  for (const sessionId of ['session-1', 'session-2'] as const) {
    cacheService.setShared(`topic.stream.statuses.agent-session:${sessionId}`, {
      status: 'streaming',
      turnId: `assistant-${sessionId}`,
      activeExecutions: [],
      awaitingApprovalAnchors: []
    })
  }
  const ack = deferred<AiStreamOpenResponse>()
  const histories = { 'session-1': deferred<HistoryPage>(), 'session-2': deferred<HistoryPage>() }
  const get = vi.mocked(dataApiService.get)
  get.mockImplementation((path) => {
    if (path === '/agent-sessions/session-1/messages') return histories['session-1'].promise as never
    if (path === '/agent-sessions/session-2/messages') return histories['session-2'].promise as never
    throw new Error(`Unexpected history request: ${path}`)
  })
  const resetOverlay = vi.fn()
  const queryError = vi.fn()
  const cache = new Map()
  const pendingSelection = deferred<never>().promise
  const wrapper = ({ children }: { children: ReactNode }) => (
    <SWRConfig value={{ provider: () => cache, onError: queryError }}>
      <Suspense fallback={null}>{children}</Suspense>
    </SWRConfig>
  )
  const view = renderHook(
    ({ sessionId, shouldSuspend }: { sessionId: SessionId; shouldSuspend?: boolean }) => {
      const topicId = `agent-session:${sessionId}`
      const history = useAgentSessionParts(sessionId, { enabled: true, fetchOnMount: false })
      const turn = useConversationTurnController({
        scopeKey: topicId,
        historyAdapter: {
          seedReservedMessages: history.seedReservedMessages,
          refresh: history.refresh,
          rollback: history.refresh
        },
        ensureConversation: () => ({ topicId }),
        buildStreamRequest: (_input: string, conversation) => ({
          trigger: 'submit-message' as const,
          topicId: conversation.topicId,
          userMessageParts: []
        }),
        openStream: () => ack.promise
      })
      useTopicOverlayHandoffOnTerminal(topicId, createOverlayRefreshHandoff(history.refresh, resetOverlay))
      if (shouldSuspend) throw pendingSelection
      return { history, turn }
    },
    { initialProps: { sessionId: 'session-1' }, wrapper }
  )
  return { ...view, ack, histories, get, resetOverlay, queryError }
}

function expectReply(h: ReturnType<typeof harness>, sessionId: SessionId) {
  expect(
    h.result.current.history.messages.find((message) => message.id === `assistant-${sessionId}`)?.parts
  ).toContainEqual({ type: 'text', text: `reply-${sessionId}`, state: 'done' })
}

beforeEach(() => {
  vi.clearAllMocks()
  MockCacheUtils.resetMocks()
  MockDataApiUtils.resetMocks()
})
afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

describe('agent session reservation and terminal history handoff', () => {
  it('keeps final text when the acknowledgement precedes terminal refresh', async () => {
    const h = harness()
    await act(async () => {
      const sent = h.result.current.turn.send('request')
      h.ack.resolve(reservedAck('session-1'))
      expect(await sent).toBe(true)
    })
    expect(h.result.current.history.messages.find((message) => message.role === 'assistant')?.parts).toEqual([])
    act(() => terminal('session-1'))
    await waitFor(() => expect(h.get).toHaveBeenCalled())
    await act(async () => h.histories['session-1'].resolve(finalHistory('session-1')))
    await waitFor(() => expectReply(h, 'session-1'))
  })

  it('keeps final text when terminal refresh is in flight before the acknowledgement', async () => {
    const h = harness()
    let sent!: Promise<boolean>
    act(() => {
      sent = h.result.current.turn.send('request')
    })
    act(() => terminal('session-1'))
    await waitFor(() => expect(h.get).toHaveBeenCalled())
    await act(async () => {
      h.ack.resolve(reservedAck('session-1'))
      expect(await sent).toBe(true)
    })
    expect(h.result.current.history.messages.find((message) => message.role === 'assistant')?.parts).toEqual([])
    await act(async () => h.histories['session-1'].resolve(finalHistory('session-1')))
    await waitFor(() => expect(h.resetOverlay).toHaveBeenCalledOnce())
    await waitFor(() => expectReply(h, 'session-1'))
  })

  it('keeps final text when a duplicate acknowledgement overlaps terminal refresh', async () => {
    const h = harness()
    await act(async () => h.result.current.history.seedReservedMessages(reservedAck('session-1').reservedMessages))
    act(() => terminal('session-1'))
    await waitFor(() => expect(h.get).toHaveBeenCalled())
    await act(async () => h.result.current.history.seedReservedMessages(reservedAck('session-1').reservedMessages))
    await act(async () => h.histories['session-1'].resolve(finalHistory('session-1')))
    await waitFor(() => expectReply(h, 'session-1'))
    await act(async () => h.result.current.history.seedReservedMessages(reservedAck('session-1').reservedMessages))
    await waitFor(() => expectReply(h, 'session-1'))
    expect(h.result.current.history.messages.map((message) => message.id)).toEqual([
      'user-session-1',
      'assistant-session-1'
    ])
  })

  it('does not put an old session refresh into the newly selected session', async () => {
    const h = harness()
    await act(async () => h.result.current.history.seedReservedMessages(reservedAck('session-1').reservedMessages))
    act(() => terminal('session-1'))
    await waitFor(() => expect(h.get).toHaveBeenCalled())
    h.rerender({ sessionId: 'session-2' })
    expect(h.result.current.history.messages).toEqual([])
    await act(async () => h.result.current.history.seedReservedMessages(reservedAck('session-2').reservedMessages))
    act(() => terminal('session-2'))
    await act(async () => h.histories['session-2'].resolve(finalHistory('session-2')))
    await waitFor(() => expectReply(h, 'session-2'))
    await act(async () => h.histories['session-1'].resolve(finalHistory('session-1')))
    expectReply(h, 'session-2')
    expect(h.result.current.history.messages.every((message) => message.id.endsWith('session-2'))).toBe(true)
  })

  it('ignores a reservation callback retained from a previous session', async () => {
    const h = harness()
    const oldSeed = h.result.current.history.seedReservedMessages
    h.rerender({ sessionId: 'session-2' })
    h.get.mockClear()
    await act(async () => oldSeed(reservedAck('session-1').reservedMessages))
    expect(h.result.current.history.messages).toEqual([])
    expect(h.get).not.toHaveBeenCalled()
  })

  it('does not revalidate the new session when selection changes during the seed await', async () => {
    const h = harness()
    let seeding!: Promise<void>
    act(() => {
      seeding = h.result.current.history.seedReservedMessages(reservedAck('session-1').reservedMessages)
      h.rerender({ sessionId: 'session-2' })
    })
    // The selection itself may fetch B; A's awaited continuation must not add another B read.
    h.get.mockClear()
    await act(async () => seeding)
    expect(h.result.current.history.messages).toEqual([])
    expect(h.get.mock.calls.some(([path]) => path === '/agent-sessions/session-2/messages')).toBe(false)
  })

  it('keeps the visible session send while another session selection is suspended', async () => {
    const h = harness()
    const seedVisibleSession = h.result.current.history.seedReservedMessages
    act(() => {
      startTransition(() => h.rerender({ sessionId: 'session-2', shouldSuspend: true }))
    })
    await act(async () => seedVisibleSession(reservedAck('session-1').reservedMessages))
    expect(h.result.current.history.messages.map((message) => message.id).sort()).toEqual([
      'assistant-session-1',
      'user-session-1'
    ])
    act(() => terminal('session-1'))
    await act(async () => h.histories['session-1'].resolve(finalHistory('session-1')))
    await waitFor(() => expectReply(h, 'session-1'))
  })

  it('accepts the send before history resolves and retains it if the background refresh fails', async () => {
    const h = harness()
    await act(async () => {
      const sent = h.result.current.turn.send('request')
      h.ack.resolve(reservedAck('session-1'))
      expect(await sent).toBe(true)
    })
    expect(h.result.current.turn.phase).toBe('streaming')
    expect(h.result.current.history.messages.map((message) => message.id)).toHaveLength(2)
    await waitFor(() => expect(h.get).toHaveBeenCalled())
    const error = new Error('history temporarily unavailable')
    await act(async () => h.histories['session-1'].reject(error))
    await waitFor(() => expect(h.queryError).toHaveBeenCalledWith(error, expect.any(String), expect.any(Object)))
    expect(h.result.current.turn.phase).toBe('streaming')
    expect(h.result.current.history.messages.map((message) => message.id)).toHaveLength(2)
  })
})
