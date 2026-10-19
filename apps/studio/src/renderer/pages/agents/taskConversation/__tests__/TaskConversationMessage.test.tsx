import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@renderer/components/chat/messages/markdown/OpenUiMessage', () => ({
  OpenUiMessage: ({ source, streaming }: { source: string; streaming: boolean }) => (
    <div data-testid="task-message-openui" data-streaming={String(streaming)}>
      {source}
    </div>
  )
}))

const openTab = vi.hoisted(() => vi.fn())

vi.mock('@renderer/hooks/tab', () => ({ useTabs: () => ({ openTab }) }))

vi.mock('@renderer/drive/encyclopediaClient', async (original) => ({
  ...(await original<typeof import('@renderer/drive/encyclopediaClient')>()),
  teamPassageIsStale: vi.fn(async () => true)
}))

const taskConversationCss = readFileSync(
  resolve(dirname(fileURLToPath(import.meta.url)), '../task-conversation.css'),
  'utf8'
)
const mainStyles = readFileSync(
  resolve(dirname(fileURLToPath(import.meta.url)), '../../../../assets/styles/index.css'),
  'utf8'
)

import { MessagePartsScopeProvider } from '@renderer/components/chat/messages/blocks/MessagePartsContext'
import MessageContent from '@renderer/components/chat/messages/frame/MessageContent'
import { WorkbenchAssistantMessageContext } from '@renderer/components/chat/messages/frame/workbenchAssistantMessage'
import type { MessageListItem } from '@renderer/components/chat/messages/types'
import { PERSONAL_CENTER, writeCenterId } from '@renderer/data/memberOrgs'
import { teamPassageIsStale } from '@renderer/drive/encyclopediaClient'
import { resetSavedTaskProductsForTests } from '@renderer/drive/TaskFindingSave'
import enUs from '@renderer/i18n/locales/en-us.json'
import zhCn from '@renderer/i18n/locales/zh-cn.json'
import { WorkbenchProjectLockedContext, WorkbenchTurnStreamContext } from '@renderer/pages/agents/workbenchComposerHost'
import {
  resetPluginCatalogForTests,
  setPluginCatalogForTests
} from '@renderer/pages/settings/DependenciesSettings/pluginCatalog'
import type { CherryMessagePart } from '@shared/data/types/message'

import { officialWritingTaskText } from '../../officialWriting'
import {
  armOfficialWritingDocument,
  readOfficialWritingSession,
  resetOfficialWritingSessionForTests
} from '../../officialWritingAgentEdit'
import { TaskConversationMessage } from '../TaskConversationMessage'

const mermaidRender = vi.hoisted(() => vi.fn(async () => ({ svg: '<svg data-testid="task-mermaid-svg"></svg>' })))
const openArtifactFile = vi.hoisted(() => vi.fn())
const messageRenderConfig = vi.hoisted(() => ({
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
}))

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
    useMessageRenderConfig: () => messageRenderConfig,
    useOptionalMessageListActions: () => ({ openArtifactFile }),
    useMessageListActions: () => ({
      openArtifactFile,
      removeMessageTranslation: vi.fn(),
      notifySuccess: vi.fn(),
      openAgentToolFlow: vi.fn(),
      isAgentToolFlowActive: () => false
    }),
    useMessageListItemActivityState: () => ({
      isProcessing: false,
      isStreamTarget: false,
      isApprovalAnchor: false,
      isActiveTurnProcessing: false,
      isStreamLive: false
    })
  }
})

function assistant(status: MessageListItem['status']): MessageListItem {
  return {
    id: 'turn-1',
    role: 'assistant',
    topicId: 'topic-1',
    createdAt: '2026-10-04T00:00:00.000Z',
    status
  } as MessageListItem
}

function follows(before: Element | null | undefined, after: Element | null | undefined) {
  if (!before || !after) return false
  return Boolean(before.compareDocumentPosition(after) & Node.DOCUMENT_POSITION_FOLLOWING)
}

function renderTurn(parts: CherryMessagePart[], status: MessageListItem['status'] = 'success') {
  return render(
    <WorkbenchProjectLockedContext value={true}>
      <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
        <MessagePartsScopeProvider messageId="turn-1" parts={parts}>
          <MessageContent message={assistant(status)} />
        </MessagePartsScopeProvider>
      </WorkbenchAssistantMessageContext>
    </WorkbenchProjectLockedContext>
  )
}

describe('task conversation turn', () => {
  it('mounts the OpenUI host while the model fence is still streaming', () => {
    render(
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
          <MessagePartsScopeProvider
            messageId="turn-1"
            parts={[{ type: 'text', text: '```openui\nroot = Card("Budget")' }] as CherryMessagePart[]}>
            <MessageContent message={assistant('pending')} />
          </MessagePartsScopeProvider>
        </WorkbenchAssistantMessageContext>
      </WorkbenchProjectLockedContext>
    )
    expect(screen.getByTestId('task-message-openui')).toHaveAttribute('data-streaming', 'true')
    expect(screen.getByTestId('task-message-openui')).toHaveTextContent('root = Card("Budget")')
  })

  it('paints a completed json display document as a form', () => {
    const source = JSON.stringify({
      root: 'leave_card',
      state: { form: { cm_content: '去医院', cm_type: '病假', days: 2 } }
    })
    renderTurn([{ type: 'text', text: ['```json', source, '```'].join('\n') }] as CherryMessagePart[])
    const card = screen.getByTestId('task-message-openui')
    expect(card).toHaveAttribute('data-streaming', 'false')
    expect(card).toHaveTextContent('Input("cm_content", "cm_content", "去医院", "text")')
    expect(card).toHaveTextContent('root = Card("leave_card"')
  })

  it('keeps an unfinished json fence as source', () => {
    renderTurn(
      [
        {
          type: 'text',
          text: '```json\n{"root":"leave_card","state":{"form":{"cm_content":"去医院"}}}'
        }
      ] as CherryMessagePart[],
      'pending'
    )
    expect(screen.queryByTestId('task-message-openui')).toBeNull()
  })
  it('keeps an unbound historical form readable without live controls', () => {
    renderTurn(
      [
        {
          type: 'data-form',
          data: {
            id: 'form-1',
            format: 'openui',
            state: 'form-requested',
            spec: 'root = Input("topic", "Topic")'
          }
        }
      ] as CherryMessagePart[],
      'pending'
    )
    expect(screen.getByText('root = Input("topic", "Topic")').closest('pre')).toHaveAttribute(
      'data-intelligent-ui-form',
      'legacy-source'
    )
    expect(screen.queryByRole('textbox', { name: 'Topic' })).not.toBeInTheDocument()
  })
  it('lets the form card stand for the tool call that asked it', () => {
    const spec = 'root = Input("topic", "Topic")'
    const { container } = renderTurn(
      [
        {
          type: 'dynamic-tool',
          toolName: 'ask_user_form',
          toolCallId: 'call-form',
          state: 'input-available',
          input: { form: spec }
        },
        {
          type: 'dynamic-tool',
          toolName: 'ask_user_form',
          toolCallId: 'call-other',
          state: 'input-available',
          input: { form: 'root = Input("other", "Other")' }
        },
        { type: 'data-form', data: { id: 'form-1', format: 'openui', state: 'form-requested', spec } }
      ] as CherryMessagePart[],
      'pending'
    )
    const rows = container.querySelectorAll('[data-task-tool-name="ask_user_form"]')
    expect(rows).toHaveLength(1)
    expect(rows[0]?.querySelector('[data-task-tool-label]')).toHaveTextContent(/请你填写表单|Ask you to fill in a form/)
    expect(container.querySelector('[data-intelligent-ui-form]')).not.toBeNull()
  })
  beforeAll(() => {
    const style = document.createElement('style')
    style.setAttribute('data-task-conversation-contract', 'true')
    style.textContent = taskConversationCss
    document.head.appendChild(style)
  })

  it('keeps the new work-product strings in zh-cn and en-us', () => {
    expect(zhCn['workbench.composer.review']).toBe('自动审查')
    expect(zhCn['workbench.composer.trace']).toBe('产物追溯')
    expect(enUs['workbench.composer.review']).toBe('Auto review')
    expect(enUs['workbench.composer.trace']).toBe('Artifact lineage')
    expect(zhCn['workbench.products']).toBe('工作产物')
    expect(zhCn['workbench.products.code']).toBe('代码')
    expect(zhCn['workbench.products.document']).toBe('文档')
    expect(zhCn['workbench.products.file']).toBe('文件')
    expect(zhCn['workbench.products.image']).toBe('图片')
    expect(zhCn['workbench.products.open']).toBe('打开 {{name}}')
    expect(zhCn['workbench.products.page']).toBe('网页')
    expect(enUs['workbench.products']).toBe('Work products')
    expect(enUs['workbench.products.code']).toBe('Code')
    expect(enUs['workbench.products.document']).toBe('Document')
    expect(enUs['workbench.products.file']).toBe('File')
    expect(enUs['workbench.products.image']).toBe('Image')
    expect(enUs['workbench.products.open']).toBe('Open {{name}}')
    expect(enUs['workbench.products.page']).toBe('Web page')
    expect(zhCn['workbench.diagram.failed']).toBe('图表未能渲染')
    expect(enUs['workbench.diagram.failed']).toBe('The diagram could not be rendered')
    expect(zhCn['workbench.tool.diff_pending']).toBe('差异待生成')
    expect(enUs['workbench.tool.diff_pending']).toBe('Diff pending')
    expect(zhCn['workbench.tool.running']).toBe('执行中')
    expect(enUs['workbench.tool.running']).toBe('Running')
    expect(zhCn['workbench.tool.pending']).toBe('待确认')
    expect(enUs['workbench.tool.pending']).toBe('Needs approval')
    expect(zhCn['workbench.tool.done']).toBe('已完成')
    expect(enUs['workbench.tool.done']).toBe('Done')
    expect(zhCn['workbench.tool.failed']).toBe('失败')
    expect(enUs['workbench.tool.failed']).toBe('Failed')
  })

  beforeEach(() => {
    mermaidRender.mockClear()
    openArtifactFile.mockClear()
    resetOfficialWritingSessionForTests()
    resetSavedTaskProductsForTests()
  })

  it('keeps the armed document until the assistant draft finishes streaming', () => {
    const task = officialWritingTaskText('notice', '周五开会', { banner: false }, { org: '市教育局', mainTo: '各学校' })
    armOfficialWritingDocument(task ?? '')
    const armed = readOfficialWritingSession()?.document ?? ''
    const marker = '（待填写）'
    const markerAt = armed.indexOf(marker)
    const finished = `${armed.slice(0, markerAt)}请各校于周五下午开会。${armed.slice(markerAt + marker.length)}`
    const parts = [{ type: 'text', text: finished }] as CherryMessagePart[]

    const view = renderTurn(parts, 'pending')
    expect(readOfficialWritingSession()?.document).toBe(armed)
    expect(readOfficialWritingSession()?.update).toBeNull()

    view.rerender(
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
          <MessagePartsScopeProvider messageId="turn-1" parts={parts}>
            <MessageContent message={assistant('success')} />
          </MessagePartsScopeProvider>
        </WorkbenchAssistantMessageContext>
      </WorkbenchProjectLockedContext>
    )
    expect(readOfficialWritingSession()?.document).toBe(armed)
    expect(readOfficialWritingSession()?.update).toBe(finished)
  })

  it('offers drive save on the product files after a team turn finishes', () => {
    const org = '0b9d2a4e-1c3f-4a5b-8d6e-7f8091a2b3c4'
    const files = [
      {
        type: 'dynamic-tool',
        toolName: 'edit',
        toolCallId: 't-write',
        state: 'output-available',
        input: {},
        output: '',
        files: ['/workspace/notes.md']
      }
    ] as CherryMessagePart[]
    try {
      writeCenterId(PERSONAL_CENTER)
      const personal = renderTurn(files)
      expect(personal.container.querySelector('[data-task-product]')).not.toBeNull()
      expect(personal.container.querySelector('[data-ui="task.save-finding"]')).toBeNull()
      personal.unmount()

      writeCenterId(org)
      const live = renderTurn(files, 'pending')
      expect(live.container.querySelector('[data-ui="task.save-finding"]')).toBeNull()
      live.unmount()

      const textOnly = renderTurn([{ type: 'text', text: '结论' }])
      expect(textOnly.container.querySelector('[data-ui="task.save-finding"]')).toBeNull()
      textOnly.unmount()

      const done = renderTurn(files)
      const button = done.container.querySelector('[data-ui="task.save-finding"]')
      const item = button?.closest('.task-product-item')
      expect(item?.querySelector('[data-task-product]')).not.toBeNull()
      expect(done.container.querySelector('header [data-ui="task.save-finding"]')).toBeNull()
      expect(button?.closest('[data-ui~="part:message-actions"]')).toBeNull()
      expect(button?.querySelector('svg')).not.toBeNull()
      expect(button?.textContent).not.toContain('写入云盘')
      done.unmount()
    } finally {
      writeCenterId(PERSONAL_CENTER)
      localStorage.removeItem('a3s.org.center')
      document.cookie = 'a3s.org.center=; Path=/; Max-Age=0'
    }
  })

  it('shows a loading row on a pending assistant turn before any reply arrives', () => {
    renderTurn([], 'pending')
    const pending = screen.getByTestId('task-pending')
    const verb = pending.querySelector('[data-testid="task-pending-verb"]')?.textContent ?? ''
    expect(pending.querySelector('[data-tool-spinner]')).not.toBeNull()
    expect(pending.querySelector('.task-pending__dots')).toHaveTextContent('...')
    expect(verb.startsWith('正在') || /[A-Za-z]/.test(verb)).toBe(true)
  })

  it('keeps the loading row after prose while the turn is still running', () => {
    const { container } = renderTurn([{ type: 'text', text: '你好' }] as CherryMessagePart[], 'pending')
    const prose = container.querySelector('[data-task-prose]')
    const pending = screen.getByTestId('task-pending')
    expect(pending.querySelector('[data-tool-spinner]')).not.toBeNull()
    expect(pending.querySelector('.task-pending__dots')).toHaveTextContent('...')
    expect(container.querySelector('[data-task-turn-state="live"]')).not.toBeNull()
    expect(follows(prose, pending)).toBe(true)
  })

  it('hides the loading row once the turn finishes', () => {
    const { container } = renderTurn([{ type: 'text', text: '你好' }] as CherryMessagePart[], 'success')
    expect(screen.queryByTestId('task-pending')).toBeNull()
    expect(container.querySelector('[data-task-turn-state="idle"]')).not.toBeNull()
  })

  it('shows the loading row after the latest tool while the turn is still waiting', () => {
    const { container } = renderTurn(
      [
        {
          type: 'tool-read',
          toolCallId: 'call-read',
          state: 'output-available',
          input: { path: 'a.ts' },
          output: 'ok'
        }
      ] as CherryMessagePart[],
      'pending'
    )
    const tool = container.querySelector('[data-task-tool-name="read"]')
    const pending = screen.getByTestId('task-pending')
    expect(follows(tool, pending)).toBe(true)
  })

  it('keeps a finished tool quiet once the topic stream has ended', () => {
    render(
      <WorkbenchTurnStreamContext value="done">
        <WorkbenchProjectLockedContext value={true}>
          <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
            <MessagePartsScopeProvider
              messageId="turn-1"
              parts={
                [
                  {
                    type: 'tool-read',
                    toolCallId: 'call-read',
                    state: 'output-available',
                    input: { path: 'a.ts' },
                    output: 'ok'
                  }
                ] as CherryMessagePart[]
              }>
              <MessageContent latest message={assistant('pending')} />
            </MessagePartsScopeProvider>
          </WorkbenchAssistantMessageContext>
        </WorkbenchProjectLockedContext>
      </WorkbenchTurnStreamContext>
    )
    expect(screen.queryByTestId('task-pending')).toBeNull()
  })

  it('hides changed files on the latest turn while it waits for approval', () => {
    const parts = [
      {
        type: 'tool-write_file',
        state: 'output-available',
        input: { file_path: '/tmp/a.ts', content: 'next\n' }
      }
    ] as CherryMessagePart[]
    const tree = (status: 'awaiting-approval' | 'done', latest: boolean) => (
      <WorkbenchTurnStreamContext value={status}>
        <WorkbenchProjectLockedContext value={true}>
          <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
            <MessagePartsScopeProvider messageId="turn-1" parts={parts}>
              <MessageContent latest={latest} message={assistant('success')} />
            </MessagePartsScopeProvider>
          </WorkbenchAssistantMessageContext>
        </WorkbenchProjectLockedContext>
      </WorkbenchTurnStreamContext>
    )

    const waiting = render(tree('awaiting-approval', true))
    expect(waiting.container.querySelector('[data-task-changed-files]')).toBeNull()
    waiting.unmount()

    const older = render(tree('awaiting-approval', false))
    expect(older.container.querySelector('[data-task-changed-path="/tmp/a.ts"]')).not.toBeNull()
    older.unmount()

    const finished = render(tree('done', true))
    expect(finished.container.querySelector('[data-task-changed-path="/tmp/a.ts"]')).not.toBeNull()
  })

  it('shows the working row after a finished tool while the topic stream is still live', () => {
    const { container } = render(
      <WorkbenchTurnStreamContext value="streaming">
        <WorkbenchProjectLockedContext value={true}>
          <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
            <MessagePartsScopeProvider
              messageId="turn-1"
              parts={
                [
                  {
                    type: 'tool-read',
                    toolCallId: 'call-read',
                    state: 'output-available',
                    input: { path: 'a.ts' },
                    output: 'ok'
                  }
                ] as CherryMessagePart[]
              }>
              <MessageContent latest message={assistant('success')} />
            </MessagePartsScopeProvider>
          </WorkbenchAssistantMessageContext>
        </WorkbenchProjectLockedContext>
      </WorkbenchTurnStreamContext>
    )
    const tool = container.querySelector('[data-task-tool-state="success"]')
    const pending = screen.getByTestId('task-pending')
    expect(follows(tool, pending)).toBe(true)
  })

  it('shows prose and tool rows in arrival order for a live call and a restored call', () => {
    const { container } = renderTurn(
      [
        { type: 'text', text: '先查文件。' },
        {
          type: 'tool-read',
          toolCallId: 'call-read',
          state: 'input-streaming',
          input: { path: 'a.ts' }
        },
        { type: 'text', text: '找到了。' },
        {
          type: 'tool-write',
          toolCallId: 'call-write',
          state: 'output-available',
          input: { path: 'b.ts' },
          output: 'ok'
        }
      ] as CherryMessagePart[],
      'pending'
    )

    const proses = container.querySelectorAll('[data-task-prose]')
    const read = container.querySelector('[data-task-tool-name="read"]')
    const write = container.querySelector('[data-task-tool-name="write"]')
    expect(proses[0]?.textContent).toContain('先查文件')
    expect(proses[1]?.textContent).toContain('找到了')
    expect(follows(proses[0], read)).toBe(true)
    expect(follows(read, proses[1])).toBe(true)
    expect(follows(proses[1], write)).toBe(true)

    expect(read).toHaveAttribute('data-task-tool-row', '')
    expect(write).toHaveAttribute('data-task-tool-row', '')
    expect(read).toHaveAttribute('data-tool-call', 'true')
    expect(read).toHaveAttribute('data-task-tool-state', 'running')
    expect(write).toHaveAttribute('data-task-tool-state', 'success')
    expect(read?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Read a\.ts|读取 a\.ts/)
    expect(write?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Write b\.ts|写入 b\.ts/)
    expect(read?.textContent).toMatch(/Running|执行中/)

    const waiting = renderTurn([
      {
        type: 'dynamic-tool',
        toolName: '是否执行这条命令：printf a3s-code-prompt-ok ?',
        toolCallId: 'call-ask',
        state: 'approval-requested',
        input: {}
      }
    ] as CherryMessagePart[])
    const ask = waiting.container.querySelector('[data-task-tool-row]')
    expect(ask).toHaveAttribute('data-task-tool-state', 'pending')
    expect(ask?.querySelector('[data-task-tool-label]')).toHaveTextContent('printf a3s-code-prompt-ok')
    expect(ask?.textContent).toMatch(/Needs approval|待确认/)
    expect(ask?.textContent).not.toMatch(/执行中|Running/)
    expect(ask?.querySelector('[data-task-tool-spinner]')).toBeNull()
    expect(ask?.querySelector('[data-task-tool-wait]')).not.toBeNull()
    waiting.unmount()
    expect(write?.textContent).toMatch(/Done|已完成/)
    expect(read?.querySelector('[data-task-tool-spinner]')).not.toBeNull()
    expect(container.querySelector('[data-task-prose]')).toHaveClass('task-conversation__markdown')
    expect(read?.closest('[data-task-tool-group]')).toBeNull()
    expect(write?.closest('[data-task-tool-group]')).toBeNull()
  })

  it('groups consecutive tool calls on a left rail and keeps a prose break split', () => {
    const { container } = renderTurn([
      {
        type: 'tool-read',
        toolCallId: 'call-read',
        state: 'output-available',
        input: { path: 'a.ts' },
        output: 'ok'
      },
      {
        type: 'tool-bash',
        toolCallId: 'call-bash',
        state: 'output-available',
        input: { command: 'ls' },
        output: { stdout: 'a.ts' }
      },
      { type: 'text', text: '中间。' },
      {
        type: 'tool-write',
        toolCallId: 'call-write',
        state: 'output-available',
        input: { path: 'b.ts' },
        output: 'ok'
      }
    ] as CherryMessagePart[])

    const groups = container.querySelectorAll('[data-task-tool-group]')
    expect(groups).toHaveLength(1)
    const group = groups[0] as HTMLElement
    expect(group).toHaveAttribute('data-task-tool-count', '2')
    const grouped = group.querySelectorAll('[data-task-tool-row]')
    expect(grouped).toHaveLength(2)
    expect(grouped[0]).toHaveAttribute('data-task-tool-name', 'read')
    expect(grouped[1]).toHaveAttribute('data-task-tool-name', 'bash')
    expect(group.querySelectorAll('.task-tool-group__item')).toHaveLength(2)
    const prose = container.querySelector('[data-task-prose]')
    const write = container.querySelector('[data-task-tool-name="write"]')
    expect(follows(group, prose)).toBe(true)
    expect(follows(prose, write)).toBe(true)
    expect(write?.closest('[data-task-tool-group]')).toBeNull()
    expect(getComputedStyle(group).marginInlineEnd).toBe('auto')
    expect(getComputedStyle(group).width).toBe('max-content')
    const summary = grouped[0]?.querySelector('.task-tool-row__summary') as HTMLElement
    expect(getComputedStyle(summary).backgroundColor).toBe('rgba(0, 0, 0, 0)')
    expect(getComputedStyle(grouped[0] as HTMLElement).margin).toBe('0px')
    const status = grouped[0]?.querySelector('[data-task-tool-status]') as HTMLElement
    expect(status.textContent).toMatch(/Done|已完成/)
    expect(getComputedStyle(status).position).toBe('absolute')
    expect(taskConversationCss).toMatch(
      /\.task-tool-group__item:not\(:last-child\)::before \{[^}]*top:\s*26px;[^}]*bottom:\s*-10px;[^}]*left:\s*17px;[^}]*width:\s*1px;/
    )
  })

  it('shows wiki search as note retrieval, not a raw index token', () => {
    const preparing = renderTurn([
      {
        type: 'tool-search_wiki',
        toolCallId: 'wiki-preparing',
        state: 'output-available',
        input: { query: '北城豆沙' },
        output: 'not-ready'
      }
    ] as CherryMessagePart[])
    const preparingRow = preparing.container.querySelector('[data-task-tool-name="search_wiki"]')
    expect(preparingRow?.querySelector('[data-task-tool-label]')).toHaveTextContent(
      /检索笔记 北城豆沙|Search notes 北城豆沙/
    )
    expect(preparingRow?.textContent).toMatch(
      /笔记检索还在准备，会自动再试|Note search is still preparing and will be tried again/
    )
    expect(preparingRow?.textContent).toMatch(/准备中|Preparing/)
    expect(preparingRow?.textContent).not.toContain('not-ready')
    expect(preparingRow?.textContent).not.toMatch(/已完成|Done/)
    expect(getComputedStyle(preparingRow?.querySelector('[data-tool-status]') as Element).color).toBe(
      'rgb(138, 138, 138)'
    )
    preparing.unmount()

    const missed = renderTurn([
      {
        type: 'tool-search_wiki',
        toolCallId: 'wiki-miss',
        state: 'output-available',
        input: { query: '北城豆沙' },
        output: 'no-match'
      }
    ] as CherryMessagePart[])
    const missedRow = missed.container.querySelector('[data-task-tool-name="search_wiki"]')
    expect(missedRow?.textContent).toMatch(/没有匹配的笔记|No matching notes/)
    expect(missedRow?.textContent).not.toContain('no-match')
    expect(missedRow?.textContent).toMatch(/已完成|Done/)
    missed.unmount()

    const { container } = renderTurn([
      {
        type: 'tool-search_wiki',
        toolCallId: 'wiki-hit',
        state: 'output-available',
        input: { query: '北城豆沙' },
        output: '食谱\nlocal/偏好/食谱.md\n北城豆沙只写在食谱9f3a。'
      }
    ] as CherryMessagePart[])
    const row = container.querySelector('[data-task-tool-name="search_wiki"]')
    expect(row?.querySelector('[data-agent-tool-copy] small')).toHaveTextContent('食谱')
    fireEvent.click(row?.querySelector('summary') as Element)
    const hit = container.querySelector('[data-wiki-search-hit]')
    expect(hit?.querySelector('.task-wiki-hit__name')).toHaveTextContent('食谱')
    expect(hit?.querySelector('.task-wiki-hit__path')).toHaveTextContent('local/偏好/食谱.md')
    expect(hit?.querySelector('.task-wiki-hit__passage')).toHaveTextContent('北城豆沙只写在食谱9f3a。')
    expect(container.querySelector('[data-task-tool-result]')).toBeNull()
    expect(getComputedStyle(hit?.querySelector('.task-wiki-hit__name') as Element).fontSize).toBe('13px')
    expect(getComputedStyle(hit?.querySelector('.task-wiki-hit__name') as Element).color).toBe('rgb(23, 23, 23)')
    expect(getComputedStyle(hit?.querySelector('.task-wiki-hit__path') as Element).fontSize).toBe('12px')
    expect(getComputedStyle(hit?.querySelector('.task-wiki-hit__path') as Element).color).toBe('rgb(138, 138, 138)')
    expect(getComputedStyle(hit?.querySelector('.task-wiki-hit__passage') as Element).color).toBe('rgb(63, 63, 63)')
  })

  it('links a team passage to its drive file and flags a newer generation before opening it', async () => {
    const file = '6f1c1f1e-0000-4000-8000-0000000000aa'
    const passage = '6f1c1f1e-0000-4000-8000-000000000001'
    const { container } = renderTurn([
      {
        type: 'tool-search_wiki',
        toolCallId: 'wiki-team',
        state: 'output-available',
        input: { query: '出差回来多久要交票' },
        output: `差旅票据提交时限\nteam/${file}/${passage}@1\n出差结束后十个工作日内提交。`
      }
    ] as CherryMessagePart[])
    fireEvent.click(container.querySelector('[data-task-tool-name="search_wiki"] summary') as Element)
    const link = container.querySelector('[data-ui="wiki.team-citation"]')
    expect(link).toHaveAttribute('href', `/app/files?driveFile=${file}`)
    expect(link).toHaveAttribute('data-passage', passage)
    expect(link).toHaveTextContent(/打开来源|Open source/)
    fireEvent.click(link as Element)
    await waitFor(() =>
      expect(container.querySelector('[data-ui="wiki.team-citation-stale"]')).toHaveTextContent(
        /该段落已更新|This passage has been updated/
      )
    )
    expect(openTab).not.toHaveBeenCalled()
    fireEvent.click(link as Element)
    expect(openTab).toHaveBeenCalledWith(`/app/files?driveFile=${file}`)
  })

  it('flags an older generation in the same result before opening it, and opens a current hit', async () => {
    vi.mocked(teamPassageIsStale).mockResolvedValue(false)
    openTab.mockClear()
    const file = '6f1c1f1e-0000-4000-8000-0000000000bb'
    const older = '6f1c1f1e-0000-4000-8000-000000000002'
    const newer = '6f1c1f1e-0000-4000-8000-000000000003'
    try {
      const { container } = renderTurn([
        {
          type: 'tool-search_wiki',
          toolCallId: 'wiki-gen',
          state: 'output-available',
          input: { query: '口径' },
          output: `过期口径\nteam/${file}/${older}@1\n旧段落。\n\n新口径\nteam/${file}/${newer}@4\n新段落。`
        }
      ] as CherryMessagePart[])
      fireEvent.click(container.querySelector('[data-task-tool-name="search_wiki"] summary') as Element)
      const links = [...container.querySelectorAll('[data-ui="wiki.team-citation"]')]
      const staleLink = links.find((link) => link.getAttribute('data-passage') === older)
      const freshLink = links.find((link) => link.getAttribute('data-passage') === newer)
      fireEvent.click(staleLink as Element)
      expect(container.querySelector('[data-ui="wiki.team-citation-stale"]')).toHaveTextContent(
        /该段落已更新|This passage has been updated/
      )
      expect(openTab).not.toHaveBeenCalled()
      fireEvent.click(staleLink as Element)
      expect(openTab).toHaveBeenCalledWith(`/app/files?driveFile=${file}`)
      openTab.mockClear()
      fireEvent.click(freshLink as Element)
      await waitFor(() => expect(openTab).toHaveBeenCalledWith(`/app/files?driveFile=${file}`))
    } finally {
      vi.mocked(teamPassageIsStale).mockResolvedValue(true)
    }
  })

  it('shows an error tool row with the same presentation', () => {
    const { container } = renderTurn([
      {
        type: 'tool-bash',
        toolCallId: 'call-bash',
        state: 'output-error',
        input: { command: 'nope' },
        errorText: 'failed'
      }
    ] as CherryMessagePart[])

    const row = container.querySelector('[data-task-tool-name="bash"]')
    expect(row).toHaveAttribute('data-task-tool-row', '')
    expect(row).toHaveAttribute('data-task-tool-state', 'error')
    expect(row?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Run nope|运行 nope/)
    expect(row?.textContent).toMatch(/Failed|失败/)
    const status = row?.querySelector('[data-tool-status]')
    expect(status).toHaveAttribute('data-state', 'error')
    expect(getComputedStyle(status as Element).color).toBe('rgb(180, 35, 24)')
    expect(row?.querySelector('.task-tool-row__summary')?.textContent).not.toContain('failed')
    expect(row?.querySelector('.task-tool-row__summary')?.textContent).not.toContain('{')
    fireEvent.click(row?.querySelector('summary') as Element)
    expect(container.querySelector('[data-tool-payload="input"]')).toHaveTextContent('nope')
    expect(container.querySelector('[data-tool-payload="input"] h4')).toHaveTextContent(/输入|Input/)
    expect(container.querySelector('[data-task-tool-error]')).toHaveTextContent('failed')
    expect(container.querySelector('[data-tool-payload="error"] h4')).toHaveTextContent(/错误|Error/)
  })

  it('keeps an assistant error note in the turn', () => {
    renderTurn([
      { type: 'text', text: '没能完成。' },
      { type: 'data-error', data: { message: '发生未知错误' } }
    ] as CherryMessagePart[])

    expect(screen.getByText('没能完成。')).toBeInTheDocument()
    expect(screen.getByText('发生未知错误')).toBeInTheDocument()
  })

  it('shows the recovered scheduled task failure instead of preparing forever', () => {
    const reason = '执行已中断，运行进程已不存在。请重新运行任务。'
    renderTurn([{ type: 'data-error', data: { name: 'Error', message: reason } }] as CherryMessagePart[], 'error')
    expect(screen.getByText(reason)).toBeInTheDocument()
    expect(screen.queryByTestId('task-pending')).not.toBeInTheDocument()
  })

  it('turns a leaked tool_call payload into a tool row and keeps the surrounding prose', () => {
    const { container } = renderTurn([
      {
        type: 'text',
        text: '结果如下。\n<tool_call>\nread path="src/a.ts"\n</tool_call>\n已经打开。'
      }
    ] as CherryMessagePart[])

    const row = container.querySelector('[data-task-tool-name="read"]')
    const label = row?.querySelector('[data-task-tool-label]')
    expect(label).toHaveTextContent(/Read src\/a\.ts|读取 src\/a\.ts/)
    const text = container.textContent ?? ''
    const title = label?.textContent ?? ''
    expect(text.indexOf('结果如下')).toBeLessThan(text.indexOf(title))
    expect(text.indexOf(title)).toBeLessThan(text.indexOf('已经打开'))
    expect(text).not.toContain('tool_call')
    expect(row).toHaveAttribute('data-task-tool-state', 'success')
    expect(row?.querySelector('.task-tool-row__summary')?.textContent).not.toContain('{')
  })

  it('turns a leaked DeepSeek DSML invoke into a bash row and keeps the thought', () => {
    const command = 'ls ~/.claude'
    const ns = '\uFF5CDSML\uFF5C'
    const { container } = renderTurn([
      {
        type: 'text',
        text: `Let me search more broadly.\n<${ns}calls><${ns}invoke name="bash"><${ns}parameter name="command" string="true">${command}</${ns}parameter></${ns}invoke></${ns}calls>`
      }
    ] as CherryMessagePart[])

    const row = container.querySelector('[data-task-tool-name="bash"]')
    expect(row?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Run ls ~\/\.claude|运行 ls ~\/\.claude/)
    const text = container.textContent ?? ''
    expect(text.indexOf('Let me search more broadly.')).toBeLessThan(text.indexOf('ls ~/.claude'))
    expect(text).not.toContain('DSML')
    expect(row).toHaveAttribute('data-task-tool-state', 'success')
  })

  it('turns a spaced double-bar DSML invoke into a bash row', () => {
    const bar = '\uFF5C\uFF5C'
    const command = 'ls "$d" 2>/dev/null | head -30'
    const { container } = renderTurn([
      {
        type: 'text',
        text: `<${bar}DSML${bar} calls>\n<${bar}DSML${bar} invoke name="bash">\n<${bar}DSML${bar} parameter name="command" string="true">${command}</${bar}DSML${bar} parameter>\n</${bar}DSML${bar} invoke>\n</${bar}DSML${bar} calls>`
      }
    ] as CherryMessagePart[])

    expect(container.querySelector('[data-task-tool-name="bash"] [data-task-tool-label]')).toHaveTextContent(
      /Run ls|运行 ls/
    )
    expect(container.textContent).not.toContain('DSML')
  })

  it('does not offer drive save for a recovered command with no product file', () => {
    const org = '0b9d2a4e-1c3f-4a5b-8d6e-7f8091a2b3c4'
    const bar = '\uFF5C\uFF5C'
    const command = 'ls "$d" 2>/dev/null | head -30'
    try {
      writeCenterId(org)
      const { container } = render(
        <div className="message">
          <div data-ui="chat.message-menu-bar.message-actions part:message-actions" />
          <WorkbenchProjectLockedContext value={true}>
            <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
              <MessagePartsScopeProvider
                messageId="turn-1"
                parts={
                  [
                    {
                      type: 'text',
                      text: `<${bar}DSML${bar} calls>\n<${bar}DSML${bar} invoke name="bash">\n<${bar}DSML${bar} parameter name="command" string="true">${command}</${bar}DSML${bar} parameter>\n</${bar}DSML${bar} invoke>\n</${bar}DSML${bar} calls>`
                    }
                  ] as CherryMessagePart[]
                }>
                <MessageContent message={assistant('success')} />
              </MessagePartsScopeProvider>
            </WorkbenchAssistantMessageContext>
          </WorkbenchProjectLockedContext>
        </div>
      )
      expect(container.querySelector('[data-ui="task.save-finding"]')).toBeNull()
      expect(container.querySelector('[data-ui~="part:message-actions"]')).not.toBeNull()
      expect(container.textContent).not.toContain('DSML')
    } finally {
      writeCenterId(PERSONAL_CENTER)
      localStorage.removeItem('a3s.org.center')
      document.cookie = 'a3s.org.center=; Path=/; Max-Age=0'
    }
  })

  it('turns a single-pipe DSML invoke into a bash row', () => {
    const { container } = renderTurn([
      {
        type: 'text',
        text: '<|DSML||calls><|DSML||invoke name="bash"><|DSML||parameter name="command" string="true">pwd</|DSML||parameter></|DSML||invoke></|DSML||calls>'
      }
    ] as CherryMessagePart[])

    expect(container.querySelector('[data-task-tool-name="bash"] [data-task-tool-label]')).toHaveTextContent(
      /Run pwd|运行 pwd/
    )
    expect(container.textContent).not.toContain('DSML')
  })

  it('lifts DSML out of reasoning and keeps the surrounding thought', () => {
    const ns = '\uFF5CDSML\uFF5C'
    const { container } = renderTurn([
      {
        type: 'reasoning',
        text: `Let me search more broadly.\n<${ns}invoke name="bash"><${ns}parameter name="command" string="true">pwd</${ns}parameter></${ns}invoke>`,
        state: 'done'
      }
    ] as CherryMessagePart[])

    const reasoning = container.querySelector('[data-ui~="part:message-reasoning"]')
    expect(reasoning?.querySelector('[data-reasoning-body]')).toBeNull()
    fireEvent.click(reasoning?.querySelector('button') as Element)
    expect(reasoning?.querySelector('[data-reasoning-body]')).toHaveTextContent('Let me search more broadly.')
    expect(reasoning?.textContent).not.toContain('DSML')
    expect(container.querySelector('[data-task-tool-name="bash"]')).not.toBeNull()
    expect(container.textContent?.indexOf('Let me search more broadly.')).toBeLessThan(
      container.textContent?.indexOf('pwd') ?? -1
    )
  })

  it('strips a leaked tool_call when the turn already has that structured tool', () => {
    const { container } = renderTurn([
      {
        type: 'text',
        text: '先看。\n<tool_call>\nread path="src/a.ts"\n</tool_call>\n后看。'
      },
      {
        type: 'tool-read',
        toolCallId: 'call-read',
        state: 'output-available',
        input: { path: 'src/a.ts' },
        output: 'ok'
      }
    ] as CherryMessagePart[])

    const rows = container.querySelectorAll('[data-task-tool-name="read"]')
    expect(rows).toHaveLength(1)
    expect(rows[0]?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Read src\/a\.ts|读取 src\/a\.ts/)
    expect(container.textContent).toContain('先看')
    expect(container.textContent).toContain('后看')
    expect(container.textContent).not.toContain('tool_call')
  })

  it('keeps a different leaked call beside a structured tool', () => {
    const { container } = renderTurn([
      {
        type: 'text',
        text: '先跑。\n<tool_call>\nread path="src/a.ts"\n</tool_call>\n再看。'
      },
      {
        type: 'tool-bash',
        toolCallId: 'call-ls',
        state: 'output-available',
        input: { command: 'ls' },
        output: { stdout: 'ok' }
      }
    ] as CherryMessagePart[])

    const read = container.querySelector('[data-task-tool-name="read"]')
    const bash = container.querySelector('[data-task-tool-name="bash"]')
    expect(read?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Read src\/a\.ts|读取 src\/a\.ts/)
    expect(bash?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Run ls|运行 ls/)
    expect(container.textContent).not.toContain('tool_call')
    expect(follows(read, bash)).toBe(true)
  })

  it('keeps a leaked read of another file beside a structured read', () => {
    const { container } = renderTurn([
      {
        type: 'text',
        text: '<tool_call>\nread path="src/a.ts"\n</tool_call>'
      },
      {
        type: 'tool-read',
        toolCallId: 'call-read-b',
        state: 'output-available',
        input: { path: 'src/b.ts' },
        output: 'ok'
      }
    ] as CherryMessagePart[])

    const rows = container.querySelectorAll('[data-task-tool-name="read"]')
    expect(rows).toHaveLength(2)
    const labels = [...rows].map((row) => row.querySelector('[data-task-tool-label]')?.textContent ?? '')
    expect(labels.some((label) => /src\/a\.ts/.test(label))).toBe(true)
    expect(labels.some((label) => /src\/b\.ts/.test(label))).toBe(true)
  })

  it('leaves a review fence as code until the skill surface is published', () => {
    resetPluginCatalogForTests()
    const { container } = renderTurn([
      {
        type: 'text',
        text: [
          '```a3s-review-finding',
          '{"id":"f1","title":"自动作废","evidenceQuote":"甲方应在三日内付款。","summary":"解除过重","guidance":"改为书面通知"}',
          '```'
        ].join('\n')
      }
    ] as CherryMessagePart[])

    expect(container.querySelector('[data-chat-review-findings="true"]')).toBeNull()
    expect(container.querySelector('[data-streamdown="code-block"]')).not.toBeNull()
  })

  it('keeps manual review protocol collapsed without claiming chat findings were placed', () => {
    setPluginCatalogForTests({
      status: 'ready',
      plugins: [{ name: 'review', description: '', source: 'installed' }],
      directory: null
    })
    const { container } = renderTurn([
      {
        type: 'text',
        text: [
          '```a3s-review-finding',
          '{"id":"f1","title":"自动作废","evidenceQuote":"甲方应在三日内付款。","summary":"解除过重","guidance":"改为书面通知"}',
          '```'
        ].join('\n')
      }
    ] as CherryMessagePart[])

    expect(container.querySelector('[data-chat-review-findings]')).toBeNull()
    const protocol = container.querySelector('details[data-review-protocol]')
    expect(protocol).not.toBeNull()
    expect(protocol).not.toHaveAttribute('open')
    expect(protocol).toHaveTextContent('a3s-review-finding')
    resetPluginCatalogForTests()
  })

  it('leaves a JSON spec in the tool result as text', () => {
    const card = {
      root: 'summary',
      values: { status: 'Ready', note: 'Bring the brief' },
      elements: {
        summary: {
          type: 'Card',
          props: {
            title: 'Summary',
            items: [
              { label: 'Status', content: { $ref: '/values/status' } },
              { label: 'Note', content: { $ref: '/values/note' } }
            ]
          }
        }
      }
    }
    const { container } = renderTurn([
      {
        type: 'tool-record',
        toolCallId: 'record-1',
        state: 'output-available',
        input: { text: 'show the record' },
        output: JSON.stringify({
          answer: `record\n\n\`\`\`json\n${JSON.stringify(card)}\n\`\`\`\n`
        })
      }
    ] as CherryMessagePart[])
    const row = container.querySelector('[data-task-tool-row]')
    expect(row).not.toHaveAttribute('data-open')
    expect(container.querySelector('[data-testid="task-message-openui"]')).toBeNull()
  })

  it('highlights a complete code fence with its language and a copy control', async () => {
    const { container } = renderTurn([
      { type: 'text', text: '看这段。\n\n```ts\nconst answer = 1\n```' }
    ] as CherryMessagePart[])

    await waitFor(() => {
      expect(container.querySelector('[data-task-code="highlighted"]')).not.toBeNull()
    })
    const block = container.querySelector('[data-task-code="highlighted"]') as HTMLElement
    const prose = container.querySelector('[data-task-prose]') as HTMLElement
    expect(block).toHaveAttribute('data-streamdown', 'code-block')
    expect(block).toHaveAttribute('data-chat-code', 'shiki')
    expect(getComputedStyle(prose).maxWidth).toBe('100%')
    expect(getComputedStyle(block).maxWidth).toBe('100%')
    expect(getComputedStyle(block).overflow).toBe('hidden')
    expect(block.querySelector('[data-streamdown="code-block-header"]')).toHaveTextContent('ts')
    const copy = block.querySelector('[data-streamdown="code-block-copy-button"]') as HTMLElement
    expect(copy.tagName).toBe('SPAN')
    expect(copy.getAttribute('role')).toBeNull()
    expect(copy).toHaveAttribute('aria-label', expect.stringMatching(/Copy code|复制代码/))
    expect(getComputedStyle(copy).width).toBe('24px')
    expect(getComputedStyle(copy).height).toBe('24px')
    expect(getComputedStyle(copy).borderRadius).toBe('6px')
    expect(copy.querySelector('svg')).toHaveAttribute('width', '14')
    expect(copy.querySelector('svg')).toHaveAttribute('stroke-width', '1.7')
    expect(container.querySelector('[data-language="ts"]')).not.toBeNull()
    expect(container).toHaveTextContent('const answer = 1')
  })

  it('omits an empty closed code fence', () => {
    const { container } = renderTurn([
      { type: 'text', text: '开头。\n\n```ts\n\n```\n\n结尾。' }
    ] as CherryMessagePart[])

    expect(container.querySelector('[data-streamdown="code-block"]')).toBeNull()
    expect(container.querySelector('[data-task-code]')).toBeNull()
    expect(container).toHaveTextContent('开头。')
    expect(container).toHaveTextContent('结尾。')
  })

  it('keeps an incomplete code fence as source', () => {
    for (const status of ['pending', 'success'] as const) {
      const { container, unmount } = renderTurn(
        [{ type: 'text', text: '```ts\nconst answer = 1', state: 'streaming' }] as CherryMessagePart[],
        status
      )

      expect(container.querySelector('[data-task-fence="source"]')).toHaveTextContent('const answer = 1')
      expect(container.querySelector('[data-task-fence="source"]')).toHaveAttribute('data-streamdown', 'code-block')
      expect(container.querySelector('[data-task-code="highlighted"]')).toBeNull()
      expect(container.querySelector('[data-task-code="pending"]')).toBeNull()
      unmount()
    }
  })

  it('renders a complete mermaid fence as a diagram', async () => {
    const { container } = renderTurn([
      { type: 'text', text: '```mermaid\ngraph TD\nA-->B\n```' }
    ] as CherryMessagePart[])

    expect(await screen.findByTestId('task-mermaid-svg')).toBeInTheDocument()
    const diagram = container.querySelector('[data-task-diagram="rendered"]')
    expect(diagram).toHaveAttribute('data-chat-diagram', 'rendered')
    expect(diagram).toHaveAttribute('data-chat-diagram-engine', 'mermaid')
    expect(diagram?.querySelector('[data-task-diagram-label]')).toHaveTextContent(/Flowchart|流程图/)
    expect(getComputedStyle(diagram as HTMLElement).maxWidth).toBe('100%')
    expect(mermaidRender).toHaveBeenCalled()
    const source = String(mermaidRender.mock.calls.at(-1)?.[1] ?? '')
    expect(source).toContain('graph TD')
    expect(source).toContain('A-->B')
  })

  it('renders a glued mermaidflowchart header as a diagram', async () => {
    const { container } = renderTurn([
      { type: 'text', text: '```mermaidflowchart TD\nA-->B\n```' }
    ] as CherryMessagePart[])

    expect(await screen.findByTestId('task-mermaid-svg')).toBeInTheDocument()
    const source = String(mermaidRender.mock.calls.at(-1)?.[1] ?? '')
    expect(source).toContain('flowchart TD')
    expect(source).toContain('A-->B')
    expect(container.querySelector('[data-language="mermaidflowchart"]')).toBeNull()
    expect(container.textContent).not.toContain('mermaidflowchart')
  })

  it('leaves an open mermaid fence as source while it is still streaming', async () => {
    const lone = renderTurn([{ type: 'text', text: '```mermaid\ngraph TD\nA-->B' }] as CherryMessagePart[], 'success')
    expect(lone.container.querySelector('[data-task-diagram="source"]')).toHaveTextContent('A-->B')
    expect(lone.container.querySelector('[data-task-diagram="rendered"]')).toBeNull()
    expect(lone.container.textContent).not.toMatch(/Mermaid Error|Syntax error/i)
    expect(mermaidRender).not.toHaveBeenCalled()
    lone.unmount()

    for (const status of ['pending', 'success'] as const) {
      mermaidRender.mockClear()
      const { container, unmount } = renderTurn(
        [
          {
            type: 'text',
            text: '```ts\nconst answer = 1\n```\n\n```mermaid\ngraph TD\nA-->B\n```\n\n```mermaid\ngraph TD\nC-->D'
          }
        ] as CherryMessagePart[],
        status
      )

      await waitFor(() => {
        expect(container.querySelector('[data-task-code="highlighted"]')).not.toBeNull()
        expect(container.querySelector('[data-task-diagram="rendered"]')).not.toBeNull()
      })
      const open = container.querySelector('[data-task-diagram="source"]')
      expect(open).toHaveAttribute('data-chat-diagram', 'source')
      expect(open).toHaveTextContent('C-->D')
      expect(open).not.toHaveTextContent('A-->B')
      expect(container.querySelectorAll('[data-task-diagram="rendered"]')).toHaveLength(1)
      expect(container.textContent).not.toMatch(/Mermaid Error|Syntax error/i)
      const rendered = String(mermaidRender.mock.calls.at(-1)?.[1] ?? '')
      expect(rendered).toContain('A-->B')
      expect(rendered).not.toContain('C-->D')
      unmount()
    }
  })

  it('keeps a failed mermaid diagram as source with the failure note', async () => {
    mermaidRender.mockImplementation(async () => {
      throw new Error('bad diagram')
    })
    try {
      const { container } = renderTurn([
        { type: 'text', text: '```mermaid\ngraph TD\nA-->B\n```' }
      ] as CherryMessagePart[])

      await waitFor(() => {
        expect(container.querySelector('[data-chat-diagram="failed"]')).not.toBeNull()
      })
      const failed = container.querySelector('[data-chat-diagram="failed"]')
      expect(failed).toHaveAttribute('data-task-diagram', 'source')
      expect(failed?.querySelector('pre code')).toHaveTextContent('A-->B')
      expect(failed).toHaveTextContent(/图表未能渲染|The diagram could not be rendered/)
      expect(container.querySelector('[data-chat-diagram-canvas]')).toBeNull()
    } finally {
      mermaidRender.mockImplementation(async () => ({ svg: '<svg data-testid="task-mermaid-svg"></svg>' }))
    }
  })

  it('keeps reasoning and an image in arrival order and drops raw citation markers', async () => {
    const { container } = renderTurn([
      { type: 'text', text: '前文。' },
      { type: 'reasoning', text: '先想清楚 [cite:ghost]', state: 'done' },
      { type: 'source-url', sourceId: 'citation-0', url: 'https://example.com/source', title: 'Source' },
      { type: 'file', mediaType: 'image/png', url: 'https://example.com/diagram.png' },
      {
        type: 'text',
        text: '答案在这 [cite:1]，旁注 [cite:ghost]。\n\n```ts\nconst marker = "[cite:keep]"\n```'
      }
    ] as CherryMessagePart[])

    const html = container.innerHTML
    const before = html.indexOf('前文')
    const reasoningAt = html.indexOf('part:message-reasoning')
    const image = html.indexOf('https://example.com/diagram.png')
    const answer = html.indexOf('答案在这')
    expect(before).toBeGreaterThanOrEqual(0)
    expect(before).toBeLessThan(reasoningAt)
    expect(reasoningAt).toBeLessThan(image)
    expect(image).toBeLessThan(answer)
    const reasoning = container.querySelector('[data-ui~="part:message-reasoning"]')
    expect(reasoning).not.toBeNull()
    expect(reasoning).not.toHaveAttribute('open')
    expect(reasoning?.textContent).toMatch(/思考过程|Thought/)
    expect(reasoning).not.toHaveAttribute('data-reasoning-preview')
    expect(reasoning).not.toHaveAttribute('data-open')
    expect(reasoning?.querySelector('[data-reasoning-body]')).toBeNull()
    expect(reasoning?.querySelector('[data-task-disclosure]')).toBeNull()
    fireEvent.click(reasoning?.querySelector('button') as Element)
    expect(reasoning?.querySelector('[data-reasoning-body]')).toHaveTextContent('先想清楚')
    expect(reasoning?.querySelector('[data-reasoning-body]')?.textContent).not.toContain('[cite:ghost]')
    expect(reasoning?.querySelector('[data-reasoning-content]')).not.toHaveAttribute('hidden')
    const shown = container.textContent ?? ''
    expect(shown).not.toContain('已深度思考')
    expect(container.querySelector('img')).toHaveAttribute('src', 'https://example.com/diagram.png')
    expect(shown).not.toContain('[cite:ghost]')
    expect(shown).not.toContain('[cite:1]')
    expect(shown).toContain('[cite:keep]')
    await waitFor(() => {
      expect(container.querySelector('[data-task-code="highlighted"]')).not.toBeNull()
      expect(container.querySelector('sup')).toHaveTextContent('1')
    })
  })

  it('peeks the newest reasoning lines while a turn is streaming', () => {
    const lines = Array.from({ length: 8 }, (_, index) => `step-${index + 1}`).join('\n')
    const streamingParts = [{ type: 'reasoning', text: lines, state: 'streaming' }] as CherryMessagePart[]
    const view = renderTurn(streamingParts, 'pending')
    const reasoning = () => view.container.querySelector('[data-ui~="part:message-reasoning"]')
    const body = () => reasoning()?.querySelector('[data-reasoning-body]')

    expect(reasoning()).toHaveAttribute('data-reasoning-preview', 'true')
    expect(reasoning()).not.toHaveAttribute('data-open')
    expect(reasoning()?.querySelector('button')).toHaveAttribute('aria-expanded', 'false')
    expect(body()).toHaveTextContent('step-4')
    expect(body()?.textContent).toBe('step-4\nstep-5\nstep-6\nstep-7\nstep-8')
    expect(body()?.textContent).not.toContain('step-1')
    expect(reasoning()?.querySelector('[data-reasoning-truncated]')).not.toBeNull()
    expect(getComputedStyle(body() as Element).fontSize).toBe('12px')
    expect(getComputedStyle(body() as Element).opacity).toBe('0.72')
    expect(getComputedStyle(reasoning()?.querySelector('[data-reasoning-content]') as Element).maxHeight).toBe(
      'calc(12px * 1.35 * 5)'
    )
    expect(getComputedStyle(reasoning()?.querySelector('[data-reasoning-content]') as Element).overflow).toBe('auto')
    expect(getComputedStyle(body() as Element).color).toBe('var(--task-reasoning)')
    expect(getComputedStyle(body() as Element).color).not.toBe('rgb(63, 63, 63)')

    fireEvent.click(reasoning()?.querySelector('button') as Element)
    expect(reasoning()).toHaveAttribute('data-open', 'true')
    expect(reasoning()).not.toHaveAttribute('data-reasoning-preview')
    expect(reasoning()?.querySelector('[data-reasoning-summary]')).toBeNull()
    expect(reasoning()?.querySelectorAll('[data-reasoning-body]')).toHaveLength(1)
    expect(body()?.textContent).toBe(lines)
    const full = reasoning()?.querySelector('[data-reasoning-full]')
    expect(full).not.toBeNull()
    expect(getComputedStyle(full as Element).maxHeight).toBe('min(18rem, 40vh)')
    expect(getComputedStyle(full as Element).overflow).toBe('auto')
    expect(getComputedStyle(full as Element).backgroundColor).toBe('rgb(247, 247, 247)')
    expect(getComputedStyle(full as Element).borderRadius).toBe('8px')
    expect(getComputedStyle(body() as Element).fontSize).toBe('13px')
    expect(getComputedStyle(body() as Element).lineHeight).toBe('20px')
    expect(getComputedStyle(body() as Element).opacity).toBe('1')
    expect(getComputedStyle(body() as Element).color).toBe('rgb(92, 92, 92)')

    view.rerender(
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
          <MessagePartsScopeProvider messageId="turn-1" parts={streamingParts}>
            <MessageContent message={assistant('pending')} />
          </MessagePartsScopeProvider>
        </WorkbenchAssistantMessageContext>
      </WorkbenchProjectLockedContext>
    )
    expect(reasoning()).toHaveAttribute('data-open', 'true')
    expect(body()?.textContent).toContain('step-1')
  })

  it('keeps the last reasoning lines in a fixed window after the turn settles', () => {
    const lines = Array.from({ length: 8 }, (_, index) => `step-${index + 1}`)
    const text = lines.join('\n')
    const view = renderTurn([{ type: 'reasoning', text, state: 'streaming' }] as CherryMessagePart[], 'pending')
    expect(view.container.querySelector('[data-reasoning-preview]')).toHaveAttribute('data-reasoning-preview', 'true')

    view.rerender(
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
          <MessagePartsScopeProvider
            messageId="turn-1"
            parts={[{ type: 'reasoning', text, state: 'done' }] as CherryMessagePart[]}>
            <MessageContent message={assistant('success')} />
          </MessagePartsScopeProvider>
        </WorkbenchAssistantMessageContext>
      </WorkbenchProjectLockedContext>
    )

    const reasoning = view.container.querySelector('[data-ui~="part:message-reasoning"]')
    expect(reasoning).not.toHaveAttribute('data-open')
    expect(reasoning).not.toHaveAttribute('data-reasoning-preview')
    expect(reasoning?.querySelector('button')).toHaveAttribute('aria-expanded', 'false')
    expect(reasoning?.querySelector('[data-reasoning-body]')).toBeNull()
    expect(reasoning?.querySelector('[data-reasoning-content]')).toBeNull()
    expect(reasoning?.querySelector('[data-reasoning-full]')).toBeNull()
    expect(reasoning?.querySelector('[data-task-disclosure]')).toBeNull()
    expect(reasoning?.textContent).toMatch(/思考过程|Thought/)
    expect(reasoning?.textContent).not.toContain('step-1')
    expect(reasoning?.textContent).not.toContain('step-8')

    fireEvent.click(reasoning?.querySelector('button') as Element)
    expect(reasoning).toHaveAttribute('data-open', 'true')
    expect(reasoning).not.toHaveAttribute('data-reasoning-preview')
    expect(reasoning?.querySelector('[data-reasoning-summary]')).toBeNull()
    expect(reasoning?.querySelectorAll('[data-reasoning-body]')).toHaveLength(1)
    expect(reasoning?.querySelector('[data-reasoning-body]')?.textContent).toBe(text)
    expect(getComputedStyle(reasoning?.querySelector('[data-reasoning-full]') as Element).maxHeight).toBe(
      'min(18rem, 40vh)'
    )

    fireEvent.click(reasoning?.querySelector('button') as Element)
    expect(reasoning).not.toHaveAttribute('data-open')
    expect(reasoning).not.toHaveAttribute('data-reasoning-preview')
    expect(reasoning?.querySelector('[data-reasoning-full]')).toBeNull()
    expect(reasoning?.querySelector('[data-reasoning-body]')).toBeNull()
  })

  it('shows compact input and output for a tool call', () => {
    const { container } = renderTurn([
      {
        type: 'dynamic-tool',
        toolName: 'lookup_docs',
        toolCallId: 'lookup-1',
        state: 'output-available',
        input: { query: 'acl', limit: 2 },
        output: { title: 'ACL guide', note: 'use a3s-acl' }
      }
    ] as CherryMessagePart[])

    const row = container.querySelector('[data-task-tool-name="lookup_docs"]')
    expect(row?.querySelector('.task-tool-row__summary')?.textContent).not.toContain('a3s-acl')
    expect(row?.querySelector('.task-tool-row__summary')?.textContent).not.toContain('{')
    fireEvent.click(row?.querySelector('summary') as Element)
    const input = container.querySelector('[data-tool-payload="input"]')
    const output = container.querySelector('[data-tool-payload="output"]')
    expect(input?.querySelector('h4')).toHaveTextContent(/输入|Input/)
    expect(output?.querySelector('h4')).toHaveTextContent(/输出|Output/)
    expect(input?.querySelector('pre')).toHaveTextContent(/"query": "acl"/)
    expect(output?.querySelector('pre')).toHaveTextContent('a3s-acl')
    expect(getComputedStyle(input?.querySelector('h4') as Element).fontSize).toBe('12px')
    expect(getComputedStyle(input?.querySelector('h4') as Element).color).toBe('rgb(138, 138, 138)')
    expect(getComputedStyle(input?.querySelector('pre') as Element).maxHeight).toBe('calc(20px * 6)')
  })

  it('shows workspace product chips after a finished turn that wrote files', () => {
    const { container } = renderTurn([
      { type: 'text', text: '先写文件。' },
      {
        type: 'dynamic-tool',
        toolName: 'edit',
        toolCallId: 't-write',
        state: 'output-available',
        input: {},
        output: '',
        files: ['/workspace/notes.md']
      },
      { type: 'text', text: '写好了。' }
    ] as CherryMessagePart[])

    const proses = container.querySelectorAll('[data-task-prose]')
    const tool = container.querySelector('[data-task-tool-name="edit"]')
    expect(proses[0]?.textContent).toContain('先写文件')
    expect(proses[1]?.textContent).toContain('写好了')
    expect(follows(proses[0], tool)).toBe(true)
    expect(follows(tool, proses[1])).toBe(true)
    expect(tool?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Edit notes\.md|修改 notes\.md/)
    const chip = container.querySelector('[data-task-product]')
    expect(chip).toHaveAttribute('data-task-product-path', '/workspace/notes.md')
    expect(chip).toHaveTextContent('notes.md')
    expect(chip).toHaveAttribute('data-task-product-kind', 'document')
    expect(chip?.querySelector('[data-task-product-kind-label]')).toBeNull()
    expect(chip).not.toHaveTextContent(/文档|Document/)
    expect(chip).toHaveClass('task-product-row')
    expect(chip?.querySelector('svg')).toHaveAttribute('width', '16')
    expect(chip?.querySelector('svg')).toHaveAttribute('height', '16')
    expect(chip?.querySelector('svg')).toHaveAttribute('stroke-width', '1.7')
    expect(container.querySelector('[data-task-products]')).toHaveTextContent(/工作产物 · 1|Work products · 1/)
    expect(container.querySelector('[data-task-products-more]')).toBeNull()
    expect(container.querySelector('.task-products-list')).not.toBeNull()
    expect(chip).toHaveAttribute('aria-label', expect.stringMatching(/打开 notes\.md|Open notes\.md/))
    fireEvent.click(chip as Element)
    expect(openArtifactFile).toHaveBeenCalledWith('/workspace/notes.md')
  })

  it('never certifies a path or ordinary reply as host review or provenance', () => {
    localStorage.setItem(
      'workbench.turn-arms.v1',
      JSON.stringify({ pending: [{ at: 0, reviewMode: true, artifactTrace: true }], bound: {} })
    )
    const { container } = renderTurn([
      { type: 'text', text: 'Created /workspace/notes.md' },
      {
        type: 'dynamic-tool',
        toolName: 'edit',
        toolCallId: 't-write',
        state: 'output-available',
        input: { file_path: '/workspace/notes.md' },
        output: '',
        files: ['/workspace/notes.md']
      }
    ] as CherryMessagePart[])
    expect(container.querySelector('[data-task-auto-review]')).toBeNull()
    expect(container.querySelector('[data-task-lineage-open]')).toBeNull()
    localStorage.removeItem('workbench.turn-arms.v1')
  })

  it('reads a diff path on the settled tool when the files list is absent', () => {
    const { container } = renderTurn([
      {
        type: 'dynamic-tool',
        toolName: 'edit',
        toolCallId: 't-write',
        state: 'output-available',
        input: {},
        output: { content: [{ type: 'diff', path: 'C:\\workspace\\notes.md' }] }
      }
    ] as CherryMessagePart[])

    const chip = container.querySelector('[data-task-product]')
    expect(chip).toHaveAttribute('data-task-product-path', 'C:\\workspace\\notes.md')
    expect(chip).toHaveTextContent('notes.md')
  })

  it('hides workspace products while the turn is still running', () => {
    const { container } = renderTurn(
      [
        {
          type: 'dynamic-tool',
          toolName: 'edit',
          toolCallId: 't-write',
          state: 'output-available',
          input: {},
          files: ['/workspace/notes.md']
        }
      ] as CherryMessagePart[],
      'pending'
    )

    expect(container.querySelector('[data-task-product]')).toBeNull()
    expect(container.querySelector('[data-task-products]')).toBeNull()
  })

  it('shows a product for a finished Write or Edit that names file_path or filePath', () => {
    const wrote = renderTurn([
      {
        type: 'dynamic-tool',
        toolName: 'Write',
        toolCallId: 'w1',
        state: 'output-available',
        input: { file_path: '/workspace/notes.md', content: 'hello' },
        output: 'ok'
      },
      {
        type: 'dynamic-tool',
        toolName: 'Read',
        toolCallId: 'r1',
        state: 'output-available',
        input: { file_path: '/workspace/secret.md' },
        output: { filePath: '/workspace/secret.md' }
      },
      {
        type: 'tool-Edit',
        toolCallId: 'e1',
        state: 'output-available',
        input: { old_string: 'a', new_string: 'b' },
        output: { filePath: 'src/app.ts' }
      }
    ] as CherryMessagePart[])
    const chips = [...wrote.container.querySelectorAll('[data-task-product]')]
    expect(chips.map((chip) => chip.getAttribute('data-task-product-path'))).toEqual([
      '/workspace/notes.md',
      'src/app.ts'
    ])
    expect(chips[0]).toHaveTextContent('notes.md')
    expect(chips[1]).toHaveTextContent('app.ts')
    expect(wrote.container.querySelector('[data-task-product-path="/workspace/secret.md"]')).toBeNull()
    wrote.unmount()

    const edited = renderTurn([
      {
        type: 'tool-Edit',
        toolCallId: 'e1',
        state: 'output-available',
        input: { old_string: 'a', new_string: 'b' },
        output: { filePath: 'src/app.ts' }
      }
    ] as CherryMessagePart[])
    expect(edited.container.querySelector('[data-task-product]')).toHaveAttribute(
      'data-task-product-path',
      'src/app.ts'
    )
    expect(edited.container.querySelector('[data-task-product]')).toHaveTextContent('app.ts')
    edited.unmount()

    const streaming = renderTurn(
      [
        {
          type: 'dynamic-tool',
          toolName: 'Write',
          toolCallId: 'w2',
          state: 'output-available',
          input: { file_path: '/workspace/notes.md' },
          output: 'ok'
        }
      ] as CherryMessagePart[],
      'pending'
    )
    expect(streaming.container.querySelector('[data-task-product]')).toBeNull()
    streaming.unmount()

    const failed = renderTurn([
      {
        type: 'dynamic-tool',
        toolName: 'Edit',
        toolCallId: 'e2',
        state: 'output-error',
        input: { file_path: '/workspace/notes.md' },
        output: { filePath: '/workspace/notes.md' },
        errorText: 'disk full'
      }
    ] as CherryMessagePart[])
    expect(failed.container.querySelector('[data-task-product]')).toBeNull()
  })

  it('shows no product row when the turn wrote nothing', () => {
    const { container } = renderTurn([
      { type: 'text', text: '没有文件。' },
      {
        type: 'tool-read',
        toolCallId: 'call-read',
        state: 'output-available',
        input: { path: '/workspace/notes.md' },
        output: 'hello'
      }
    ] as CherryMessagePart[])

    expect(container.querySelector('[data-task-products]')).toBeNull()
    expect(container.querySelector('[data-task-product]')).toBeNull()
  })

  it('scrolls product rows in a three-row pane and saves each file from its row', () => {
    const org = '0b9d2a4e-1c3f-4a5b-8d6e-7f8091a2b3c4'
    const files = ['a.json', 'b.json', 'c.json', 'd.json'].map((name) => `/workspace/${name}`)
    try {
      writeCenterId(org)
      const { container } = renderTurn([
        {
          type: 'dynamic-tool',
          toolName: 'edit',
          toolCallId: 't-write',
          state: 'output-available',
          input: {},
          output: '',
          files
        }
      ] as CherryMessagePart[])

      const items = [...container.querySelectorAll('.task-products-list > li')]
      expect(items).toHaveLength(4)
      expect(items.every((item) => !item.hasAttribute('hidden'))).toBe(true)
      expect(container.querySelector('[data-task-product-kind-label]')).toBeNull()
      expect(container.querySelector('[data-task-products-more]')).toBeNull()
      expect(container.querySelector('[data-task-products]')).toHaveTextContent(/工作产物 · 4|Work products · 4/)
      expect(container.textContent).not.toContain('代码')
      const saves = [...container.querySelectorAll('[data-ui="task.save-finding"]')]
      expect(saves).toHaveLength(4)
      expect(saves.every((save) => save.closest('.task-product-item')?.querySelector('[data-task-product]'))).toBe(true)
      expect(container.querySelector('header [data-ui="task.save-finding"]')).toBeNull()
      expect(taskConversationCss).toMatch(/\.task-products-list \{[\s\S]*?max-height:\s*calc\(36px \* 3 \+ 2px \* 2\)/)
      expect(taskConversationCss).toMatch(/\.task-products-list \{[\s\S]*?overflow-y:\s*auto/)
      expect(taskConversationCss).toMatch(
        /\.task-product-item:hover \[data-ui='task\.save-finding'\][\s\S]*?opacity:\s*1/
      )
    } finally {
      writeCenterId(PERSONAL_CENTER)
      localStorage.removeItem('a3s.org.center')
      document.cookie = 'a3s.org.center=; Path=/; Max-Age=0'
    }
  })

  it('does not show a product for a failed edit', () => {
    const { container } = renderTurn([
      {
        type: 'dynamic-tool',
        toolName: 'edit',
        toolCallId: 't-write',
        state: 'output-error',
        input: {},
        errorText: 'disk full',
        files: ['/workspace/notes.md']
      }
    ] as CherryMessagePart[])

    expect(container.querySelector('[data-task-product]')).toBeNull()
  })

  it('renders an announced local HTML page as a web-page product', () => {
    const path = '/private/var/folders/T/a3s-sandbox-test/.answer-me-with-html/pages/什么是智能体-20261007-145559.html'
    const { container } = renderTurn([
      { type: 'text', text: `页面： ${path}` },
      {
        type: 'dynamic-tool',
        toolName: 'bash',
        toolCallId: 't-page',
        state: 'output-available',
        input: { command: `node am.mjs --out ${path}` },
        output: path
      }
    ] as CherryMessagePart[])

    const prose = container.querySelector('[data-task-prose]')
    expect(prose?.textContent).toContain('什么是智能体-20261007-145559.html')
    expect(prose?.textContent).not.toContain('/private/var/folders')
    const chip = container.querySelector('[data-task-product]')
    expect(container.querySelectorAll('[data-task-product]')).toHaveLength(1)
    expect(chip).toHaveAttribute('data-task-product-kind', 'page')
    expect(chip).toHaveAttribute('data-task-product-path', path)
    expect(chip?.querySelector('[data-task-product-kind-label]')).toBeNull()
    expect(chip).not.toHaveTextContent(/网页|Web page/)
    expect(getComputedStyle(chip as Element).minHeight).toBe('44px')
    expect(getComputedStyle(chip as Element).backgroundColor).toBe('rgb(250, 250, 250)')
    fireEvent.click(chip as Element)
    expect(openArtifactFile).toHaveBeenCalledWith(path)
    expect(container.querySelector('[data-task-prose] code')).toBeNull()
  })

  it('keeps a path inside a code fence and ignores a read tool path', () => {
    const fenced = '/workspace/example.html'
    const { container } = renderTurn([
      { type: 'text', text: `例子：\n\`\`\`\n${fenced}\n\`\`\`` },
      {
        type: 'dynamic-tool',
        toolName: 'Read',
        toolCallId: 't-read',
        state: 'output-available',
        input: { file_path: '/workspace/secret.md' },
        output: 'see /workspace/secret.md'
      }
    ] as CherryMessagePart[])

    expect(container.querySelector('[data-task-prose]')?.textContent).toContain(fenced)
    expect(container.querySelector('[data-task-product]')).toBeNull()
  })

  it('opens an edit as a diff and can switch to the split view', () => {
    const { container } = renderTurn([
      {
        type: 'tool-edit',
        toolCallId: 'call-edit',
        state: 'output-available',
        input: { path: 'src/a.ts', old_string: 'const a = 1', new_string: 'const a = 2' },
        output: 'ok'
      }
    ] as CherryMessagePart[])

    const row = container.querySelector('[data-task-tool-name="edit"]')
    expect(row?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Edit a\.ts|修改 a\.ts/)
    expect(row?.textContent).toContain('+1 · \u22121')
    const diff = container.querySelector('.task-code-diff') as HTMLElement
    expect(getComputedStyle(diff).maxWidth).toBe('100%')
    expect(getComputedStyle(diff).overflow).toBe('hidden')
    fireEvent.click(row?.querySelector('summary') as Element)
    expect(container.querySelector('[data-kind="deletion"] code')).toHaveTextContent('const a = 1')
    expect(container.querySelector('[data-kind="addition"] code')).toHaveTextContent('const a = 2')
    expect(container.querySelector('[data-kind="deletion"] [data-code-diff-token]')).toHaveTextContent('1')
    expect(container.querySelector('[data-kind="addition"] [data-code-diff-token]')).toHaveTextContent('2')
    expect(container.querySelector('[data-code-diff-additions]')).toHaveTextContent('+1')
    expect(container.querySelector('[data-code-diff-deletions]')).toHaveTextContent('−1')
    fireEvent.click(screen.getByRole('button', { name: /Split|分栏/ }))
    expect(container.querySelector('[data-code-diff-mode="split"]')).not.toBeNull()
    expect(container.querySelector('[data-code-diff-side="old"][data-kind="deletion"]')).toHaveTextContent(
      'const a = 1'
    )
    expect(container.querySelector('[data-code-diff-side="new"][data-kind="addition"]')).toHaveTextContent(
      'const a = 2'
    )
    const segments = container.querySelector('[role="radiogroup"]') as HTMLElement
    expect(segments).toHaveAttribute('aria-label', expect.stringMatching(/Diff view|差异视图/))
    expect(getComputedStyle(segments).backgroundColor).toBe('rgb(245, 245, 245)')
    expect(getComputedStyle(segments).borderRadius).toBe('8px')
    expect(getComputedStyle(screen.getByRole('button', { name: /Split|分栏/ })).fontSize).toBe('13px')
    fireEvent.click(container.querySelector('[data-code-diff-open-file]') as Element)
    expect(openArtifactFile).toHaveBeenCalledWith('src/a.ts')
  })

  it('names a path-only edit and does not invent hunks', () => {
    const { container } = renderTurn([
      {
        type: 'tool-edit',
        toolCallId: 'call-edit',
        state: 'output-available',
        input: { path: 'src/a.ts' },
        output: 'ok'
      }
    ] as CherryMessagePart[])

    const row = container.querySelector('[data-task-tool-name="edit"]')
    expect(row?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Edit a\.ts|修改 a\.ts/)
    expect(row?.querySelector('small')).toHaveTextContent(/差异待生成|Diff pending/)
    expect(row?.textContent).not.toMatch(/\+\d+ · −\d+/)
    fireEvent.click(row?.querySelector('summary') as Element)
    expect(container.querySelector('[data-code-diff-status="pending"]')).not.toBeNull()
    expect(container.querySelector('[data-kind="addition"]')).toBeNull()
    expect(container.querySelector('[data-kind="deletion"]')).toBeNull()
    expect(container.querySelector('[role="radiogroup"]')).toBeNull()
    fireEvent.click(container.querySelector('[data-code-diff-open-file]') as Element)
    expect(openArtifactFile).toHaveBeenCalledWith('src/a.ts')
  })

  it('renders a unified patch payload as a diff card', () => {
    const { container } = renderTurn([
      {
        type: 'dynamic-tool',
        toolName: 'apply_patch',
        toolCallId: 'patch-1',
        state: 'output-available',
        input: {},
        output: ['diff --git a/n.ts b/n.ts', '--- a/n.ts', '+++ b/n.ts', '@@ -1 +1 @@', '-old', '+new'].join('\n')
      }
    ] as CherryMessagePart[])

    expect(container.querySelector('[data-task-tool-label]')).toHaveTextContent(/Apply patch · n\.ts|应用补丁 · n\.ts/)
    fireEvent.click(container.querySelector('summary') as Element)
    expect(container.querySelector('[data-code-diff-operation="patch"]')).not.toBeNull()
    expect(container.querySelector('[data-kind="addition"] code')).toHaveTextContent('new')
    expect(container.querySelector('[data-kind="deletion"] code')).toHaveTextContent('old')
  })

  it('opens a workspace path cited in prose and keeps web links external', () => {
    const { container } = renderTurn([
      { type: 'text', text: '看 `src/notes.md` 和 [说明](https://example.com/doc)。' }
    ] as CherryMessagePart[])

    const citation = container.querySelector('[data-workspace-path-citation="true"]')
    expect(citation).toHaveTextContent('src/notes.md')
    fireEvent.click(citation as Element)
    expect(openArtifactFile).toHaveBeenCalledWith('src/notes.md')
    const link = container.querySelector('a')
    expect(link).toHaveAttribute('href', 'https://example.com/doc')
    expect(link).toHaveAttribute('target', '_blank')
  })

  function TaskTurns({ tail }: { tail: string }) {
    const earlierParts = [
      { type: 'text', text: '先写文件。' },
      {
        type: 'tool-read',
        toolCallId: 'read-1',
        state: 'output-available',
        input: { path: 'a.ts' },
        output: 'const a = 1'
      },
      {
        type: 'tool-bash',
        toolCallId: 'bash-1',
        state: 'output-available',
        input: { command: 'ls' },
        output: { stdout: 'minutes.md\nextra.txt' }
      },
      {
        type: 'dynamic-tool',
        toolName: 'Write',
        toolCallId: 'write-1',
        state: 'output-available',
        input: { file_path: '/tmp/alpha/minutes.md', content: '纪要' },
        output: 'ok'
      }
    ] as CherryMessagePart[]
    const message = (id: string, role: 'user' | 'assistant', createdAt: string, status: MessageListItem['status']) =>
      ({ id, role, topicId: 'topic-1', createdAt, status }) as MessageListItem
    return (
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchAssistantMessageContext value={TaskConversationMessage}>
          <MessagePartsScopeProvider
            messageId="user-1"
            parts={[{ type: 'text', text: '起草一份纪要' }] as CherryMessagePart[]}>
            <MessageContent message={message('user-1', 'user', '2026-10-05T00:00:00.000Z', 'success')} />
          </MessagePartsScopeProvider>
          <MessagePartsScopeProvider messageId="assistant-1" parts={earlierParts}>
            <MessageContent message={message('assistant-1', 'assistant', '2026-10-05T00:00:01.000Z', 'success')} />
          </MessagePartsScopeProvider>
          <MessagePartsScopeProvider
            messageId="user-2"
            parts={[{ type: 'text', text: '再补一条决议' }] as CherryMessagePart[]}>
            <MessageContent message={message('user-2', 'user', '2026-10-05T00:00:02.000Z', 'success')} />
          </MessagePartsScopeProvider>
          <MessagePartsScopeProvider
            messageId="assistant-2"
            parts={[{ type: 'text', text: tail }] as CherryMessagePart[]}>
            <MessageContent message={message('assistant-2', 'assistant', '2026-10-05T00:00:03.000Z', 'pending')} />
          </MessagePartsScopeProvider>
        </WorkbenchAssistantMessageContext>
      </WorkbenchProjectLockedContext>
    )
  }

  it('shows the user text, then prose, one tool group, and a file result', () => {
    const view = render(<TaskTurns tail="好的" />)
    const prose = view.container.querySelector('[data-task-prose]')
    const group = view.container.querySelector('[data-task-tool-group]')
    const product = view.container.querySelector('[data-task-product]')
    const userNodes = [...view.container.querySelectorAll('*')].filter((node) => node.textContent === '起草一份纪要')
    const firstUserNode = userNodes.at(-1) ?? null

    expect(firstUserNode).toBeTruthy()
    expect(prose?.textContent).toContain('先写文件')
    expect(group).toHaveAttribute('data-task-tool-count', '3')
    expect(group?.querySelector('[data-task-tool-name="read"]')).toBeTruthy()
    expect(group?.querySelector('[data-task-tool-name="bash"]')).toBeTruthy()
    expect(product).toHaveAttribute('data-task-product-path', '/tmp/alpha/minutes.md')
    expect(follows(firstUserNode, prose)).toBe(true)
    expect(follows(prose, group)).toBe(true)
    expect(follows(group, product)).toBe(true)
    expect(view.container).toHaveTextContent('再补一条决议')
  })

  it('keeps completed tool groups collapsed until the user opens them', () => {
    const view = render(<TaskTurns tail="好的" />)
    const group = view.container.querySelector('[data-task-tool-group]')
    const bash = group?.querySelector('[data-task-tool-name="bash"]') as HTMLDetailsElement
    const read = group?.querySelector('[data-task-tool-name="read"]')

    expect(bash).not.toHaveAttribute('data-open')
    expect(read).not.toHaveAttribute('data-open')
    expect(bash.querySelector('summary')?.textContent).not.toContain('extra.txt')
    const disclosure = bash.querySelector('[data-task-disclosure]')
    expect(disclosure).not.toHaveAttribute('data-open')
    expect(getComputedStyle(disclosure as Element).gridTemplateRows).toMatch(/0fr/)
    bash.open = true
    act(() => {
      bash.dispatchEvent(new Event('toggle'))
    })
    expect(bash).toHaveAttribute('data-open', 'true')
    expect(disclosure).toHaveAttribute('data-open', 'true')
    expect(getComputedStyle(disclosure as Element).gridTemplateRows).toMatch(/1fr/)
    expect(bash.querySelector('[data-task-tool-result]')?.textContent).toContain('extra.txt')
    expect(read).not.toHaveAttribute('data-open')
  })

  it('keeps the first turn when a follow-up stays in the conversation', () => {
    const view = render(<TaskTurns tail="好的" />)
    const prose = view.container.querySelector('[data-task-prose]')
    const bash = view.container.querySelector('[data-task-tool-name="bash"]') as HTMLDetailsElement
    bash.open = true
    act(() => {
      bash.dispatchEvent(new Event('toggle'))
    })

    view.rerender(<TaskTurns tail="好的，已补" />)

    expect(view.container.querySelector('[data-task-prose]')).toBe(prose)
    expect(prose?.textContent).toContain('先写文件')
    expect(view.container.querySelector('[data-task-tool-name="bash"]')).toHaveAttribute('data-open', 'true')
    expect(view.container.querySelector('[data-task-tool-name="read"]')).not.toHaveAttribute('data-open')
    expect(view.container.querySelector('[data-task-product-path="/tmp/alpha/minutes.md"]')).toBeTruthy()
    expect(view.container).toHaveTextContent('起草一份纪要')
    expect(view.container).toHaveTextContent('再补一条决议')
    expect(view.container).toHaveTextContent('好的，已补')
  })

  it('shows command stdout as a terminal and only the first line on the chip', () => {
    const { container } = renderTurn([
      {
        type: 'tool-bash',
        toolCallId: 'call-ls',
        state: 'output-available',
        input: { command: 'ls' },
        output: { stdout: 'AGENTS.md\nREADME.md' }
      }
    ] as CherryMessagePart[])

    const row = container.querySelector('[data-task-tool-name="bash"]')
    expect(row?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Run ls|运行 ls/)
    expect(row?.querySelector('small')).toHaveTextContent('AGENTS.md')
    expect(row?.querySelector('small')).not.toHaveTextContent('README.md')
    expect(row?.querySelector('.task-tool-row__summary')?.textContent).not.toContain('{')
    fireEvent.click(row?.querySelector('summary') as Element)
    expect(container.querySelector('[data-tool-payload="input"] h4')).toHaveTextContent(/输入|Input/)
    expect(container.querySelector('[data-tool-payload="input"]')).toHaveTextContent('ls')
    expect(container.querySelector('[data-tool-payload="output"] h4')).toHaveTextContent(/输出|Output/)
    expect(container.querySelector('[data-task-tool-result]')?.textContent).toBe('AGENTS.md\nREADME.md')
    expect(container.textContent).not.toContain('查看原始输入')
    expect(container.textContent).not.toContain('查看原始输出')
    expect(container.querySelectorAll('details')).toHaveLength(1)
    expect(getComputedStyle(container.querySelector('[data-tool-payload="output"] pre') as Element).maxHeight).toBe(
      'calc(20px * 6)'
    )
  })

  it('decodes a command byte list on the first expand', () => {
    const output = [...'auto-ask\n'].map((char) => char.charCodeAt(0))
    const { container } = renderTurn([
      {
        type: 'tool-bash',
        toolCallId: 'call-echo',
        state: 'output-available',
        input: { command: 'echo auto-ask' },
        output: {
          command: 'echo auto-ask',
          current_dir: '',
          description: null,
          exit_code: 0,
          output
        }
      }
    ] as CherryMessagePart[])

    const row = container.querySelector('[data-task-tool-name="bash"]')
    expect(row?.querySelector('[data-task-tool-label]')).toHaveTextContent(/Run echo auto-ask|运行 echo auto-ask/)
    expect(row?.querySelector('small')).toHaveTextContent('auto-ask')
    expect(row?.querySelector('.task-tool-row__summary')?.textContent).not.toContain('97')
    fireEvent.click(row?.querySelector('summary') as Element)
    expect(container.querySelector('[data-tool-payload="input"]')).toHaveTextContent('echo auto-ask')
    expect(container.querySelector('[data-task-tool-result]')?.textContent).toBe('auto-ask')
    expect(container.textContent).not.toContain('查看原始输入')
    expect(container.textContent).not.toContain('查看原始输出')
    expect(container.textContent).not.toContain('"exit_code"')
    expect(container.querySelectorAll('details')).toHaveLength(1)
  })

  it('renders an ls directory as list rows with 16px kind icons', () => {
    const listing = [
      'Directory: /Users/roylin/code/a3s/packages',
      '',
      'dir         128B  ce/',
      'file        672B  readme.md',
      'link          0B  current',
      'unknown       4B  mystery',
      '',
      '4 of 9 entries',
      'More entries available; continue with cursor=page-2'
    ].join('\n')
    const { container } = renderTurn([
      {
        type: 'tool-ls',
        toolCallId: 'call-dir',
        state: 'output-available',
        input: { path: 'packages' },
        output: { stdout: listing }
      }
    ] as CherryMessagePart[])

    fireEvent.click(container.querySelector('summary') as Element)
    const card = container.querySelector('[data-task-directory]')
    expect(card?.querySelector('.task-dir__path')).toHaveTextContent('/Users/roylin/code/a3s/packages')
    expect(container.querySelector('[data-task-tool-result]')).toBeNull()
    const rows = [...(card?.querySelectorAll('.task-dir__row') ?? [])]
    expect(rows.map((row) => row.getAttribute('data-kind'))).toEqual(['dir', 'file', 'link', 'unknown'])
    expect(rows.map((row) => row.querySelector('.task-dir__name')?.textContent)).toEqual([
      'ce',
      'readme.md',
      'current',
      'mystery'
    ])
    expect(rows[0]?.querySelector('.task-dir__size')).toHaveTextContent('128B')
    for (const row of rows) {
      const icon = row.querySelector('svg')
      expect(icon).toHaveAttribute('width', '16')
      expect(icon).toHaveAttribute('height', '16')
      expect(icon).toHaveAttribute('stroke-width', '1.7')
    }
    expect(getComputedStyle(rows[0] as Element).minHeight).toBe('32px')
    expect(getComputedStyle(rows[0] as Element).fontSize).toBe('13px')
    expect(getComputedStyle(rows[0] as Element).color).toBe('rgb(63, 63, 63)')
    expect(getComputedStyle(card as Element).borderRadius).toBe('8px')
    expect(getComputedStyle(card as Element).backgroundColor).toBe('rgb(255, 255, 255)')
    expect(card?.querySelector('.task-dir__meta')).toHaveTextContent('4 of 9 entries')
    expect(card?.textContent).toContain('cursor=page-2')
    expect(taskConversationCss).toMatch(/\.task-dir__row:hover[\s\S]*?#f7f7f7/)
  })

  it('pins DESIGN.md row metrics on a tool call and a file product', () => {
    const { container } = renderTurn([
      { type: 'text', text: '先看 `src/notes.md`。' },
      {
        type: 'tool-bash',
        toolCallId: 'call-ls',
        state: 'output-available',
        input: { command: 'ls' },
        output: { stdout: 'AGENTS.md\nREADME.md', raw: { dumped: true } }
      },
      { type: 'text', text: '然后。' },
      {
        type: 'dynamic-tool',
        toolName: 'edit',
        toolCallId: 't-write',
        state: 'output-available',
        input: {},
        output: '',
        files: ['/workspace/notes.md']
      }
    ] as CherryMessagePart[])

    const tool = container.querySelector('[data-task-tool-name="bash"]')
    const label = tool?.querySelector('[data-task-tool-label]') as HTMLElement
    const summary = tool?.querySelector('.task-tool-row__summary') as HTMLElement
    const icon = tool?.querySelector('[data-agent-tool-icon] svg')
    const product = container.querySelector('[data-task-product]') as HTMLElement
    const productIcon = product?.querySelector('svg')
    expect(label).toHaveTextContent(/Run ls|运行 ls/)
    expect(getComputedStyle(label).fontSize).toBe('13px')
    expect(getComputedStyle(label).color).toBe('rgb(23, 23, 23)')
    expect(getComputedStyle(summary).minHeight).toBe('36px')
    expect(getComputedStyle(summary).borderRadius).toBe('8px')
    expect(getComputedStyle(summary).backgroundColor).toBe('rgb(255, 255, 255)')
    expect(getComputedStyle(summary).paddingLeft).toBe('10px')
    expect(getComputedStyle(summary).paddingRight).toBe('10px')
    expect(taskConversationCss).toMatch(/\.task-tool-row \[data-tool-summary\] \{[\s\S]*?gap: 10px;/)
    expect(icon).toHaveAttribute('width', '16')
    expect(icon).toHaveAttribute('height', '16')
    expect(icon).toHaveAttribute('stroke-width', '1.7')
    expect(summary.textContent).not.toContain('{')
    expect(summary.textContent).not.toContain('dumped')
    expect(getComputedStyle(product).height).toBe('36px')
    expect(getComputedStyle(product).fontSize).toBe('13px')
    expect(getComputedStyle(product).color).toBe('rgb(23, 23, 23)')
    expect(getComputedStyle(product).borderRadius).toBe('8px')
    expect(getComputedStyle(product).backgroundColor).toBe('rgb(255, 255, 255)')
    expect(getComputedStyle(product).paddingLeft).toBe('10px')
    expect(taskConversationCss).toMatch(/\.task-product-row \{[\s\S]*?gap: 10px;/)
    expect(productIcon).toHaveAttribute('width', '16')
    expect(productIcon).toHaveAttribute('height', '16')
    expect(productIcon).toHaveAttribute('stroke-width', '1.7')
    const status = tool?.querySelector('[data-tool-status]')
    expect(getComputedStyle(status as Element).color).toBe('rgb(138, 138, 138)')
    expect(taskConversationCss).toMatch(/\.task-tool-row__summary:hover[\s\S]*?#f7f7f7/)
    expect(taskConversationCss).toMatch(/\.task-product-row:hover[\s\S]*?#f7f7f7/)
    expect(taskConversationCss).toMatch(/\.task-tool-row__summary:focus-visible[\s\S]*?rgba\(0, 0, 0, 0\.1\)/)
    expect(taskConversationCss).toMatch(/\.task-product-row:focus-visible[\s\S]*?rgba\(0, 0, 0, 0\.1\)/)
    expect(mainStyles).toContain('pages/agents/taskConversation/task-conversation.css')
    expect(mainStyles).not.toMatch(/task-conversation\.css['"]\s+layer\(/)
    expect(taskConversationCss).toMatch(
      /\[data-ui='chat\.view'\]\.workbench-conversation \.messages-container \{[^}]*max-width:\s*none/
    )
    expect(taskConversationCss).toMatch(
      /\[data-ui='chat\.view'\]\.workbench-conversation \[data-composer-dock-surface\] \{[^}]*max-width:\s*var\(--app-chat-max-width, 768px\)/
    )
    expect(taskConversationCss).toMatch(
      /\[data-task-disclosure\] \{[\s\S]*?grid-template-rows:\s*minmax\(0,\s*0fr\);[\s\S]*?grid-template-rows 180ms cubic-bezier\(0\.22, 1, 0\.36, 1\)/
    )
    expect(taskConversationCss).toMatch(
      /\[data-task-disclosure\]\[data-open=['"]true['"]\][\s\S]*?grid-template-rows:\s*minmax\(0,\s*1fr\);/
    )
    expect(taskConversationCss).toMatch(
      /@media \(prefers-reduced-motion: reduce\) \{[\s\S]*?\[data-task-disclosure\],[\s\S]*?transition:\s*none;/
    )
  })
})
