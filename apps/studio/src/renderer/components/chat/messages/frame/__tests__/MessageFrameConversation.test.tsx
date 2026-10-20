import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import {
  WorkbenchProjectLockedContext,
  WorkbenchTurnStreamContext
} from '@renderer/components/composer/workbenchComposerHost'
import { TaskConversationMessage } from '@renderer/pages/agents/taskConversation/TaskConversationMessage'
import type { Topic } from '@renderer/types/topic'
import type { CherryMessagePart } from '@shared/data/types/message'

import { MessageListProvider } from '../../MessageListProvider'
import { defaultMessageRenderConfig, type MessageListItem, type MessageListProviderValue } from '../../types'
import MessageItem from '../MessageFrame'
import { WorkbenchAssistantMessageContext } from '../workbenchAssistantMessage'

function renderConversationMessage(role: MessageListItem['role'], latest = false) {
  const item: MessageListItem = {
    id: `${role}-1`,
    role,
    topicId: 'topic-1',
    createdAt: '2026-01-01T00:00:00.000Z',
    status: 'success'
  }
  const text = role === 'user' ? '请先规划，再执行中文输入测试。' : '已读取项目要求。'
  const parts: CherryMessagePart[] = [{ type: 'text', text }]
  const value: MessageListProviderValue = {
    state: {
      topic: {
        id: 'topic-1',
        assistantId: 'assistant-1',
        name: 'Topic',
        createdAt: item.createdAt,
        updatedAt: item.createdAt,
        lastActivityAt: item.createdAt,
        messages: []
      } as Topic,
      messages: [item],
      partsByMessageId: { [item.id]: parts },
      messageNavigation: 'none',
      estimateSize: 0,
      overscan: 0,
      loadOlderDelayMs: 0,
      loadingResetDelayMs: 0,
      renderConfig: { ...defaultMessageRenderConfig, messageStyle: 'bubble' },
      getMessageActivityState: () => ({
        isProcessing: false,
        isStreamTarget: false,
        isApprovalAnchor: false,
        isActiveTurnProcessing: false,
        isStreamLive: false
      })
    },
    actions: {},
    meta: { selectionLayer: false }
  }
  return render(
    <MessageListProvider value={value}>
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchTurnStreamContext value="streaming">
          <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
            <MessageItem message={item} messageParts={parts} index={0} hideMenuBar isLatestAssistantMessage={latest} />
          </WorkbenchAssistantMessageContext>
        </WorkbenchTurnStreamContext>
      </WorkbenchProjectLockedContext>
    </MessageListProvider>
  )
}

describe('real workbench conversation message frame', () => {
  it('renders a newly submitted Chinese user bubble without assistant state', () => {
    renderConversationMessage('user')
    expect(screen.getByText('请先规划，再执行中文输入测试。')).toBeInTheDocument()
    expect(screen.getByText('01/01 00:00')).toBeInTheDocument()
    expect(screen.queryByTestId('task-pending')).not.toBeInTheDocument()
  })

  it('retains live awaiting state for the actual latest assistant caller', () => {
    renderConversationMessage('assistant', true)
    expect(screen.getByText('已读取项目要求。')).toBeInTheDocument()
    expect(screen.getByTestId('task-pending')).toBeInTheDocument()
  })

  it('does not revive awaiting state for an older settled assistant', () => {
    renderConversationMessage('assistant')
    expect(screen.getByText('已读取项目要求。')).toBeInTheDocument()
    expect(screen.queryByTestId('task-pending')).not.toBeInTheDocument()
  })
})
