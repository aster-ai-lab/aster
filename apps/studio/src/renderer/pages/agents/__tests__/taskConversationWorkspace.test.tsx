import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterAll, beforeEach, describe, expect, it, vi } from 'vitest'

import { MessagePartsScopeProvider } from '@renderer/components/chat/messages/blocks/MessagePartsContext'
import MessageContent from '@renderer/components/chat/messages/frame/MessageContent'
import type * as MessageListProvider from '@renderer/components/chat/messages/MessageListProvider'
import type { MessageListItem } from '@renderer/components/chat/messages/types'
import type * as RightPanelShell from '@renderer/components/chat/panes/Shell'
import i18n from '@renderer/i18n/resolver'
import type { CherryMessagePart } from '@shared/data/types/message'

import { conversationEditorTarget, openConversationFileInWorkspace } from '../taskConversation/focusConversationFile'
import {
  fileTreeCases,
  terminalCases,
  workspaceOpenCases,
  type TaskConversationPart
} from '../taskConversation/taskConversationCases'
import { WorkbenchProjectLockedContext } from '../workbenchComposerHost'
import { WorkbenchSessionFiles } from '../WorkbenchSessionFiles'
import { WorkbenchTaskWorkspace } from '../WorkbenchTaskWorkspace'
import { useWorkbenchWorkspaceTabs, WorkbenchWorkspaceTabsProvider } from '../workbenchWorkspaceTabs'

const bridge = vi.hoisted(() => ({
  workspacePath: '/tmp/alpha',
  openEditor: (() => undefined) as (path: string, name: string) => void,
  open: (() => undefined) as (path: string) => void
}))

vi.mock('../WorkbenchFileEditor', () => ({
  WorkbenchFileEditor: ({ path }: { path: string }) => <div data-workspace-file-editor={path} />
}))

vi.mock('@renderer/components/chat/panes/Shell', async (importOriginal) => ({
  ...(await importOriginal<typeof RightPanelShell>()),
  useRightPanelState: () => ({
    presentationOpen: true,
    presentationMaximized: false,
    activePanelId: undefined as string | undefined
  }),
  useRightPanelActions: () => ({
    tryOpen: () => true,
    close: () => undefined,
    toggleMaximized: () => undefined,
    canOpen: () => true
  }),
  useRightPanelPresentationMaximized: () => false
}))

vi.mock('@renderer/components/chat/messages/MessageListProvider', async (importOriginal) => {
  const actual = await importOriginal<typeof MessageListProvider>()
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
    useOptionalMessageListActions: () => ({ openArtifactFile: (path: string) => bridge.open(path) })
  }
})

bridge.open = (path: string) => {
  openConversationFileInWorkspace(path, bridge.workspacePath, bridge.openEditor)
}

const initialLanguage = i18n.language

interface DiskRow {
  name: string
  path: string
  kind: 'file' | 'directory'
  sizeBytes: number
  modifiedMs: number
}

function disk(root: string, folder: string, fileName: string) {
  const folderPath = `${root}/${folder}`
  const lists = new Map<string, DiskRow[]>([
    [
      root,
      [
        { name: folder, path: folderPath, kind: 'directory', sizeBytes: 0, modifiedMs: 3 },
        { name: fileName, path: `${root}/${fileName}`, kind: 'file', sizeBytes: 2, modifiedMs: 2 }
      ]
    ],
    [folderPath, []]
  ])
  const parentOf = (path: string) => path.slice(0, path.lastIndexOf('/')) || root
  return {
    async handle(input: RequestInfo, init?: RequestInit) {
      const url = String(input)
      if (url.includes('/changes')) return { ok: false, status: 404, json: async () => ({}) }
      if (init?.method === 'POST') {
        const body = JSON.parse(String(init.body)) as {
          parent?: string
          name?: string
          path?: string
          destination?: string
        }
        if (url.endsWith('/api/local-documents/entries')) {
          const parent = body.parent && lists.has(body.parent) ? body.parent : root
          const created: DiskRow = {
            name: body.name ?? '',
            path: `${parent}/${body.name}`,
            kind: 'file',
            sizeBytes: 0,
            modifiedMs: 1
          }
          lists.set(parent, [...(lists.get(parent) ?? []), created])
          return { ok: true, status: 200, json: async () => created }
        }
        if (url.endsWith('/api/local-documents/rename')) {
          const parent = parentOf(body.path ?? '')
          const next = (lists.get(parent) ?? []).map((row) =>
            row.path === body.path ? { ...row, name: body.name ?? row.name, path: `${parent}/${body.name}` } : row
          )
          lists.set(parent, next)
          return { ok: true, status: 200, json: async () => next.find((row) => row.name === body.name) }
        }
        if (url.endsWith('/api/local-documents/move')) {
          const from = parentOf(body.path ?? '')
          const source = lists.get(from) ?? []
          const moved = source.find((row) => row.path === body.path)
          lists.set(
            from,
            source.filter((row) => row.path !== body.path)
          )
          const destination = body.destination ?? root
          const placed: DiskRow = {
            ...(moved ?? { name: '', path: '', kind: 'file', sizeBytes: 0, modifiedMs: 1 }),
            name: moved?.name ?? '',
            path: `${destination}/${moved?.name ?? ''}`
          }
          lists.set(destination, [...(lists.get(destination) ?? []), placed])
          return { ok: true, status: 200, json: async () => placed }
        }
      }
      const path = new URL(url, 'http://127.0.0.1').searchParams.get('path') || root
      return { ok: true, status: 200, json: async () => ({ entries: lists.get(path) ?? [], capped: false }) }
    }
  }
}

function EditorBridge() {
  const api = useWorkbenchWorkspaceTabs()
  bridge.openEditor = api.openEditor
  return null
}

function assistant(): MessageListItem {
  return {
    id: 'turn-1',
    role: 'assistant',
    topicId: 'topic-1',
    createdAt: '2026-10-05T00:00:00.000Z',
    status: 'success'
  }
}

function renderConversation(parts: readonly TaskConversationPart[], workspacePath: string) {
  bridge.workspacePath = workspacePath
  return render(
    <WorkbenchProjectLockedContext value={true}>
      <WorkbenchWorkspaceTabsProvider>
        <EditorBridge />
        <MessagePartsScopeProvider messageId="turn-1" parts={parts as CherryMessagePart[]}>
          <MessageContent message={assistant()} />
        </MessagePartsScopeProvider>
        <WorkbenchTaskWorkspace
          agentSessionId="session-1"
          cwd={workspacePath}
          renderBrowser={() => <div data-browser-body="true" />}
          renderFiles={() => <div data-files-body="true" />}
        />
      </WorkbenchWorkspaceTabsProvider>
    </WorkbenchProjectLockedContext>
  )
}

describe('task conversation workspace', () => {
  beforeEach(async () => {
    await i18n.changeLanguage('zh-CN')
    vi.stubGlobal(
      'ResizeObserver',
      class ResizeObserver {
        observe() {}
        unobserve() {}
        disconnect() {}
      }
    )
  })

  afterAll(async () => {
    await i18n.changeLanguage(initialLanguage)
  })

  it('opens conversation files through the workbench editor hook', () => {})

  it.each(workspaceOpenCases)('focuses the editor from the product, the citation, and the diff ($id)', (sample) => {
    expect(conversationEditorTarget(sample.workspacePath, sample.http)).toBeNull()
    expect(conversationEditorTarget(sample.workspacePath, sample.productPath)?.path).toBe(sample.productPath)
    expect(conversationEditorTarget(sample.workspacePath, sample.citation)?.path).toBe(sample.citationPath)
    expect(conversationEditorTarget(sample.workspacePath, sample.diffPath)?.path).toBe(sample.diffEditorPath)

    const { container } = renderConversation(sample.parts, sample.workspacePath)
    fireEvent.click(container.querySelector(`a[href="${sample.http}"]`) as Element)
    expect(container.querySelector('[data-workspace-file-editor]')).toBeNull()

    fireEvent.click(container.querySelector('[data-task-product]') as Element)
    expect(container.querySelector(`[data-workspace-file-editor="${sample.productPath}"]`)).toBeTruthy()

    fireEvent.click(container.querySelector('[data-workspace-path-citation="true"]') as Element)
    expect(container.querySelector(`[data-workspace-file-editor="${sample.citationPath}"]`)).toBeTruthy()

    fireEvent.click(container.querySelector('[data-task-tool-name="edit"] summary') as Element)
    fireEvent.click(container.querySelector('[data-code-diff-open-file]') as Element)
    expect(container.querySelector(`[data-workspace-file-editor="${sample.diffEditorPath}"]`)).toBeTruthy()
    expect(container.querySelectorAll('[data-workspace-editor-path]')).toHaveLength(3)
  })

  it.each(fileTreeCases)('lists, opens, creates, renames, and moves a session file ($id)', async (sample) => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:4318', token: 'ab'.repeat(32) }
    })
    const store = disk(sample.root, sample.folder, sample.fileName)
    vi.stubGlobal(
      'fetch',
      vi.fn((input: RequestInfo, init?: RequestInit) => store.handle(input, init))
    )
    const user = userEvent.setup()
    const { container, unmount } = render(
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchWorkspaceTabsProvider>
          <WorkbenchTaskWorkspace
            agentSessionId={sample.id}
            cwd={sample.root}
            renderBrowser={() => null}
            renderFiles={() => <WorkbenchSessionFiles root={sample.root} />}
          />
        </WorkbenchWorkspaceTabsProvider>
      </WorkbenchProjectLockedContext>
    )

    fireEvent.click(screen.getByRole('button', { name: '文件' }))
    expect(await screen.findByRole('button', { name: sample.folder })).toHaveAttribute('aria-expanded', 'true')
    const file = () => document.querySelector(`[data-workspace-session-file="${sample.fileName}"]`) as HTMLButtonElement
    await waitFor(() => expect(file()).toBeTruthy())
    expect(screen.getByRole('navigation', { name: '会话文件夹' })).toHaveAttribute(
      'data-workspace-session-root',
      sample.root
    )

    fireEvent.click(file())
    expect(container.querySelector('[data-workspace-file-editor]')).toBeNull()
    fireEvent.doubleClick(file())
    await waitFor(() => {
      expect(container.querySelector(`[data-workspace-file-editor="${sample.root}/${sample.fileName}"]`)).toBeTruthy()
    })
    fireEvent.keyDown(file(), { key: 'F10', shiftKey: true })
    expect(await screen.findByRole('menuitem', { name: '删除' })).toBeTruthy()
    expect(screen.getByRole('menuitem', { name: '重命名' })).toBeTruthy()
    fireEvent.keyDown(window, { key: 'Escape' })
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    expect(vi.mocked(fetch).mock.calls.some((call) => String(call[0]).includes('/api/workspace-file/open'))).toBe(false)

    fireEvent.click(screen.getByRole('tab', { name: '文件' }))
    await user.click(screen.getByRole('button', { name: '新建文件' }))
    const created = await screen.findByRole('textbox', { name: '名称' })
    await user.clear(created)
    await user.type(created, `${sample.created}{Enter}`)
    await waitFor(() => {
      expect(document.querySelector(`[data-workspace-session-file="${sample.created}"]`)).toBeTruthy()
    })

    fireEvent.contextMenu(file())
    await user.click(await screen.findByRole('menuitem', { name: '重命名' }))
    const rename = await screen.findByRole('textbox', { name: '重命名' })
    await user.clear(rename)
    await user.type(rename, `${sample.renamed}{Enter}`)
    const renamed = () =>
      document.querySelector(`[data-workspace-session-file="${sample.renamed}"]`) as HTMLButtonElement
    await waitFor(() => expect(renamed()).toBeTruthy())
    expect(document.querySelector(`[data-workspace-session-file="${sample.fileName}"]`)).toBeNull()

    const folder = screen.getByRole('button', { name: sample.folder })
    const previous = document.elementFromPoint
    document.elementFromPoint = () => folder
    const pointer = (type: 'pointerdown' | 'pointermove' | 'pointerup', source: HTMLElement, x: number) => {
      const event = new MouseEvent(type, { bubbles: true, cancelable: true, button: 0, clientX: x, clientY: 12 })
      Object.defineProperty(event, 'pointerId', { value: 1 })
      act(() => {
        source.dispatchEvent(event)
      })
    }
    pointer('pointerdown', renamed(), 8)
    pointer('pointermove', renamed(), 40)
    pointer('pointerup', renamed(), 40)
    document.elementFromPoint = previous
    await waitFor(() => {
      expect(document.querySelector(`[data-workspace-session-file="${sample.renamed}"]`)).toHaveAttribute(
        'data-workspace-session-file-folder',
        sample.folder
      )
    })
    unmount()
    Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
  })

  it.each(terminalCases)('keeps the terminal session mounted across files and the browser ($id)', (sample) => {
    const { container } = render(
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchWorkspaceTabsProvider sessionId={sample.sessionId}>
          <WorkbenchTaskWorkspace
            agentSessionId={sample.sessionId}
            cwd={sample.cwd}
            renderBrowser={(active) => <div data-browser-body={sample.cwd} data-browser-active={String(active)} />}
            renderFiles={(active) => <div data-files-body={sample.cwd} data-files-active={String(active)} />}
          />
        </WorkbenchWorkspaceTabsProvider>
      </WorkbenchProjectLockedContext>
    )

    fireEvent.click(screen.getByRole('button', { name: '终端' }))
    fireEvent.click(screen.getByRole('button', { name: '新建终端' }))
    expect(container.querySelectorAll('[data-conversation-terminal-pane]')).toHaveLength(2)

    fireEvent.click(screen.getByRole('button', { name: '新建工作区标签' }))
    fireEvent.click(screen.getByRole('menuitem', { name: '文件' }))
    expect(container.querySelector('[data-files-body]')?.getAttribute('data-files-body')).toBe(sample.cwd)
    expect(container.querySelectorAll('[data-workspace-terminal="true"]')).toHaveLength(2)

    fireEvent.click(screen.getByRole('button', { name: '新建工作区标签' }))
    fireEvent.click(screen.getByRole('menuitem', { name: '浏览器' }))
    expect(container.querySelector('[data-browser-body]')?.getAttribute('data-browser-body')).toBe(sample.cwd)
    expect(container.querySelectorAll('[data-conversation-terminal-pane]')).toHaveLength(2)

    fireEvent.click(container.querySelector('[data-workspace-tab="terminal"]') as HTMLButtonElement)
    expect(container.querySelector('[data-workspace-tab-body="terminal"]')).toBeTruthy()
    expect(container.querySelectorAll('[data-conversation-terminal-pane]')).toHaveLength(2)
    expect(container.querySelectorAll('[data-workspace-terminal="true"]')).toHaveLength(2)
  })
})
