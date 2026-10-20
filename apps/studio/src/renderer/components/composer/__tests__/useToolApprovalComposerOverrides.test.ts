import { MockCacheUtils } from '@test-mocks/renderer/CacheService'
import { act, fireEvent, render, renderHook, screen, waitFor } from '@testing-library/react'
import type { ReactElement } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { cacheService } from '@data/CacheService'
import type { CherryMessagePart } from '@shared/data/types/message'

import { useToolApprovalComposerOverrides } from '../useToolApprovalComposerOverrides'
import {
  getAskUserQuestionDraftCacheKey,
  readAskUserQuestionDraftCache,
  writeAskUserQuestionDraftCache
} from '../variants/askUserQuestionDraftCache'

const askUserQuestionInput = {
  questions: [
    {
      question: 'Choose logger',
      header: 'Logger',
      options: [{ label: 'Winston' }, { label: 'Pino' }],
      multiSelect: false
    }
  ]
}

function makePermissionPart(overrides: Partial<Record<string, unknown>> = {}): CherryMessagePart {
  return {
    type: 'tool-Read',
    toolName: 'Read',
    toolCallId: 'call-read',
    state: 'approval-requested',
    input: { file_path: '/tmp/file.ts' },
    approval: { id: 'approval-read' },
    callProviderMetadata: {
      'claude-code': {
        rawInput: { file_path: '/tmp/file.ts' },
        parentToolCallId: null
      }
    },
    ...overrides
  } as unknown as CherryMessagePart
}

function makeAskUserQuestionPart(overrides: Partial<Record<string, unknown>> = {}): CherryMessagePart {
  return makePermissionPart({
    type: 'dynamic-tool',
    toolName: 'AskUserQuestion',
    toolCallId: 'call-ask',
    input: askUserQuestionInput,
    approval: { id: 'approval-ask' },
    ...overrides
  })
}

describe('useToolApprovalComposerOverrides', () => {
  beforeEach(() => MockCacheUtils.resetMocks())

  it('builds shared composer overrides and keeps AskUserQuestion higher priority', () => {
    const { result } = renderHook(() =>
      useToolApprovalComposerOverrides({
        persistedPartsByMessageId: {},
        partsByMessageId: {
          'message-1': [makePermissionPart(), makeAskUserQuestionPart()]
        },
        onRespond: vi.fn()
      })
    )

    expect(result.current.map((override) => override.id)).toEqual([
      'ask-user-question:approval-ask',
      'tool-permission:approval-read'
    ])
    expect(result.current.map((override) => override.priority)).toEqual([100, 90])
  })

  it('renders native boolean approval without an unsupported rejection-note affordance', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined)
    const part = makePermissionPart()
    const { result } = renderHook(() =>
      useToolApprovalComposerOverrides({
        persistedPartsByMessageId: { 'message-1': [part] },
        partsByMessageId: { 'message-1': [part] },
        pendingApprovals: [
          {
            executionId: 'run',
            anchorMessageId: 'message-1',
            messageId: 'message-1',
            approvalId: 'approval-read',
            toolCallId: 'call-read'
          }
        ],
        onRespond
      })
    )
    const element = result.current[0].render({}) as ReactElement
    render(element)
    expect(screen.queryByRole('button', { name: '添加说明' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: '拒绝' }))
    await waitFor(() =>
      expect(onRespond).toHaveBeenCalledWith({
        match: expect.objectContaining({ approvalId: 'approval-read' }),
        approved: false
      })
    )
  })

  it('hides an approval the route already accepted', () => {
    const { result } = renderHook(() =>
      useToolApprovalComposerOverrides({
        persistedPartsByMessageId: {},
        partsByMessageId: {
          'message-1': [makePermissionPart(), makeAskUserQuestionPart()]
        },
        dismissedApprovalIds: new Set(['approval-read']),
        onRespond: vi.fn()
      })
    )

    expect(result.current.map((override) => override.id)).toEqual(['ask-user-question:approval-ask'])
  })

  it('returns no overrides when no pending approvals exist', () => {
    const { result } = renderHook(() =>
      useToolApprovalComposerOverrides({
        persistedPartsByMessageId: {},
        partsByMessageId: {
          'message-1': [makePermissionPart({ state: 'approval-responded' })]
        },
        onRespond: vi.fn()
      })
    )

    expect(result.current).toEqual([])
  })

  it('masks a stale historical approval with the current live message', () => {
    const historicalPart = makeAskUserQuestionPart()
    const currentPart = makeAskUserQuestionPart({ state: 'approval-responded' })
    const { result } = renderHook(() =>
      useToolApprovalComposerOverrides({
        persistedPartsByMessageId: {},
        partsByMessageId: { 'message-1': [currentPart] },
        streamingLayers: {
          historyPartsByMessageId: { 'message-1': [historicalPart] },
          liveMessageIds: ['message-1']
        },
        onRespond: vi.fn()
      })
    )

    expect(result.current).toEqual([])
  })

  it('recovers only the current native approval when an older same-anchor overlay hides its persisted part', () => {
    const previous = makePermissionPart({ state: 'approval-responded' })
    const pending = makePermissionPart({ toolCallId: 'call-next', approval: { id: 'approval-next' } })
    const { result } = renderHook(() =>
      useToolApprovalComposerOverrides({
        persistedPartsByMessageId: { 'message-1': [previous, pending] },
        partsByMessageId: { 'message-1': [previous] },
        streamingLayers: {
          historyPartsByMessageId: { 'message-1': [previous, pending] },
          liveMessageIds: ['message-1']
        },
        pendingApprovals: [
          {
            executionId: 'run-current',
            anchorMessageId: 'message-1',
            messageId: 'message-1',
            approvalId: 'approval-next',
            toolCallId: 'call-next'
          }
        ],
        onRespond: vi.fn()
      })
    )

    expect(result.current.map((override) => override.id)).toEqual(['tool-permission:approval-next'])
  })

  it('does not revive a persisted pending approval without a current native receiver', () => {
    const { result } = renderHook(() =>
      useToolApprovalComposerOverrides({
        persistedPartsByMessageId: { 'message-1': [makePermissionPart()] },
        partsByMessageId: { 'message-1': [makePermissionPart()] },
        pendingApprovals: [],
        onRespond: vi.fn()
      })
    )
    expect(result.current).toEqual([])
  })

  it('does not offer legacy edited answers to a native boolean receiver and keeps settled answers settled', () => {
    const pending = makeAskUserQuestionPart()
    const binding = {
      executionId: 'run-current',
      anchorMessageId: 'main-anchor',
      messageId: 'repair-message',
      approvalId: 'approval-ask',
      toolCallId: 'call-ask'
    }
    const { result, rerender } = renderHook(
      ({ persistedPart }) =>
        useToolApprovalComposerOverrides({
          persistedPartsByMessageId: { 'repair-message': [persistedPart] },
          partsByMessageId: {
            'main-anchor': [makePermissionPart({ state: 'approval-responded' })],
            'repair-message': [pending]
          },
          pendingApprovals: [binding],
          onRespond: vi.fn()
        }),
      { initialProps: { persistedPart: pending } }
    )
    expect(result.current).toEqual([])
    rerender({ persistedPart: makeAskUserQuestionPart({ state: 'approval-responded' }) })
    expect(result.current).toEqual([])
  })

  it('does not recover a pending approval from another message or tool call', () => {
    const pending = makePermissionPart()
    const { result } = renderHook(() =>
      useToolApprovalComposerOverrides({
        persistedPartsByMessageId: { 'message-other': [pending] },
        partsByMessageId: { 'message-other': [pending] },
        pendingApprovals: [
          {
            executionId: 'run-current',
            anchorMessageId: 'message-1',
            messageId: 'message-1',
            approvalId: 'approval-read',
            toolCallId: 'call-other'
          }
        ],
        onRespond: vi.fn()
      })
    )
    expect(result.current).toEqual([])
  })

  it('keeps the queue head until refreshed parts advance to the next permission', async () => {
    let resolveResponse: (() => void) | undefined
    const onRespond = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveResponse = resolve
        })
    )
    const firstPart = makePermissionPart()
    const secondPart = makePermissionPart({
      toolCallId: 'call-write',
      approval: { id: 'approval-write' },
      type: 'tool-Write',
      toolName: 'Write'
    })
    const { result, rerender } = renderHook(
      ({ partsByMessageId }) =>
        useToolApprovalComposerOverrides({ partsByMessageId, persistedPartsByMessageId: {}, onRespond }),
      {
        initialProps: {
          partsByMessageId: { 'message-1': [firstPart, secondPart] }
        }
      }
    )

    const override = result.current.find((override) => override.id === 'tool-permission:approval-read')
    const element = override?.render({}) as ReactElement<{ onRespond: (input: any) => Promise<void> }> | undefined

    let response: Promise<void> | undefined
    act(() => {
      response = element?.props.onRespond({
        match: { approvalId: 'approval-read' }
      })
    })

    expect(result.current.map((override) => override.id)).toContain('tool-permission:approval-read')
    expect(result.current.map((override) => override.id)).not.toContain('tool-permission:approval-write')

    await act(async () => {
      resolveResponse?.()
      await response
    })

    // The IPC success alone is not a second source of truth: the same persisted parts keep A visible.
    expect(result.current.map((override) => override.id)).toContain('tool-permission:approval-read')
    expect(result.current.map((override) => override.id)).not.toContain('tool-permission:approval-write')

    rerender({
      partsByMessageId: {
        'message-1': [makePermissionPart({ state: 'approval-responded' }), secondPart]
      }
    })

    expect(result.current.map((override) => override.id)).not.toContain('tool-permission:approval-read')
    expect(result.current.map((override) => override.id)).toContain('tool-permission:approval-write')
    expect(onRespond).toHaveBeenCalledTimes(1)
  })

  it('keeps the queue head when its response fails', async () => {
    const onRespond = vi.fn().mockRejectedValue(new Error('failed'))
    const { result } = renderHook(() =>
      useToolApprovalComposerOverrides({
        persistedPartsByMessageId: {},
        partsByMessageId: {
          'message-1': [makePermissionPart()]
        },
        onRespond
      })
    )

    const override = result.current.find((override) => override.id === 'tool-permission:approval-read')
    const element = override?.render({}) as ReactElement<{ onRespond: (input: any) => Promise<void> }> | undefined

    await act(async () => {
      await expect(
        element?.props.onRespond({
          match: { approvalId: 'approval-read' }
        })
      ).rejects.toThrow('failed')
    })

    expect(result.current.map((override) => override.id)).toContain('tool-permission:approval-read')
  })
})

describe('AskUserQuestion draft settlement', () => {
  const draft = { selectedAnswers: { 0: ['Winston'] }, customAnswers: { 0: 'Keep JSON logs' }, currentIndex: 0 }

  beforeEach(() => {
    MockCacheUtils.resetMocks()
    writeAskUserQuestionDraftCache('approval-ask', draft)
    writeAskUserQuestionDraftCache('approval-other', draft)
  })

  it.each([
    { state: 'approval-responded', approved: true },
    { state: 'approval-responded', approved: false },
    { state: 'output-available', approved: true },
    { state: 'output-denied', approved: false }
  ])('clears only the matching draft for persisted $state (approved=$approved) on mount', ({ state, approved }) => {
    const settled = makeAskUserQuestionPart({
      state,
      approval: { id: 'approval-ask', approved },
      input: approved ? { ...askUserQuestionInput, answers: { 'Choose logger': 'Winston' } } : askUserQuestionInput
    })
    renderHook(() =>
      useToolApprovalComposerOverrides({
        partsByMessageId: {},
        persistedPartsByMessageId: { 'message-1': [settled] },
        onRespond: vi.fn()
      })
    )

    expect(cacheService.has(getAskUserQuestionDraftCacheKey('approval-ask'))).toBe(false)
    expect(readAskUserQuestionDraftCache('approval-other')).toEqual(draft)
  })

  it('keeps the draft through live answers and a missing card until database parts settle', () => {
    const pending = makeAskUserQuestionPart()
    const answered = makeAskUserQuestionPart({
      state: 'output-available',
      input: { ...askUserQuestionInput, answers: { 'Choose logger': 'Winston' } }
    })
    const liveParts: Record<string, CherryMessagePart[]> = { 'message-1': [answered] }
    const pendingParts: Record<string, CherryMessagePart[]> = { 'message-1': [pending] }
    const { rerender, unmount } = renderHook(
      ({ partsByMessageId, persistedPartsByMessageId }) =>
        useToolApprovalComposerOverrides({
          partsByMessageId,
          persistedPartsByMessageId,
          streamingLayers: { historyPartsByMessageId: liveParts, liveMessageIds: ['message-1'] },
          onRespond: vi.fn()
        }),
      { initialProps: { partsByMessageId: liveParts, persistedPartsByMessageId: pendingParts } }
    )
    expect(readAskUserQuestionDraftCache('approval-ask')).toEqual(draft)

    rerender({ partsByMessageId: {}, persistedPartsByMessageId: {} })
    unmount()
    expect(readAskUserQuestionDraftCache('approval-ask')).toEqual(draft)

    renderHook(() =>
      useToolApprovalComposerOverrides({
        partsByMessageId: pendingParts,
        persistedPartsByMessageId: liveParts,
        streamingLayers: { historyPartsByMessageId: pendingParts, liveMessageIds: ['message-1'] },
        onRespond: vi.fn()
      })
    )
    expect(cacheService.has(getAskUserQuestionDraftCacheKey('approval-ask'))).toBe(false)
  })

  it('does not treat a pending question or an unrelated completed tool as a settled answer', () => {
    renderHook(() =>
      useToolApprovalComposerOverrides({
        partsByMessageId: {},
        persistedPartsByMessageId: {
          'message-1': [
            makeAskUserQuestionPart(),
            makePermissionPart({ state: 'output-available', approval: { id: 'approval-other' } })
          ]
        },
        onRespond: vi.fn()
      })
    )
    expect(readAskUserQuestionDraftCache('approval-ask')).toEqual(draft)
    expect(readAskUserQuestionDraftCache('approval-other')).toEqual(draft)
  })
})
