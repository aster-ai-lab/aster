import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterAll, beforeEach, describe, expect, it, vi } from 'vitest'

import { MessagePartsScopeProvider } from '@renderer/components/chat/messages/blocks/MessagePartsContext'
import MessageContent from '@renderer/components/chat/messages/frame/MessageContent'
import { WorkbenchAssistantMessageContext } from '@renderer/components/chat/messages/frame/workbenchAssistantMessage'
import type { MessageListItem } from '@renderer/components/chat/messages/types'
import i18n from '@renderer/i18n/resolver'
import { WorkbenchProjectLockedContext } from '@renderer/pages/agents/workbenchComposerHost'
import type { CherryMessagePart } from '@shared/data/types/message'

import {
  arrivalCases,
  diagramCases,
  diffCases,
  emptyFenceCases,
  fenceCases,
  incompleteFenceCases,
  localeCopy,
  mediaCases,
  openDiagramCases,
  pathOnlyCases,
  productCases,
  toolStateCases,
  type TaskConversationPart
} from '../taskConversationCases'
import { TaskConversationMessage } from '../TaskConversationMessage'

const mermaidRender = vi.hoisted(() => vi.fn(async () => ({ svg: '<svg data-testid="task-mermaid-svg"></svg>' })))
const openArtifactFile = vi.hoisted(() => vi.fn())
const writeText = vi.hoisted(() => vi.fn(async () => undefined))
const initialLanguage = i18n.language

vi.mock('mermaid', () => ({
  default: {
    initialize: vi.fn(),
    render: mermaidRender
  }
}))

vi.mock('@renderer/components/chat/messages/MessageListProvider', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@renderer/components/chat/messages/MessageListProvider')>()
  return {
    ...actual,
    useMessageRenderConfig: () => ({
      userName: '',
      narrowMode: false,
      messageStyle: 'bubble' as const,
      messageFont: 'system',
      fontSize: 14,
      renderInputMessageAsMarkdown: false,
      codeFancyBlock: true,
      thoughtAutoCollapse: false,
      collapseCompletedToolHistory: true,
      mathEnableSingleDollar: false,
      showMessageOutline: false,
      showEstimatedTokens: false,
      multiModelMessageStyle: 'horizontal' as const,
      multiModelGridColumns: 2,
      multiModelGridPopoverTrigger: 'click' as const
    }),
    useOptionalMessageListActions: () => ({ openArtifactFile })
  }
})

function assistant(status: MessageListItem['status']): MessageListItem {
  return {
    id: 'turn-1',
    role: 'assistant',
    topicId: 'topic-1',
    createdAt: '2026-10-05T00:00:00.000Z',
    status
  } as MessageListItem
}

function follows(before: Element | null | undefined, after: Element | null | undefined) {
  if (!before || !after) return false
  return Boolean(before.compareDocumentPosition(after) & Node.DOCUMENT_POSITION_FOLLOWING)
}

function renderTurn(parts: readonly TaskConversationPart[], status: MessageListItem['status'] = 'success') {
  return render(
    <WorkbenchProjectLockedContext value={true}>
      <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
        <MessagePartsScopeProvider messageId="turn-1" parts={parts as CherryMessagePart[]}>
          <MessageContent message={assistant(status)} />
        </MessagePartsScopeProvider>
      </WorkbenchAssistantMessageContext>
    </WorkbenchProjectLockedContext>
  )
}

describe('task conversation cases', () => {
  beforeEach(() => {
    mermaidRender.mockClear()
    mermaidRender.mockImplementation(async () => ({ svg: '<svg data-testid="task-mermaid-svg"></svg>' }))
    openArtifactFile.mockClear()
    writeText.mockClear()
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } })
  })

  afterAll(async () => {
    await i18n.changeLanguage(initialLanguage)
  })

  it.each(arrivalCases)('keeps prose and the tool in arrival order ($id)', async (sample) => {
    for (const copy of localeCopy) {
      await i18n.changeLanguage(copy.language)
      const { container, unmount } = renderTurn(sample.parts)
      const proses = container.querySelectorAll('[data-task-prose]')
      const tool = container.querySelector('[data-task-tool-name="bash"]')
      expect(proses[0]?.textContent).toContain(sample.leading)
      expect(proses[1]?.textContent).toContain(sample.trailing)
      expect(follows(proses[0], tool)).toBe(true)
      expect(follows(tool, proses[1])).toBe(true)
      expect(tool).toHaveAttribute('data-task-tool-state', 'success')
      expect(tool?.textContent).toContain(copy.done)
      expect(tool?.textContent).toContain(sample.command)
      unmount()
    }
  })

  it.each(toolStateCases)('shows the tool state on a second command ($id)', async (sample) => {
    for (const copy of localeCopy) {
      await i18n.changeLanguage(copy.language)
      const { container, unmount } = renderTurn(sample.parts)
      const row = container.querySelector('[data-task-tool-name="bash"]')
      expect(row).toHaveAttribute('data-task-tool-state', sample.state)
      expect(row?.textContent).toContain(
        copy[sample.state === 'success' ? 'done' : sample.state === 'error' ? 'failed' : 'running']
      )
      expect(row?.textContent).toContain(sample.command)
      if ('firstLine' in sample && sample.firstLine) {
        const summary = row?.querySelector('.task-tool-row__summary')
        expect(summary?.textContent).toContain(sample.firstLine)
        expect(summary?.textContent).not.toContain(sample.rest)
        fireEvent.click(row?.querySelector('summary') as Element)
        expect(container.querySelector('[data-task-tool-result]')?.textContent).toBe(
          `${sample.firstLine}\n${sample.rest}`
        )
      }
      unmount()
    }
  })

  it.each(fenceCases)('copies a closed fence in its own language ($id)', async (sample) => {
    for (const copy of localeCopy) {
      await i18n.changeLanguage(copy.language)
      writeText.mockClear()
      const { container, unmount } = renderTurn(sample.parts)
      const block = container.querySelector('[data-streamdown="code-block"]') as HTMLElement
      expect(block.querySelector('[data-task-code-language]')).toHaveTextContent(sample.language)
      expect(block.textContent).toContain(sample.source)
      const button = block.querySelector('[data-streamdown="code-block-copy-button"]') as HTMLElement
      expect(button.tagName).toBe('SPAN')
      expect(button.getAttribute('role')).toBeNull()
      expect(button).toHaveAttribute('aria-label', copy.copy)
      fireEvent.click(button)
      await waitFor(() => expect(writeText).toHaveBeenCalled())
      expect(String(writeText.mock.calls.at(-1)?.[0])).toContain(sample.source)
      unmount()
    }
  })

  it.each(incompleteFenceCases)('leaves an incomplete fence as source ($id)', (sample) => {
    const { container } = renderTurn(sample.parts)
    const source = container.querySelector('[data-task-fence="source"]')
    expect(source).toHaveTextContent(sample.source)
    expect(container.querySelector('[data-task-code="highlighted"]')).toBeNull()
    expect(container.querySelector('[data-task-diagram="rendered"]')).toBeNull()
  })

  it.each(emptyFenceCases)('omits an empty closed fence and keeps the prose ($id)', (sample) => {
    const { container } = renderTurn(sample.parts)
    expect(container.querySelector('[data-streamdown="code-block"]')).toBeNull()
    expect(container.querySelector('[data-task-code]')).toBeNull()
    expect(container.textContent).toContain(sample.leading)
    expect(container.textContent).toContain(sample.trailing)
  })

  it.each(diagramCases)('renders a complete diagram ($id)', async (sample) => {
    for (const copy of localeCopy) {
      await i18n.changeLanguage(copy.language)
      mermaidRender.mockClear()
      const { container, unmount } = renderTurn(sample.parts)
      await waitFor(() => expect(container.querySelector('[data-task-diagram="rendered"]')).not.toBeNull())
      const label = copy[sample.kind]
      expect(container.querySelector('[data-task-diagram-label]')).toHaveTextContent(label)
      const drawn = String(mermaidRender.mock.calls.at(-1)?.[1] ?? '')
      expect(drawn).toContain(sample.header)
      expect(drawn).toContain(sample.source)
      unmount()
    }
  })

  it.each(diagramCases)('keeps a failed diagram as source plus the failure note ($id)', async (sample) => {
    mermaidRender.mockImplementation(async () => {
      throw new Error(`bad ${sample.id}`)
    })
    try {
      for (const copy of localeCopy) {
        await i18n.changeLanguage(copy.language)
        const { container, unmount } = renderTurn(sample.parts)
        await waitFor(() => expect(container.querySelector('[data-chat-diagram="failed"]')).not.toBeNull())
        const failed = container.querySelector('[data-chat-diagram="failed"]')
        expect(failed).toHaveAttribute('data-task-diagram', 'source')
        expect(failed?.textContent).toContain(sample.source)
        expect(failed?.textContent).toContain(copy.diagramFailed)
        expect(container.querySelector('[data-task-diagram="rendered"]')).toBeNull()
        unmount()
      }
    } finally {
      mermaidRender.mockImplementation(async () => ({ svg: '<svg data-testid="task-mermaid-svg"></svg>' }))
    }
  })

  it.each(openDiagramCases)('leaves a trailing open diagram as source ($id)', (sample) => {
    mermaidRender.mockClear()
    const { container } = renderTurn(sample.parts)
    expect(container.querySelector('[data-task-diagram="source"]')).toHaveTextContent(sample.source)
    expect(container.querySelector('[data-task-diagram="rendered"]')).toBeNull()
    expect(mermaidRender).not.toHaveBeenCalled()
  })

  it.each(diffCases)('shows hunks, both layouts, and no invented product ($id)', async (sample) => {
    for (const copy of localeCopy) {
      await i18n.changeLanguage(copy.language)
      const { container, unmount } = renderTurn(sample.parts)
      const row = container.querySelector('[data-task-tool-name="edit"]')
      expect(row?.textContent).toContain(sample.file)
      expect(row?.textContent).toContain('+1 · \u22121')
      fireEvent.click(row?.querySelector('summary') as Element)
      expect(container.querySelector('[data-code-diff-mode="unified"]')).not.toBeNull()
      expect(container.querySelector('[data-kind="deletion"] code')).toHaveTextContent(sample.removed)
      expect(container.querySelector('[data-kind="addition"] code')).toHaveTextContent(sample.added)
      fireEvent.click(screen.getByRole('button', { name: copy.split }))
      expect(container.querySelector('[data-code-diff-mode="split"]')).not.toBeNull()
      expect(container.querySelector('[data-code-diff-side="old"][data-kind="deletion"]')).toHaveTextContent(
        sample.removed
      )
      expect(container.querySelector('[data-code-diff-side="new"][data-kind="addition"]')).toHaveTextContent(
        sample.added
      )
      fireEvent.click(screen.getByRole('button', { name: copy.unified }))
      expect(container.querySelector('[data-code-diff-mode="unified"]')).not.toBeNull()
      expect(container.querySelector('[data-task-product]')).toBeNull()
      unmount()
    }
  })

  it.each(pathOnlyCases)('names a path-only edit and does not invent hunks ($id)', async (sample) => {
    for (const copy of localeCopy) {
      await i18n.changeLanguage(copy.language)
      const { container, unmount } = renderTurn(sample.parts)
      const row = container.querySelector('[data-task-tool-name="edit"]')
      expect(row?.textContent).toContain(sample.file)
      expect(row?.textContent).toContain(copy.diffPending)
      expect(row?.textContent).not.toMatch(/\+\d+ · −\d+/)
      fireEvent.click(row?.querySelector('summary') as Element)
      expect(container.querySelector('[data-code-diff-status="pending"]')).not.toBeNull()
      expect(container.querySelector('[data-kind="addition"]')).toBeNull()
      expect(container.querySelector('[data-kind="deletion"]')).toBeNull()
      expect(container.querySelector('[data-task-product]')).toBeNull()
      unmount()
    }
  })

  it.each(productCases)('shows a product only after a finished successful write ($id)', async (sample) => {
    for (const copy of localeCopy) {
      await i18n.changeLanguage(copy.language)
      const wrote = renderTurn(sample.parts)
      const chip = wrote.container.querySelector('[data-task-product]')
      expect(wrote.container.querySelectorAll('[data-task-product]')).toHaveLength(1)
      expect(chip).toHaveAttribute('data-task-product-path', sample.path)
      expect(chip).toHaveTextContent(sample.name)
      expect(wrote.container.querySelector('[data-task-products]')).toHaveTextContent(copy.products)
      expect(wrote.container.querySelector(`[data-task-product-path="${sample.hidden}"]`)).toBeNull()
      wrote.unmount()

      const streaming = renderTurn(sample.parts, 'pending')
      expect(streaming.container.querySelector('[data-task-product]')).toBeNull()
      streaming.unmount()

      const failed = renderTurn(sample.failed)
      expect(failed.container.querySelector('[data-task-product]')).toBeNull()
      failed.unmount()
    }
  })

  it.each(mediaCases)('keeps reasoning and images in arrival order ($id)', async (sample) => {
    const { container } = renderTurn(sample.parts)
    const html = container.innerHTML
    const proseAt = html.indexOf(sample.id === 'prose-then-image' ? '前文甲' : '前文乙')
    const reasoningAt = html.indexOf('part:message-reasoning')
    const imageAt = html.indexOf(sample.image)
    const answerAt = html.indexOf(sample.prose)
    const positions = {
      prose: proseAt,
      reasoning: reasoningAt,
      image: imageAt,
      answer: answerAt
    }
    const indexes = sample.order.map((key) => positions[key])
    expect(indexes.every((index) => index >= 0)).toBe(true)
    expect([...indexes].sort((left, right) => left - right)).toEqual(indexes)
    expect(container.textContent).not.toContain(sample.dropped)
    expect(container.textContent).toContain(sample.kept)
    expect(container.querySelector('img')).toHaveAttribute('src', sample.image)
    await waitFor(() => expect(container.querySelector('[data-streamdown="code-block"]')).not.toBeNull())
    expect(container.querySelector('[data-streamdown="code-block"]')?.textContent).toContain(sample.kept)
  })
})
