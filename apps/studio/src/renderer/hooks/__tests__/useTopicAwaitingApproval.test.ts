import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { TopicStreamStatus, PendingToolApproval } from '@shared/ai/transport'

const mockEntry = vi.fn<
  () =>
    | {
        status: TopicStreamStatus | undefined
        lastCompletedAt?: number
        activeExecutions: []
        awaitingApprovalAnchors: []
        pendingApprovals?: PendingToolApproval[]
      }
    | undefined
>()

let lastSeenCompletion: number | null = null
const setLastSeenCompletion = vi.fn((next: number | null) => {
  lastSeenCompletion = next
})

// Mock at the cache layer rather than at useTopicStreamStatus — intra-module
// vi.mock can't intercept calls between functions in the same source file.
// The main-owned status entry is observed read-only (useSharedCacheValue);
// the window-owned lastSeenCompletion marker keeps the writable hook.
vi.mock('@renderer/data/hooks/useCache', () => ({
  useSharedCache: (key: string) => {
    if (key.startsWith('topic.stream.last_seen_completion.')) {
      return [lastSeenCompletion, setLastSeenCompletion]
    }
    return [mockEntry()]
  },
  useSharedCacheValue: () => mockEntry()
}))

import {
  useTopicAwaitingApproval,
  useTopicDbRefreshOnAwaitingApproval,
  useTopicStreamStatus
} from '../useTopicStreamStatus'

const setEntry = (status: TopicStreamStatus | undefined, lastCompletedAt?: number) => {
  mockEntry.mockReturnValue({ status, lastCompletedAt, activeExecutions: [], awaitingApprovalAnchors: [] })
}

describe('useTopicAwaitingApproval', () => {
  beforeEach(() => {
    mockEntry.mockReset()
    setLastSeenCompletion.mockClear()
    lastSeenCompletion = null
  })

  it('is true iff the cross-window shared-cache status is awaiting-approval', () => {
    setEntry('awaiting-approval')
    expect(renderHook(() => useTopicAwaitingApproval('t')).result.current).toBe(true)
  })

  it.each<TopicStreamStatus | undefined>(['pending', 'streaming', 'aborted', 'done', 'error', undefined])(
    'is false for status %s (no per-window partsMap scan / SWR dependency)',
    (status) => {
      setEntry(status)
      expect(renderHook(() => useTopicAwaitingApproval('t')).result.current).toBe(false)
    }
  )

  it('treats each stream completion as unread until that specific completion is marked seen', () => {
    setEntry('done', 1000)

    const { result, rerender } = renderHook(() => useTopicStreamStatus('t'))

    expect(result.current.isFulfilled).toBe(true)

    act(() => {
      result.current.markSeen()
    })
    rerender()

    expect(result.current.isFulfilled).toBe(false)

    setEntry('done', 2000)
    rerender()

    expect(result.current.isFulfilled).toBe(true)
  })

  it.each<TopicStreamStatus>(['pending', 'streaming'])(
    'refreshes once on %s to awaiting-approval',
    async (liveStatus) => {
      const refresh = vi.fn(async () => {})
      setEntry(liveStatus)
      const { rerender } = renderHook(() => useTopicDbRefreshOnAwaitingApproval('t', refresh))

      setEntry('awaiting-approval')
      await act(async () => {
        rerender()
        await Promise.resolve()
      })
      expect(refresh).toHaveBeenCalledTimes(1)

      await act(async () => {
        rerender()
        await Promise.resolve()
      })
      expect(refresh).toHaveBeenCalledTimes(1)
    }
  )

  it.each<TopicStreamStatus>(['done', 'error', 'aborted'])(
    'does not refresh on streaming to %s',
    async (terminalStatus) => {
      const refresh = vi.fn(async () => {})
      setEntry('streaming')
      const { rerender } = renderHook(() => useTopicDbRefreshOnAwaitingApproval('t', refresh))

      setEntry(terminalStatus)
      await act(async () => {
        rerender()
        await Promise.resolve()
      })

      expect(refresh).not.toHaveBeenCalled()
    }
  )

  it('refreshes a current native pending approval on mount and when the receiver changes at the same status', async () => {
    const refresh = vi.fn(async () => {})
    const pending = {
      executionId: 'run',
      anchorMessageId: 'message',
      messageId: 'message',
      approvalId: 'approval-1',
      toolCallId: 'call-1'
    }
    const entry = {
      status: 'awaiting-approval' as const,
      activeExecutions: [] as [],
      awaitingApprovalAnchors: [] as [],
      pendingApprovals: [pending]
    }
    mockEntry.mockReturnValue(entry)
    const { rerender } = renderHook(() => useTopicDbRefreshOnAwaitingApproval('t', refresh))
    expect(refresh).toHaveBeenCalledTimes(1)
    rerender()
    expect(refresh).toHaveBeenCalledTimes(1)
    mockEntry.mockReturnValue({
      ...entry,
      pendingApprovals: [{ ...pending, approvalId: 'approval-2', toolCallId: 'call-2' }]
    })
    await act(async () => {
      rerender()
    })
    expect(refresh).toHaveBeenCalledTimes(2)
  })

  it('does not carry a live edge across topic identities', async () => {
    const refresh = vi.fn(async () => {})
    setEntry('streaming')
    const { rerender } = renderHook(
      ({ topicId }: { topicId: string }) => useTopicDbRefreshOnAwaitingApproval(topicId, refresh),
      { initialProps: { topicId: 'topic-1' } }
    )

    setEntry('awaiting-approval')
    await act(async () => {
      rerender({ topicId: 'topic-2' })
      await Promise.resolve()
    })

    expect(refresh).not.toHaveBeenCalled()
  })
})
