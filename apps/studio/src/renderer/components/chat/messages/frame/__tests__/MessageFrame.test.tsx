import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import type { Topic } from '@renderer/types/topic'

import { MessageListProvider } from '../../MessageListProvider'
import { defaultMessageRenderConfig, type MessageListItem, type MessageListProviderValue } from '../../types'
import MessageItem, { HideMessageMenuBarContext } from '../MessageFrame'

vi.mock('../MessageContent', () => ({
  default: () => <div>body</div>
}))

vi.mock('../MessageMenuBar', () => ({
  default: () => <div data-ui="part:message-actions" />
}))

const topic = {
  id: 'topic-1',
  assistantId: 'assistant-1',
  name: 'Topic',
  lastActivityAt: '2026-01-01T00:00:00.000Z',
  createdAt: '2026-01-01T00:00:00.000Z',
  updatedAt: '2026-01-01T00:00:00.000Z',
  messages: []
} as Topic

function message(role: MessageListItem['role']): MessageListItem {
  return {
    id: `${role}-1`,
    role,
    topicId: topic.id,
    createdAt: '2026-01-01T00:00:00.000Z',
    status: 'success'
  }
}

function renderMessage(role: MessageListItem['role'], hideMenuBar: boolean) {
  const item = message(role)
  const value: MessageListProviderValue = {
    state: {
      topic,
      messages: [item],
      partsByMessageId: { [item.id]: [] },
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
  const frame = <MessageItem message={item} index={0} isLatestAssistantMessage messageParts={[]} />
  return render(
    <MessageListProvider value={value}>
      {hideMenuBar ? <HideMessageMenuBarContext value={true}>{frame}</HideMessageMenuBarContext> : frame}
    </MessageListProvider>
  )
}

describe('MessageFrame menu bar', () => {
  it('keeps the action bar on an ordinary assistant message', () => {
    renderMessage('assistant', false)
    expect(screen.getByText('body')).toBeInTheDocument()
    expect(document.querySelector('[data-ui="part:message-actions"]')).not.toBeNull()
  })

  it('omits the action bar when the compile dialog hides it', () => {
    renderMessage('assistant', true)
    expect(screen.getByText('body')).toBeInTheDocument()
    expect(document.querySelector('[data-ui="part:message-actions"]')).toBeNull()
  })

  it('omits the user bubble action bar and keeps the timestamp', () => {
    renderMessage('user', true)
    expect(screen.getByText(/\d{2}\/\d{2} \d{2}:\d{2}/)).toBeInTheDocument()
    expect(document.querySelector('[data-ui="part:message-actions"]')).toBeNull()
  })

  it('places the user avatar beside the message column', () => {
    const { container } = renderMessage('user', true)
    const avatar = container.querySelector('.message-avatar')
    const row = avatar?.parentElement

    expect(row?.className).toContain('items-end')
    expect(row?.firstElementChild?.className).toContain('max-w-[calc(100%-2.5rem)]')
  })
})
