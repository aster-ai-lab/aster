import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { WorkbenchProjectLockedContext, WorkbenchTurnStreamContext } from '@renderer/pages/agents/workbenchComposerHost'
import type { TopicStreamStatus } from '@shared/ai/transport'

import MessageHeader, { HideMessageIdentityContext } from '../MessageHeader'

const providerState = vi.hoisted(() => ({
  actions: {} as {
    navigateToRoute?: (target: { path: string; query?: Record<string, string> }) => void
    selectMessage?: (messageId: string, selected: boolean) => void
  },
  selection: undefined as { isMultiSelectMode: boolean; selectedMessageIds: string[] } | undefined
}))

vi.mock('@a3s-lab/ui', () => ({
  Avatar: ({ children, className }: { children?: ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
  AvatarFallback: ({ children, className }: { children?: ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
  AvatarImage: ({ className }: { className?: string }) => <div className={className} />,
  Badge: ({ asChild, children }: { asChild?: boolean; children?: ReactNode }) =>
    asChild ? <>{children}</> : <span>{children}</span>,
  Checkbox: ({
    className,
    ...props
  }: {
    className?: string
    checked?: boolean
    onCheckedChange?: (checked: boolean) => void
    [key: string]: unknown
  }) => {
    const domProps = { ...props }
    delete domProps.checked
    delete domProps.onCheckedChange

    return <div className={className} role="checkbox" {...domProps} />
  },
  EmojiAvatar: ({ children, className }: { children?: ReactNode; className?: string }) => (
    <div className={className}>{children}</div>
  ),
  EmojiIcon: ({ emoji }: { emoji?: string }) => <span>{emoji}</span>,
  Tooltip: ({ children }: { children: ReactNode }) => <>{children}</>
}))

vi.mock('@renderer/utils/model', () => ({
  getModelLogoRef: () => undefined
}))

vi.mock('@renderer/hooks/useTheme', () => ({
  useTheme: () => ({ theme: 'light' })
}))

vi.mock('@renderer/utils/naming', () => ({
  firstLetter: (value: string) => value.slice(0, 1),
  isEmoji: () => false,
  removeLeadingEmoji: (value: string) => value
}))

vi.mock('../../MessageListProvider', () => ({
  useMessageListActions: () => providerState.actions,
  useOptionalMessageListActions: () => providerState.actions,
  useMessageListMeta: () => ({
    assistantProfile: undefined,
    userProfile: undefined
  }),
  useMessageListSelection: () => providerState.selection,
  useMessageRenderConfig: () => ({
    userName: 'User',
    messageStyle: 'plain'
  })
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: { agent?: string; session?: string; round?: number }) => {
      if (key === 'agent.session_delivery.from') return `From ${values?.agent} / ${values?.session}`
      if (key === 'agent.session_turn_origin.goal_round') return `Goal round ${values?.round}`
      return key
    }
  })
}))

const createMessage = (role: 'assistant' | 'user' = 'assistant', extra: Record<string, unknown> = {}) =>
  ({
    id: 'message-1',
    role,
    createdAt: new Date('2026-06-06T00:00:00.000Z').toISOString(),
    updatedAt: new Date('2026-06-06T00:00:00.000Z').toISOString(),
    ...extra
  }) as Parameters<typeof MessageHeader>[0]['message']

describe('MessageHeader', () => {
  beforeEach(() => {
    providerState.actions = {}
    providerState.selection = undefined
  })

  it('keeps content and footer in the same body column', () => {
    const { container } = render(
      <MessageHeader
        message={createMessage()}
        contentSlot={<div className="message-content-container">Content</div>}
        footerSlot={<div className="MessageFooter">Footer</div>}
      />
    )

    const bodyColumn = container.querySelector('.message-body-column')
    const content = container.querySelector('.message-body-content')
    const footerSlot = container.querySelector('.message-footer-slot')
    const footer = container.querySelector('.MessageFooter')

    expect(content?.closest('.message-body-column')).toBe(bodyColumn)
    expect(footer?.closest('.message-body-column')).toBe(bodyColumn)
    expect(footer?.closest('.message-footer-slot')).toBe(footerSlot)
  })

  it('omits the body column when there is no body slot', () => {
    const { container } = render(<MessageHeader message={createMessage()} />)

    expect(container.querySelector('.message-body-column')).toBeNull()
  })

  it('explains a runtime-started assistant turn with its origin', () => {
    // A goal round has no user message above it; the badge is the transcript's only explanation.
    const { getByText } = render(
      <MessageHeader message={createMessage('assistant', { turnOrigin: { kind: 'goal-round', round: 2 } })} />
    )
    expect(getByText('Goal round 2')).toBeTruthy()
  })

  it('renders no origin badge for a prompted assistant turn', () => {
    const { queryByText } = render(<MessageHeader message={createMessage('assistant')} />)
    expect(queryByText(/Goal round/)).toBeNull()
    expect(queryByText('agent.session_turn_origin.background_work')).toBeNull()
  })

  it('drops the avatar and name when identity is hidden and keeps the stream', () => {
    const { container, queryByText } = render(
      <HideMessageIdentityContext value={true}>
        <MessageHeader
          showModelIdentity
          message={createMessage('assistant', {
            messageSnapshot: {
              id: 'a1',
              name: 'ToWork Helper',
              emoji: '🤖',
              model: { id: 'auto', name: 'auto', provider: 'in' }
            }
          })}
          contentSlot={<div>stream</div>}
        />
      </HideMessageIdentityContext>
    )

    expect(queryByText('ToWork Helper')).toBeNull()
    expect(queryByText('auto')).toBeNull()
    expect(container.querySelector('[data-a3s-bot-avatar="true"]')).toBeNull()
    expect(container.querySelector('.message-body-content')?.textContent).toBe('stream')
  })

  it('uses the aster star for the workbench assistant while preserving its name', () => {
    document.documentElement.dataset.tenant = 'aster'
    document.documentElement.dataset.a3sProduct = 'AsterDream'
    try {
      const { container, getByText } = render(
        <WorkbenchProjectLockedContext value={true}>
          <MessageHeader message={createMessage('assistant')} />
        </WorkbenchProjectLockedContext>
      )
      expect(container.querySelector('[data-ui="workbench.assistant.mark"]')).toHaveAttribute('src', '/tenant-mark.png')
      expect(container.querySelector('[data-a3s-bot-avatar="true"]')).toBeNull()
      expect(getByText('AsterDream')).toBeInTheDocument()
    } finally {
      delete document.documentElement.dataset.tenant
      delete document.documentElement.dataset.a3sProduct
    }
  })

  it('uses the document expert face on a workbench assistant turn', () => {
    const { container, getByText } = render(
      <WorkbenchProjectLockedContext value={true}>
        <MessageHeader
          animateAssistant
          message={createMessage('assistant', {
            status: 'pending',
            messageSnapshot: {
              id: 'a1',
              name: 'My Assistant',
              emoji: '🤖',
              model: { id: 'gpt-4', name: 'GPT-4', provider: 'openai' }
            }
          })}
        />
      </WorkbenchProjectLockedContext>
    )

    const face = container.querySelector('[data-a3s-bot-avatar="true"]')
    expect(face).not.toBeNull()
    expect(face).toHaveAttribute('data-avatar-variant', 'default')
    expect(face).toHaveAttribute('data-bot-activity', 'working')
    expect(face).toHaveAttribute('data-animation-enabled', 'true')
    expect(face?.querySelector('.a3s-bot-avatar__glasses')).toBeNull()
    expect(getByText('workbench.conversation.agent')).toBeTruthy()
    expect(container.textContent).not.toContain('My Assistant')
  })

  it('hides the model name on a workbench assistant turn', () => {
    const { queryByText, getByText } = render(
      <WorkbenchProjectLockedContext value={true}>
        <MessageHeader
          showModelIdentity
          message={createMessage('assistant', {
            model: { id: 'deepseek-v4-flash', name: 'deepseek-v4-flash', provider: 'boyue' },
            messageSnapshot: {
              id: 'a1',
              name: 'My Assistant',
              emoji: '🤖',
              model: { id: 'deepseek-v4-flash', name: 'deepseek-v4-flash', provider: 'boyue' }
            }
          })}
        />
      </WorkbenchProjectLockedContext>
    )

    expect(getByText('workbench.conversation.agent')).toBeTruthy()
    expect(queryByText('deepseek-v4-flash')).toBeNull()
  })

  it('plays a different document-expert pose for each live turn state', () => {
    const cases: Array<{ stream: TopicStreamStatus; activity: string }> = [
      { stream: 'pending', activity: 'working' },
      { stream: 'streaming', activity: 'working' },
      { stream: 'awaiting-approval', activity: 'paused' },
      { stream: 'aborted', activity: 'paused' },
      { stream: 'error', activity: 'failed' },
      { stream: 'done', activity: 'completed' }
    ]

    for (const item of cases) {
      const { container, unmount } = render(
        <WorkbenchProjectLockedContext value={true}>
          <WorkbenchTurnStreamContext value={item.stream}>
            <MessageHeader animateAssistant message={createMessage('assistant', { status: 'success' })} />
          </WorkbenchTurnStreamContext>
        </WorkbenchProjectLockedContext>
      )
      const face = container.querySelector('[data-a3s-bot-avatar="true"]')
      expect(face).toHaveAttribute('data-bot-activity', item.activity)
      expect(face).toHaveAttribute('data-animation-enabled', 'true')
      unmount()
    }
  })

  it('keeps an older workbench row on its saved face while the topic is still streaming', () => {
    const { container } = render(
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchTurnStreamContext value="streaming">
          <MessageHeader message={createMessage('assistant', { status: 'success' })} />
        </WorkbenchTurnStreamContext>
      </WorkbenchProjectLockedContext>
    )
    const face = container.querySelector('[data-a3s-bot-avatar="true"]')
    expect(face).toHaveAttribute('data-bot-activity', 'completed')
    expect(face).toHaveAttribute('data-animation-enabled', 'false')
  })

  it('uses the saved status when the latest turn has no live stream', () => {
    const { container } = render(
      <WorkbenchProjectLockedContext value={true}>
        <MessageHeader animateAssistant message={createMessage('assistant', { status: 'error' })} />
      </WorkbenchProjectLockedContext>
    )
    const face = container.querySelector('[data-a3s-bot-avatar="true"]')
    expect(face).toHaveAttribute('data-bot-activity', 'failed')
    expect(face).toHaveAttribute('data-animation-enabled', 'true')
  })

  it('shows the snapshot assistant name without repeating the model beside it', () => {
    const { getByText, queryByText } = render(
      <MessageHeader
        message={createMessage('assistant', {
          model: { id: 'gpt-4', name: 'GPT-4', provider: 'openai' },
          messageSnapshot: {
            id: 'a1',
            name: 'My Assistant',
            emoji: '🤖',
            model: { id: 'gpt-4', name: 'GPT-4', provider: 'openai' }
          }
        })}
      />
    )
    expect(getByText('My Assistant')).toBeTruthy()
    expect(queryByText('GPT-4')).toBeNull()
  })

  it('shows the model avatar and name beside the assistant when model identity is requested', () => {
    const { getByText } = render(
      <MessageHeader
        showModelIdentity
        message={createMessage('assistant', {
          model: { id: 'gpt-4', name: 'GPT-4', provider: 'openai' },
          messageSnapshot: {
            id: 'a1',
            name: 'My Assistant',
            emoji: '🤖',
            model: { id: 'gpt-4', name: 'GPT-4', provider: 'openai' }
          }
        })}
      />
    )

    expect(getByText('My Assistant')).toBeTruthy()
    expect(getByText('G').closest('[aria-hidden="true"]')).toBeTruthy()
    expect(getByText('GPT-4')).toBeTruthy()
  })

  it('shows the snapshot agent name as primary', () => {
    const { getByText } = render(
      <MessageHeader
        message={createMessage('assistant', {
          model: { id: 'claude', name: 'Claude', provider: 'anthropic' },
          messageSnapshot: {
            id: 'ag1',
            name: 'My Agent',
            model: { id: 'claude', name: 'Claude', provider: 'anthropic' }
          }
        })}
      />
    )
    expect(getByText('My Agent')).toBeTruthy()
  })

  it('marks the real message selection checkbox for drag selection lookup', () => {
    providerState.actions = { selectMessage: vi.fn() }
    providerState.selection = { isMultiSelectMode: true, selectedMessageIds: [] }

    const { container } = render(<MessageHeader message={createMessage()} />)

    expect(container.querySelector('[data-message-select-checkbox]')).not.toBeNull()
  })

  it('shows durable sender attribution without transport status on a received message', () => {
    const sender = { agentId: 'agent-a', sessionId: 'session-a' }
    const { container, getByText } = render(
      <MessageHeader
        message={createMessage('user', {
          delivery: {
            version: 1,
            sender,
            receiver: { agentId: 'agent-b', sessionId: 'session-b' },
            senderSnapshot: { agentName: 'Agent A', sessionName: 'Research' },
            receiverSnapshot: { agentName: 'Agent B', sessionName: 'Build' },
            replyPolicy: 'none',
            turnRef: null,
            sourceMessageId: null,
            outcome: null,
            error: null,
            statusAt: '2026-06-06T00:00:01.000Z',
            status: 'accepted',
            inReplyTo: null
          }
        })}
      />
    )

    expect(getByText('From Agent A / Research')).toBeTruthy()
    expect(container.querySelector('.lucide-mouse-pointer-click')).not.toBeNull()
    expect(screen.queryByText('agent.session_delivery.status.accepted')).toBeNull()
  })

  it('opens the sending session from durable attribution', async () => {
    const user = userEvent.setup()
    const navigateToRoute = vi.fn()
    providerState.actions = { navigateToRoute }

    render(
      <MessageHeader
        message={createMessage('user', {
          delivery: {
            version: 1,
            sender: { agentId: 'agent-a', sessionId: 'session-source' },
            receiver: { agentId: 'agent-b', sessionId: 'session-current' },
            senderSnapshot: { agentName: 'Agent A', sessionName: 'Research' },
            receiverSnapshot: { agentName: 'Agent B', sessionName: 'Build' },
            replyPolicy: 'none',
            turnRef: null,
            sourceMessageId: null,
            outcome: null,
            error: null,
            statusAt: '2026-06-06T00:00:01.000Z',
            status: 'accepted',
            inReplyTo: null
          }
        })}
      />
    )

    await user.click(screen.getByRole('button', { name: /From Agent A \/ Research/ }))

    expect(navigateToRoute).toHaveBeenCalledWith({
      path: '/app/agents',
      query: { sessionId: 'session-source' }
    })
  })
})
