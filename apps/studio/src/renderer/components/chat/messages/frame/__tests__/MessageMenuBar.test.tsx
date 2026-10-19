import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type React from 'react'
import type { ReactNode } from 'react'
import { describe, expect, it, vi } from 'vitest'

import type { Topic } from '@renderer/types/topic'

import { MessageListProvider } from '../../MessageListProvider'
import {
  defaultMessageMenuConfig,
  defaultMessageRenderConfig,
  type MessageListItem,
  type MessageListProviderValue
} from '../../types'
import type { MessageMenuBarResolvedAction } from '../messageMenuBarActions'

vi.mock('../MessageMenuBarToolbar', () => ({
  MessageMenuBarToolbarAction: ({
    action,
    executeAction,
    menuActions
  }: {
    action: MessageMenuBarResolvedAction
    executeAction: (action: MessageMenuBarResolvedAction) => Promise<void>
    menuActions: MessageMenuBarResolvedAction[]
  }) => {
    const button = (item: MessageMenuBarResolvedAction) => (
      <button
        type="button"
        key={item.id}
        disabled={!item.availability.enabled}
        onClick={() => void executeAction(item)}>
        {item.id}
      </button>
    )
    return (
      <>
        {button(action)}
        {action.id === 'more-menu' ? menuActions.flatMap((item) => item.children).map(button) : null}
      </>
    )
  }
}))

vi.mock('@renderer/utils/style', () => ({
  classNames: (...values: unknown[]) => values.filter(Boolean).join(' ')
}))

vi.mock('@renderer/services/ExportService', () => ({
  exportService: {
    captureScrollableAsBlob: vi.fn(),
    captureScrollableAsDataUrl: vi.fn()
  },
  getMessageTitle: vi.fn(),
  messageToMarkdown: vi.fn()
}))

vi.mock('@renderer/utils/export', () => ({
  messageToPlainText: vi.fn()
}))

vi.mock('@renderer/utils/message/partsHelpers', () => ({
  canEditAssistantMessageParts: () => true,
  getTranslationFromParts: () => undefined,
  getTextFromParts: () => 'hello',
  hasTextParts: () => true,
  hasTranslationParts: () => false
}))

vi.mock('react-i18next', () => ({
  initReactI18next: {
    type: '3rdParty',
    init: vi.fn()
  },
  useTranslation: () => ({
    t: (key: string, options?: { value?: string }) =>
      key === 'chat.message.token_details.tokens' ? `${options?.value} Tokens` : key,
    i18n: { resolvedLanguage: 'en-US' }
  })
}))

const { default: MessageMenuBar } = await import('../MessageMenuBar')

const topic = {
  id: 'topic-1',
  assistantId: 'assistant-1',
  name: 'Topic',
  lastActivityAt: '2026-01-01T00:00:00.000Z',
  createdAt: '2026-01-01T00:00:00.000Z',
  updatedAt: '2026-01-01T00:00:00.000Z',
  messages: []
} as Topic

const assistantMessage = {
  id: 'message-1',
  role: 'assistant',
  topicId: topic.id,
  parentId: 'message-0',
  createdAt: '2026-01-01T00:00:00.000Z',
  status: 'success',
  stats: {
    inputTokens: 10,
    outputTokens: 32,
    totalTokens: 42
  }
} as MessageListItem

function renderWithProvider(
  children: ReactNode,
  renderConfig: Partial<typeof defaultMessageRenderConfig> = {},
  actions: MessageListProviderValue['actions'] = {}
) {
  const value: MessageListProviderValue = {
    state: {
      topic,
      messages: [assistantMessage],
      partsByMessageId: {
        [assistantMessage.id]: []
      },
      hasOlder: false,
      messageNavigation: 'none',
      estimateSize: 0,
      overscan: 0,
      loadOlderDelayMs: 0,
      loadingResetDelayMs: 0,
      renderConfig: {
        ...defaultMessageRenderConfig,
        messageStyle: 'bubble',
        ...renderConfig
      },
      selection: {
        enabled: false,
        isMultiSelectMode: false,
        selectedMessageIds: []
      },
      menuConfig: defaultMessageMenuConfig,
      getMessageUiState: () => ({}),
      getMessageActivityState: () => ({
        isProcessing: false,
        isStreamTarget: false,
        isApprovalAnchor: false,
        isActiveTurnProcessing: false,
        isStreamLive: false
      }),
      translationLanguages: []
    },
    actions: {
      copyText: vi.fn(),
      locateMessage: vi.fn(),
      ...actions
    },
    meta: {
      selectionLayer: false
    }
  }

  return render(<MessageListProvider value={value}>{children}</MessageListProvider>)
}

describe('MessageMenuBar', () => {
  it('releases a failed operation and reports its failure before allowing retry', async () => {
    const exportToNotes = vi.fn().mockRejectedValue(new Error('disk full'))
    const notifyError = vi.fn()
    renderWithProvider(
      <MessageMenuBar
        message={assistantMessage}
        isLastMessage
        isAssistantMessage
        isProcessing={false}
        messageContainerRef={{ current: null } as unknown as React.RefObject<HTMLDivElement>}
      />,
      {},
      { exportToNotes, notifyError }
    )
    const notes = screen.getByRole('button', { name: 'notes' })
    fireEvent.click(notes)
    await waitFor(() => expect(notifyError).toHaveBeenCalledTimes(1))
    expect(notes).toBeEnabled()
    fireEvent.click(notes)
    await waitFor(() => expect(notifyError).toHaveBeenCalledTimes(2))
    expect(exportToNotes).toHaveBeenCalledTimes(2)
  })
  it('prevents duplicate note saves across toolbar and submenu while leaving copy available', async () => {
    let finish!: () => void
    const exportToNotes = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve
        })
    )
    renderWithProvider(
      <MessageMenuBar
        message={assistantMessage}
        isLastMessage
        isAssistantMessage
        isProcessing={false}
        messageContainerRef={{ current: null } as unknown as React.RefObject<HTMLDivElement>}
      />,
      {},
      { exportToNotes }
    )
    const notes = screen.getByRole('button', { name: 'notes' })
    const submenu = screen.getByRole('button', { name: 'save.notes' })
    fireEvent.click(notes)
    fireEvent.click(submenu)
    expect(exportToNotes).toHaveBeenCalledTimes(1)
    expect(notes).toBeDisabled()
    expect(submenu).toBeDisabled()
    expect(screen.getByRole('button', { name: 'copy' })).toBeEnabled()
    await act(async () => finish())
    await waitFor(() => expect(notes).toBeEnabled())
    fireEvent.click(submenu)
    expect(exportToNotes).toHaveBeenCalledTimes(2)
    await act(async () => finish())
  })

  it('shows assistant token usage in the bubble footer toolbar regardless of the estimated-tokens setting', () => {
    const { container } = renderWithProvider(
      <MessageMenuBar
        message={assistantMessage}
        isLastMessage
        isAssistantMessage
        isProcessing={false}
        messageContainerRef={{ current: null } as unknown as React.RefObject<HTMLDivElement>}
      />,
      { showEstimatedTokens: false }
    )

    expect(container.querySelector('.message-tokens')).toHaveTextContent('42 Tokens')
  })
})
