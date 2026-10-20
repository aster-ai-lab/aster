import { MockCacheUtils } from '@test-mocks/renderer/CacheService'
import { MockUseDataApiUtils } from '@test-mocks/renderer/useDataApi'
import { act, render, renderHook } from '@testing-library/react'
import { Activity } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { dataApiService } from '@data/DataApiService'
import {
  readAskUserQuestionDraftCache,
  writeAskUserQuestionDraftCache
} from '@renderer/components/composer/variants/askUserQuestionDraftCache'
import type * as AgentSessionPartsModule from '@renderer/hooks/useAgentSessionParts'
import type { TopicStreamStatus, PendingToolApproval } from '@shared/ai/transport'
import type { AgentSessionMessageEntity } from '@shared/data/types/agent'
import type { CherryMessagePart, CherryUIMessage } from '@shared/data/types/message'
import { aiErrorCodes } from '@shared/ipc/errors/ai'
import { IpcError } from '@shared/ipc/errors/IpcError'

import { taskPlanMode } from '../taskConversation/taskPlanMode'

const mocks = vi.hoisted(() => ({
  refresh: vi.fn(),
  seedReservedMessages: vi.fn(),
  replaceMessageTail: vi.fn(),
  deleteSessionMessage: vi.fn(),
  useAgentSessionParts: vi.fn(),
  useChatWithHistory: vi.fn(),
  useExecutionOverlay: vi.fn(),
  disposeOverlay: vi.fn(),
  resetOverlay: vi.fn(),
  useTopicOverlayHandoffOnTerminal: vi.fn(),
  sendTurn: vi.fn(),
  editTarget: vi.fn(),
  editResend: vi.fn(),
  controllerOptions: vi.fn(),
  toastError: vi.fn(),
  chatStop: vi.fn(),
  chatSetMessages: vi.fn(),
  respondToolApproval: vi.fn(),
  invalidateMessages: vi.fn(),
  toastWarning: vi.fn(),
  scrollLatest: vi.fn(),
  worktrees: vi.fn(),
  topicStatus: {
    value: { isPending: false } as {
      isPending: boolean
      isTurnActive?: boolean
      status?: TopicStreamStatus
      pendingApprovals?: PendingToolApproval[]
    }
  }
}))

// respondToolApproval now goes through ipcApi.request('ai.tool.respond_approval', …).
vi.mock('@renderer/ipc', () => ({
  ipcApi: {
    request: (route: string, input: unknown) =>
      route === 'ai.tool.respond_approval'
        ? mocks.respondToolApproval(input)
        : route === 'ai.agent.session.edit_target'
          ? mocks.editTarget(input)
          : route === 'ai.agent.session.edit_resend'
            ? mocks.editResend(input)
            : Promise.resolve(undefined),
    on: () => () => {}
  }
}))

vi.mock('@renderer/pages/settings/aligned/worktreeStore', () => ({
  requestWorktrees: (input: unknown) => mocks.worktrees(input)
}))

vi.mock('@renderer/hooks/useAgentSessionParts', () => ({
  useAgentSessionParts: mocks.useAgentSessionParts
}))

vi.mock('@renderer/hooks/useChatWithHistory', () => ({
  useChatWithHistory: mocks.useChatWithHistory
}))

vi.mock('@renderer/hooks/useExecutionOverlay', () => ({
  useExecutionOverlay: mocks.useExecutionOverlay
}))

vi.mock('@renderer/hooks/useConversationTurnController', () => ({
  useConversationTurnController: (options: unknown) => {
    mocks.controllerOptions(options)
    return { send: mocks.sendTurn }
  }
}))

vi.mock('@renderer/hooks/useTopicStreamStatus', () => ({
  useTopicStreamStatus: () => mocks.topicStatus.value,
  useTopicOverlayHandoffOnTerminal: mocks.useTopicOverlayHandoffOnTerminal
}))

vi.mock('@renderer/services/messageUiStateCache', () => ({
  invalidateCachedMessageUiStates: mocks.invalidateMessages
}))

vi.mock('../messages/agentMessageListAdapter', () => ({
  scrollAgentMessageListToLatest: mocks.scrollLatest
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

import { getAgentTurnParts, useAgentChatRuntimeState } from '../useAgentChatRuntimeState'

vi.mock('@renderer/services/toast', () => ({ toast: { error: mocks.toastError, warning: mocks.toastWarning } }))

// <Activity> harness: tab switches hide/show the session UI without unmounting
// it, so hooks keep their state but effects are destroyed and re-created.
let latestRuntime: ReturnType<typeof useAgentChatRuntimeState> | null = null

function currentRuntime() {
  if (!latestRuntime) throw new Error('RuntimeStateHost has not rendered yet')
  return latestRuntime
}

function RuntimeStateHost({ sessionId }: { sessionId: string }) {
  latestRuntime = useAgentChatRuntimeState({
    sessionId,
    sessionMessagesEnabled: true,
    reservedMessages: []
  })
  return null
}

function ActivityHarness({ sessionId, mode }: { sessionId: string; mode: 'visible' | 'hidden' }) {
  return (
    <Activity mode={mode}>
      <RuntimeStateHost sessionId={sessionId} />
    </Activity>
  )
}

const assistantMessage = {
  id: 'assistant-1',
  role: 'assistant',
  parts: [],
  metadata: { status: 'pending' }
} as CherryUIMessage
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
const askUserQuestionUpdatedInput = {
  ...askUserQuestionInput,
  answers: { 'Choose logger': 'Winston' }
}

function makeAskUserQuestionPart(overrides: Partial<Record<string, unknown>> = {}): CherryMessagePart {
  return {
    type: 'dynamic-tool',
    toolName: 'AskUserQuestion',
    toolCallId: 'call-ask',
    state: 'approval-requested',
    input: askUserQuestionInput,
    approval: { id: 'approval-ask' },
    ...overrides
  } as unknown as CherryMessagePart
}

const originalSessionGet = vi.mocked(dataApiService.get).getMockImplementation()

function mockSessionDirectories(rows: { id: string; path: string }[]) {
  vi.mocked(dataApiService.get).mockImplementation(((path: string) => {
    if (path === '/agent-sessions') {
      return Promise.resolve({
        items: rows.map((row) => ({ id: row.id, workspace: { type: 'user', path: row.path } }))
      })
    }
    return originalSessionGet?.(path as never) ?? Promise.resolve({ id: 'mock_id' })
  }) as typeof dataApiService.get)
}

function makeAskUserQuestionApproval(part = makeAskUserQuestionPart()) {
  return {
    match: {
      part,
      state: 'approval-requested',
      toolCallId: 'call-ask',
      messageId: 'assistant-1',
      approvalId: 'approval-ask',
      input: askUserQuestionInput
    },
    approved: true,
    updatedInput: askUserQuestionUpdatedInput
  }
}

describe('useAgentChatRuntimeState', () => {
  it('restores a planned turn from its submitted parts without guessing from text or the current switch', () => {
    const parts = getAgentTurnParts({
      text: 'Update note.txt',
      options: {
        body: {
          agentId: 'agent',
          sessionId: 'session',
          userMessageParts: [{ type: 'text', text: 'Update note.txt' }],
          turnContext: { planFirst: true, directives: [], research: false }
        }
      }
    })
    const assistant: CherryUIMessage = {
      id: 'plan',
      role: 'assistant',
      parts: [{ type: 'text', text: 'Read, then update note.txt' }],
      metadata: { status: 'success' }
    }
    expect(taskPlanMode([{ id: 'user', role: 'user', parts }, assistant]).readyMessageId).toBe('plan')
    const executionParts = getAgentTurnParts({
      text: '',
      options: {
        body: {
          agentId: 'agent',
          sessionId: 'session',
          userMessageParts: parts,
          turnContext: { planFirst: false, directives: [], research: false }
        }
      }
    })
    expect(
      taskPlanMode([{ id: 'user', role: 'user', parts: executionParts }, assistant]).readyMessageId
    ).toBeUndefined()
  })
  it('replaces the edited tail before streaming even when history refresh still returns cached rows', async () => {
    MockUseDataApiUtils.resetMocks()
    const { useAgentSessionParts } = await vi.importActual<typeof AgentSessionPartsModule>(
      '@renderer/hooks/useAgentSessionParts'
    )
    mocks.useAgentSessionParts.mockImplementation(useAgentSessionParts)
    const draft = { messageId: 'edited-user', version: 'version-1', parts: [{ type: 'text', text: 'Original' }] }
    const history = ['earlier-user', 'earlier-assistant', draft.messageId, 'old-answer', 'later-user', 'later-answer']
    MockUseDataApiUtils.seedInfiniteQuery(
      '/agent-sessions/:sessionId/messages',
      [
        {
          items: history.toReversed().map(
            (id): AgentSessionMessageEntity => ({
              id,
              sessionId: 'session-1',
              role: id.endsWith('user') ? 'user' : 'assistant',
              data: { parts: [{ type: 'text', text: id }] },
              status: 'success',
              modelId: null,
              messageSnapshot: null,
              stats: null,
              searchableText: id,
              runtimeResumeToken: null,
              createdAt: '2026-01-01T00:00:00.000Z',
              updatedAt: '2026-01-01T00:00:00.000Z'
            })
          )
        }
      ],
      { params: { sessionId: 'session-1' }, query: { deferToolOutputs: true }, limit: 50 }
    )
    const reserved: CherryUIMessage[] = [
      {
        id: 'replacement-user',
        role: 'user',
        parts: [{ type: 'text', text: 'Replacement' }],
        metadata: { createdAt: '2026-01-02T00:00:00.000Z' }
      },
      {
        id: 'replacement-answer',
        role: 'assistant',
        parts: [],
        metadata: { status: 'pending', createdAt: '2026-01-02T00:00:01.000Z' }
      }
    ]
    mocks.editTarget.mockResolvedValue(draft)
    mocks.editResend.mockResolvedValue({ mode: 'started', reservedMessages: reserved })
    mocks.sendTurn.mockImplementation(async (input) => {
      const { openStream, buildStreamRequest, ensureConversation, historyAdapter } =
        mocks.controllerOptions.mock.lastCall![0]
      const ack = await openStream(buildStreamRequest(input, ensureConversation()), input)
      await historyAdapter.seedReservedMessages(ack.reservedMessages)
      return true
    })
    const { result, rerender } = renderHook(() =>
      useAgentChatRuntimeState({ sessionId: 'session-1', sessionMessagesEnabled: true, reservedMessages: [] })
    )
    await act(() => result.current.startEditing(draft.messageId))
    await act(() => result.current.resendEditedMessage({ text: 'Replacement' }))
    expect(result.current.uiMessages.map((message) => message.id)).toEqual([
      'earlier-user',
      'earlier-assistant',
      'replacement-user',
      'replacement-answer'
    ])
    mocks.useExecutionOverlay.mockReturnValue({
      ...mocks.useExecutionOverlay(),
      overlay: { 'replacement-answer': [{ type: 'text', text: 'New response in progress' }] },
      liveAssistants: [{ ...reserved[1], parts: [{ type: 'text', text: 'New response in progress' }] }]
    })
    rerender()
    expect(result.current.uiMessages.map((message) => message.id)).toEqual([
      'earlier-user',
      'earlier-assistant',
      'replacement-user',
      'replacement-answer'
    ])
    expect(result.current.partsByMessageId['replacement-answer']).toEqual([
      { type: 'text', text: 'New response in progress' }
    ])
  })

  it('shows the edit reason when resending from an unsupported checkpoint fails', async () => {
    const draft = {
      messageId: 'user-1',
      version: 'version-1',
      parts: [{ type: 'text' as const, text: 'Original question' }]
    }
    mocks.editTarget.mockResolvedValue(draft)
    mocks.editResend.mockRejectedValue(
      new IpcError(aiErrorCodes.AI_AGENT_SESSION_EDIT_FAILED, 'checkpoint_unsupported', {
        reason: 'checkpoint_unsupported'
      })
    )
    mocks.sendTurn.mockImplementation(async (input) => {
      const { openStream, buildStreamRequest, ensureConversation } = mocks.controllerOptions.mock.lastCall![0]
      await openStream(buildStreamRequest(input, ensureConversation()), input)
      return true
    })

    const { result } = renderHook(() =>
      useAgentChatRuntimeState({ sessionId: 'session-1', sessionMessagesEnabled: true, reservedMessages: [] })
    )
    await act(() => result.current.startEditing(draft.messageId))
    await act(() => result.current.resendEditedMessage({ text: 'Replacement' }))

    expect(mocks.toastError).toHaveBeenCalledExactlyOnceWith('agent.edit_resend.error.checkpoint_unsupported')
    expect(mocks.toastError).not.toHaveBeenCalledWith('agent_session_fork.unsupported_checkpoint')
  })

  it('retains the edited draft after a failed resend and clears it only on acceptance or cancel', async () => {
    const draft = {
      messageId: 'edited-user',
      version: 'version-1',
      parts: [{ type: 'text' as const, text: 'Original question' }]
    }
    const history: CherryUIMessage[] = [
      { id: 'earlier-user', role: 'user', parts: [{ type: 'text', text: 'Earlier question' }] },
      { id: 'earlier-assistant', role: 'assistant', parts: [{ type: 'text', text: 'Earlier answer' }] },
      { id: draft.messageId, role: 'user', parts: draft.parts },
      { ...assistantMessage, parts: [{ type: 'text', text: 'Old answer' }] },
      { id: 'later-user', role: 'user', parts: [{ type: 'text', text: 'Later question' }] },
      { id: 'later-assistant', role: 'assistant', parts: [{ type: 'text', text: 'Later answer' }] }
    ]
    mocks.useAgentSessionParts.mockReturnValue({ ...mocks.useAgentSessionParts(), messages: history })
    mocks.editTarget.mockResolvedValue(draft)
    mocks.editResend.mockResolvedValue({ mode: 'started', reservedMessages: [] })
    mocks.sendTurn.mockImplementation(async (input) => {
      const { openStream, buildStreamRequest, ensureConversation } = mocks.controllerOptions.mock.lastCall![0]
      await openStream(buildStreamRequest(input, ensureConversation()), input)
      return true
    })
    const { result } = renderHook(() =>
      useAgentChatRuntimeState({ sessionId: 'session-1', sessionMessagesEnabled: true, reservedMessages: [] })
    )
    await act(() => result.current.startEditing(draft.messageId))
    expect(result.current.editing).toMatchObject(draft)
    const sending = Promise.withResolvers<unknown>()
    mocks.editResend.mockReturnValueOnce(sending.promise)
    let resend: Promise<boolean>
    act(() => {
      resend = result.current.resendEditedMessage({ text: 'Replacement' })
    })
    expect(result.current.uiMessages.map((item) => item.id)).toEqual([
      'earlier-user',
      'earlier-assistant',
      draft.messageId
    ])
    expect(result.current.partsByMessageId[draft.messageId]).toEqual([{ type: 'text', text: 'Replacement' }])
    await act(async () => {
      sending.reject(new Error('Rejected'))
      await resend
    })
    expect(result.current.uiMessages).toEqual(history)
    expect(result.current.editing).toMatchObject(draft)
    expect(mocks.toastError).toHaveBeenCalledWith('Rejected')
    await act(() => result.current.resendEditedMessage({ text: 'Replacement' }))
    expect(mocks.editResend).toHaveBeenLastCalledWith(
      expect.objectContaining({ target: { messageId: draft.messageId, version: draft.version } })
    )
    expect(result.current.editing).toBeUndefined()
    await act(() => result.current.startEditing(draft.messageId))
    act(() => result.current.cancelEditing())
    expect(result.current.editing).toBeUndefined()
  })

  it('keeps the conversation when restoring an exclusive worktree fails', async () => {
    const history: CherryUIMessage[] = [
      { id: 'user-1', role: 'user', parts: [{ type: 'text', text: 'Question' }] },
      {
        id: 'assistant-edit',
        role: 'assistant',
        parts: [
          {
            type: 'dynamic-tool',
            toolName: 'write_file',
            toolCallId: 'write-1',
            state: 'output-available',
            input: { file_path: 'src/a.ts' }
          } as CherryMessagePart
        ]
      }
    ]
    mocks.useAgentSessionParts.mockReturnValue({ ...mocks.useAgentSessionParts(), messages: history })
    mocks.editTarget.mockResolvedValue({
      messageId: 'user-1',
      version: 'version-1',
      parts: [{ type: 'text', text: 'Question' }]
    })
    mockSessionDirectories([{ id: 'session-1', path: '/repo-feature' }])
    mocks.worktrees.mockImplementation(async (input: { action?: string }) => {
      if (input?.action === 'list') {
        return {
          items: [
            { path: '/repo', primary: true },
            { path: '/repo-feature', primary: false }
          ]
        }
      }
      return { items: [], error: 'restore failed' }
    })
    const { result } = renderHook(() =>
      useAgentChatRuntimeState({ sessionId: 'session-1', sessionMessagesEnabled: true, reservedMessages: [] })
    )
    await act(() => result.current.startEditing('user-1'))
    expect(result.current.editing).toMatchObject({ restoreFiles: true })
    await act(() => result.current.resendEditedMessage({ text: 'Replacement' }))
    expect(result.current.uiMessages.map((message) => message.id)).toEqual(['user-1', 'assistant-edit'])
    expect(mocks.editResend).not.toHaveBeenCalled()
    expect(mocks.toastError).toHaveBeenCalledWith('workbench.message.edit_files_failed')
    expect(mocks.worktrees).toHaveBeenCalledWith(
      expect.objectContaining({
        action: 'restore',
        cwd: '/repo-feature',
        paths: ['src/a.ts']
      })
    )
  })

  it.each([
    ['primary checkout', '/repo', []],
    ['shared path', '/repo-feature', [{ id: 'session-2', path: '/repo-feature/nested' }]],
    ['parent path', '/repo/child', [{ id: 'session-2', path: '/repo' }]]
  ] as const)('rewinds without restoring files on a %s', async (_label, sessionPath, others) => {
    const history: CherryUIMessage[] = [
      { id: 'user-1', role: 'user', parts: [{ type: 'text', text: 'Question' }] },
      { id: 'assistant-edit', role: 'assistant', parts: [{ type: 'text', text: 'Answer' }] }
    ]
    mocks.useAgentSessionParts.mockReturnValue({ ...mocks.useAgentSessionParts(), messages: history })
    mocks.editTarget.mockResolvedValue({
      messageId: 'user-1',
      version: 'version-1',
      parts: [{ type: 'text', text: 'Question' }]
    })
    mocks.editResend.mockResolvedValue({ mode: 'started', reservedMessages: [] })
    mocks.sendTurn.mockImplementation(async (input) => {
      const { openStream, buildStreamRequest, ensureConversation } = mocks.controllerOptions.mock.lastCall![0]
      await openStream(buildStreamRequest(input, ensureConversation()), input)
      return true
    })
    mockSessionDirectories([{ id: 'session-1', path: sessionPath }, ...others])
    mocks.worktrees.mockResolvedValue({
      items: [
        { path: '/repo', primary: true },
        { path: '/repo-feature', primary: false },
        { path: '/repo/child', primary: false }
      ]
    })
    const { result } = renderHook(() =>
      useAgentChatRuntimeState({ sessionId: 'session-1', sessionMessagesEnabled: true, reservedMessages: [] })
    )
    await act(() => result.current.startEditing('user-1'))
    expect(result.current.editing).toMatchObject({ restoreFiles: false })
    await act(() => result.current.resendEditedMessage({ text: 'Replacement' }))
    expect(mocks.editResend).toHaveBeenCalledOnce()
    expect(mocks.worktrees).toHaveBeenCalledWith(expect.objectContaining({ action: 'list', cwd: sessionPath }))
    expect(mocks.worktrees).not.toHaveBeenCalledWith(expect.objectContaining({ action: 'restore' }))
  })

  beforeEach(() => {
    vi.clearAllMocks()
    mocks.topicStatus.value = { isPending: false }
    if (originalSessionGet) vi.mocked(dataApiService.get).mockImplementation(originalSessionGet)
    mocks.worktrees.mockReset()
    mocks.worktrees.mockResolvedValue(undefined)
    MockCacheUtils.resetMocks()
    mocks.respondToolApproval.mockResolvedValue({ ok: true })
    mocks.refresh.mockResolvedValue([assistantMessage])
    mocks.seedReservedMessages.mockResolvedValue(undefined)
    mocks.deleteSessionMessage.mockResolvedValue(undefined)
    mocks.chatStop.mockResolvedValue(undefined)
    mocks.sendTurn.mockReset()
    mocks.sendTurn.mockResolvedValue(true)
    mocks.useAgentSessionParts.mockReturnValue({
      messages: [assistantMessage],
      persistedPartsByMessageId: {},
      isLoading: false,
      hasOlder: false,
      loadOlder: vi.fn(),
      refresh: mocks.refresh,
      seedReservedMessages: mocks.seedReservedMessages,
      replaceMessageTail: mocks.replaceMessageTail,
      deleteMessage: mocks.deleteSessionMessage
    })
    mocks.useChatWithHistory.mockReturnValue({
      activeExecutions: [{ executionId: 'provider::model', anchorMessageId: 'assistant-1' }],
      sendMessage: vi.fn(),
      stop: mocks.chatStop,
      setMessages: mocks.chatSetMessages,
      status: 'ready',
      error: undefined,
      chat: {}
    })
    mocks.useExecutionOverlay.mockReturnValue({
      overlay: {
        'assistant-1': [
          {
            type: 'dynamic-tool',
            toolCallId: 'tool-1',
            toolName: 'Agent',
            state: 'input-available'
          }
        ]
      },
      liveAssistants: [],
      disposeOverlay: mocks.disposeOverlay,
      reset: mocks.resetOverlay
    })

    Object.defineProperty(window, 'api', {
      configurable: true,
      value: {}
    })
    Object.defineProperty(window, 'toast', {
      configurable: true,
      value: {
        warning: mocks.toastWarning
      }
    })
  })

  it('reports a blocked stream open as not sent', async () => {
    mocks.sendTurn.mockResolvedValueOnce(false)
    const { result } = renderHook(() =>
      useAgentChatRuntimeState({
        sessionId: 'session-1',
        sessionMessagesEnabled: true,
        reservedMessages: []
      })
    )

    let sent: boolean | undefined
    await act(async () => {
      sent = await result.current.sendMessage({ text: 'keep this draft' })
    })

    expect(sent).toBe(false)
    expect(mocks.scrollLatest).not.toHaveBeenCalled()
  })

  it('brings the latest task turn into view after a send', async () => {
    mocks.sendTurn.mockResolvedValueOnce(true)
    const { result } = renderHook(() =>
      useAgentChatRuntimeState({
        sessionId: 'session-1',
        sessionMessagesEnabled: true,
        reservedMessages: []
      })
    )

    await act(async () => {
      await result.current.sendMessage({ text: '起草一份纪要' })
    })

    expect(mocks.scrollLatest).toHaveBeenCalledTimes(1)
    expect(mocks.scrollLatest).toHaveBeenCalledWith('agent-session:session-1')
  })

  it('invalidates disclosure state after deleting a session message', async () => {
    const { result } = renderHook(() =>
      useAgentChatRuntimeState({
        sessionId: 'session-1',
        sessionMessagesEnabled: true,
        reservedMessages: []
      })
    )

    await act(async () => {
      await result.current.deleteMessage('assistant-1')
    })

    expect(mocks.deleteSessionMessage).toHaveBeenCalledWith('assistant-1')
    expect(mocks.invalidateMessages).toHaveBeenCalledWith(['assistant-1'])
  })

  it('merges live assistant metadata into displayed session messages', () => {
    mocks.useExecutionOverlay.mockReturnValue({
      overlay: {},
      liveAssistants: [
        {
          ...assistantMessage,
          metadata: {
            ...assistantMessage.metadata,
            totalTokens: 256
          }
        } as CherryUIMessage
      ],
      disposeOverlay: mocks.disposeOverlay,
      reset: mocks.resetOverlay
    })

    const { result } = renderHook(() =>
      useAgentChatRuntimeState({
        sessionId: 'session-1',
        sessionMessagesEnabled: true,
        reservedMessages: []
      })
    )

    expect(result.current.uiMessages[0]?.metadata?.totalTokens).toBe(256)
  })

  it('keeps the history contract and composer callback stable across stream snapshots', () => {
    mocks.useExecutionOverlay.mockReturnValue({
      overlay: { 'assistant-1': [{ type: 'text', text: 'a' }] },
      liveAssistants: [{ ...assistantMessage, parts: [{ type: 'text', text: 'a' }] } as CherryUIMessage],
      disposeOverlay: mocks.disposeOverlay,
      reset: mocks.resetOverlay
    })
    const { result, rerender } = renderHook(() =>
      useAgentChatRuntimeState({
        sessionId: 'session-1',
        sessionMessagesEnabled: true,
        reservedMessages: []
      })
    )
    const streamingLayers = result.current.streamingLayers
    const sendMessage = result.current.sendMessage

    mocks.useExecutionOverlay.mockReturnValue({
      overlay: { 'assistant-1': [{ type: 'text', text: 'ab' }] },
      liveAssistants: [{ ...assistantMessage, parts: [{ type: 'text', text: 'ab' }] } as CherryUIMessage],
      disposeOverlay: mocks.disposeOverlay,
      reset: mocks.resetOverlay
    })
    rerender()

    expect(result.current.streamingLayers).toBe(streamingLayers)
    expect(result.current.streamingLayers.liveMessageIds).toEqual(['assistant-1'])
    expect(result.current.sendMessage).toBe(sendMessage)
  })

  it('sends exact current receiver identity and withdraws stale decisions while Stop remains scoped', async () => {
    const binding = {
      executionId: 'run',
      anchorMessageId: 'main',
      messageId: 'assistant-1',
      approvalId: 'approval-ask',
      toolCallId: 'call-ask'
    }
    mocks.topicStatus.value = {
      isPending: false,
      isTurnActive: true,
      status: 'awaiting-approval',
      pendingApprovals: [binding]
    }
    const { result, rerender } = renderHook(() =>
      useAgentChatRuntimeState({ sessionId: 'session-1', sessionMessagesEnabled: true, reservedMessages: [] })
    )
    expect(result.current.editBusy).toBe(true)
    await act(async () =>
      result.current.respondToolApproval({ ...makeAskUserQuestionApproval(), updatedInput: undefined })
    )
    expect(mocks.respondToolApproval).toHaveBeenCalledWith(
      expect.objectContaining({ ...binding, topicId: 'agent-session:session-1', approved: true })
    )
    await act(async () => result.current.composerContext.stop?.())
    expect(mocks.chatStop).toHaveBeenCalledOnce()
    mocks.topicStatus.value = { ...mocks.topicStatus.value, pendingApprovals: [] }
    rerender()
    await expect(
      result.current.respondToolApproval({ ...makeAskUserQuestionApproval(), updatedInput: undefined })
    ).rejects.toThrow('no longer available')
    expect(mocks.respondToolApproval).toHaveBeenCalledOnce()
    mocks.topicStatus.value = { isPending: false, isTurnActive: false, status: 'done', pendingApprovals: [] }
    rerender()
    expect(result.current.composerContext.stop).toBeUndefined()
  })

  it('stores AskUserQuestion submitted input as a temporary tool input', async () => {
    const part = makeAskUserQuestionPart()
    const { result } = renderHook(() =>
      useAgentChatRuntimeState({
        sessionId: 'session-1',
        sessionMessagesEnabled: true,
        reservedMessages: []
      })
    )

    await act(async () => {
      await result.current.respondToolApproval(makeAskUserQuestionApproval(part))
    })

    expect(result.current.optimisticAskUserQuestionInputsByToolCallId).toEqual({
      'call-ask': askUserQuestionUpdatedInput
    })
  })

  it('removes the temporary AskUserQuestion input when approval delivery fails', async () => {
    mocks.respondToolApproval.mockRejectedValueOnce(new Error('ipc boom'))
    const part = makeAskUserQuestionPart()
    const { result } = renderHook(() =>
      useAgentChatRuntimeState({
        sessionId: 'session-1',
        sessionMessagesEnabled: true,
        reservedMessages: []
      })
    )

    await act(async () => {
      await expect(result.current.respondToolApproval(makeAskUserQuestionApproval(part))).rejects.toThrow('ipc boom')
    })

    expect(result.current.optimisticAskUserQuestionInputsByToolCallId).toEqual({})
  })

  it('preserves optimistic AskUserQuestion inputs across an <Activity> hide/show and clears them on session change', async () => {
    latestRuntime = null
    const part = makeAskUserQuestionPart()
    const view = render(<ActivityHarness mode="visible" sessionId="session-1" />)

    await act(async () => {
      await currentRuntime().respondToolApproval(makeAskUserQuestionApproval(part))
    })
    expect(currentRuntime().optimisticAskUserQuestionInputsByToolCallId).toEqual({
      'call-ask': askUserQuestionUpdatedInput
    })

    // Same session hidden→visible: effects re-run with an unchanged topic id,
    // and the submitted input must survive the tab switch.
    view.rerender(<ActivityHarness mode="hidden" sessionId="session-1" />)
    view.rerender(<ActivityHarness mode="visible" sessionId="session-1" />)
    expect(currentRuntime().optimisticAskUserQuestionInputsByToolCallId).toEqual({
      'call-ask': askUserQuestionUpdatedInput
    })

    // Actual session change: the stale input must be dropped.
    view.rerender(<ActivityHarness mode="visible" sessionId="session-2" />)
    expect(currentRuntime().optimisticAskUserQuestionInputsByToolCallId).toEqual({})
  })

  it('evicts the AskUserQuestion draft cache only when the persisted part settles the answers', async () => {
    MockCacheUtils.resetMocks()
    writeAskUserQuestionDraftCache('approval-ask', {
      selectedAnswers: { 0: ['Winston'] },
      customAnswers: {},
      currentIndex: 0
    })
    const { result, rerender } = renderHook(() =>
      useAgentChatRuntimeState({
        sessionId: 'session-1',
        sessionMessagesEnabled: true,
        reservedMessages: []
      })
    )

    await act(async () => {
      await result.current.respondToolApproval(makeAskUserQuestionApproval())
    })

    // The dispatch ack lands before the assistant row is durably persisted, so
    // the draft must still be readable for a remount in that window.
    expect(result.current.optimisticAskUserQuestionInputsByToolCallId).toEqual({
      'call-ask': askUserQuestionUpdatedInput
    })
    expect(readAskUserQuestionDraftCache('approval-ask').selectedAnswers[0]).toEqual(['Winston'])

    const liveAnswer = makeAskUserQuestionPart({ state: 'output-available', input: askUserQuestionUpdatedInput })
    mocks.useAgentSessionParts.mockReturnValue({
      ...mocks.useAgentSessionParts(),
      messages: [{ ...assistantMessage, parts: [liveAnswer] }],
      persistedPartsByMessageId: { 'assistant-1': [makeAskUserQuestionPart()] }
    })
    mocks.useExecutionOverlay.mockReturnValue({
      ...mocks.useExecutionOverlay(),
      overlay: { 'assistant-1': [liveAnswer] }
    })
    rerender()
    expect(readAskUserQuestionDraftCache('approval-ask').selectedAnswers[0]).toEqual(['Winston'])

    // Only the database update allows the draft to be retired.
    mocks.useAgentSessionParts.mockReturnValue({
      messages: [
        {
          ...assistantMessage,
          parts: [makeAskUserQuestionPart({ state: 'approval-responded', input: askUserQuestionUpdatedInput })]
        }
      ],
      persistedPartsByMessageId: {
        'assistant-1': [makeAskUserQuestionPart({ state: 'approval-responded', input: askUserQuestionUpdatedInput })]
      },
      isLoading: false,
      hasOlder: false,
      loadOlder: vi.fn(),
      refresh: mocks.refresh,
      seedReservedMessages: mocks.seedReservedMessages,
      deleteMessage: mocks.deleteSessionMessage
    })
    mocks.useExecutionOverlay.mockReturnValue({
      overlay: {},
      liveAssistants: [],
      disposeOverlay: mocks.disposeOverlay,
      reset: mocks.resetOverlay
    })
    await act(async () => {
      rerender()
    })

    expect(result.current.optimisticAskUserQuestionInputsByToolCallId).toEqual({})
    expect(readAskUserQuestionDraftCache('approval-ask')).toEqual({
      selectedAnswers: {},
      customAnswers: {},
      currentIndex: 0
    })
  })

  it('evicts the AskUserQuestion draft cache from persisted parts after a remount or session change', async () => {
    // The settlement happened in a previous mount/session, so this mount never
    // dispatched the approval and holds no optimistic ids to sweep from.
    mocks.useExecutionOverlay.mockReturnValue({
      overlay: {},
      liveAssistants: [],
      disposeOverlay: mocks.disposeOverlay,
      reset: mocks.resetOverlay
    })
    mocks.useAgentSessionParts.mockReturnValue({
      messages: [
        {
          ...assistantMessage,
          parts: [makeAskUserQuestionPart({ state: 'approval-responded', input: askUserQuestionUpdatedInput })]
        }
      ],
      persistedPartsByMessageId: {
        'assistant-1': [makeAskUserQuestionPart({ state: 'approval-responded', input: askUserQuestionUpdatedInput })]
      },
      isLoading: false,
      hasOlder: false,
      loadOlder: vi.fn(),
      refresh: mocks.refresh,
      seedReservedMessages: mocks.seedReservedMessages,
      deleteMessage: mocks.deleteSessionMessage
    })
    writeAskUserQuestionDraftCache('approval-ask', {
      selectedAnswers: { 0: ['Winston'] },
      customAnswers: {},
      currentIndex: 0
    })

    const { result } = renderHook(() =>
      useAgentChatRuntimeState({
        sessionId: 'session-1',
        sessionMessagesEnabled: true,
        reservedMessages: []
      })
    )

    expect(result.current.optimisticAskUserQuestionInputsByToolCallId).toEqual({})
    expect(readAskUserQuestionDraftCache('approval-ask')).toEqual({
      selectedAnswers: {},
      customAnswers: {},
      currentIndex: 0
    })
  })
})
