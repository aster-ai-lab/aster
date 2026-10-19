import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { useEffect, useRef } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type * as RightPanelShell from '@renderer/components/chat/panes/Shell'
import { PluginScopeProvider } from '@renderer/components/plugins/PluginScopeProvider'
import { WorkbenchPluginHost } from '@renderer/services/plugins'
import type { PluginActivationContext, PluginRenderProps } from '@renderer/types/workbenchPlugin'

import { armPresentationDeck, resetPresentationDeckForTests } from '../presentationDeck'
import { WorkbenchProjectLockedContext } from '../workbenchComposerHost'
import { WorkbenchTaskWorkspace } from '../WorkbenchTaskWorkspace'
import { useWorkbenchWorkspaceTabs, WorkbenchWorkspaceTabsProvider } from '../workbenchWorkspaceTabs'

vi.mock('../WorkbenchFileEditor', () => ({
  WorkbenchFileEditor: ({ path }: { path: string }) => <div data-workspace-file-editor={path} />
}))

const panel = vi.hoisted(() => ({
  presentationOpen: true,
  presentationMaximized: false,
  activePanelId: undefined as string | undefined,
  tryOpen: vi.fn(() => true),
  close: vi.fn(),
  toggleMaximized: vi.fn()
}))

vi.mock('@renderer/components/chat/panes/Shell', async (importOriginal) => ({
  ...(await importOriginal<typeof RightPanelShell>()),
  RightPanelHeaderControls: () => (
    <div>
      <button type="button" aria-label="最大化" onClick={panel.toggleMaximized} />
      <button type="button" aria-label="关闭侧边栏" onClick={panel.close} />
    </div>
  ),
  useRightPanelState: () => ({
    presentationOpen: panel.presentationOpen,
    presentationMaximized: panel.presentationMaximized,
    activePanelId: panel.activePanelId
  }),
  useRightPanelActions: () => ({
    tryOpen: panel.tryOpen,
    close: panel.close,
    toggleMaximized: panel.toggleMaximized
  })
}))

function renderWorkspace() {
  return render(
    <WorkbenchProjectLockedContext value={true}>
      <WorkbenchWorkspaceTabsProvider>
        <WorkbenchTaskWorkspace
          agentSessionId="session-1"
          cwd="/tmp/workspace"
          renderBrowser={(active) => <div data-browser-active={active ? 'true' : 'false'}>browser-body</div>}
          renderFiles={(active) => <div data-files-active={active ? 'true' : 'false'}>files-body</div>}
        />
      </WorkbenchWorkspaceTabsProvider>
    </WorkbenchProjectLockedContext>
  )
}

function OpenDocumentOnMount() {
  const api = useWorkbenchWorkspaceTabs()
  const once = useRef(false)
  useEffect(() => {
    if (once.current) return
    once.current = true
    api.openEditor('/tmp/workspace/通知.docx', '通知.docx')
  }, [api])
  return null
}

describe('WorkbenchTaskWorkspace', () => {
  beforeEach(() => {
    resetPresentationDeckForTests()
    panel.presentationOpen = true
    panel.presentationMaximized = false
    panel.activePanelId = undefined
    panel.tryOpen.mockClear()
    panel.close.mockClear()
    panel.toggleMaximized.mockClear()
  })

  it('admits a scoped plugin panel, returns to files on withdrawal, and cleans up on workspace exit', async () => {
    const host = new WorkbenchPluginHost('test')
    const parent = await host.openScope({ id: 'main', kind: 'window' })
    const contexts: PluginActivationContext[] = []
    const released: string[] = []
    const Panel = ({ context }: PluginRenderProps) => (
      <p>
        Panel {context.services.workspace?.id}:{context.services.conversation?.sessionId}
      </p>
    )
    await host.setPlugins([
      {
        id: 'review',
        generation: 1,
        scopes: ['workspace'],
        services: ['workspace', 'conversation'],
        contributions: [{ id: 'panel', slot: 'workspace', labelKey: 'common.loading', component: Panel }],
        activate: (context) => {
          contexts.push(context)
          context.defer(() => {
            released.push(context.services.workspace!.id)
          })
        }
      }
    ])
    const view = (workspaceId?: string) => (
      <PluginScopeProvider scope={parent}>
        <WorkbenchProjectLockedContext value={true}>
          <WorkbenchWorkspaceTabsProvider>
            <WorkbenchTaskWorkspace
              agentSessionId="session-1"
              workspaceId={workspaceId}
              cwd="/work/alpha"
              renderBrowser={() => null}
              renderFiles={() => <p>Files remain available</p>}
            />
          </WorkbenchWorkspaceTabsProvider>
        </WorkbenchProjectLockedContext>
      </PluginScopeProvider>
    )
    const rendered = render(view())
    await waitFor(() => expect([...parent.children][0]?.getSnapshot().fibers[0]?.state).toBe('blocked'))
    fireEvent.click(screen.getByRole('button', { name: '新建工作区标签' }))
    expect(screen.queryByRole('menuitem', { name: /^加载中/ })).toBeNull()
    rendered.rerender(view('alpha'))
    const entry = await screen.findByRole('menuitem', { name: /^加载中/ })
    fireEvent.click(entry)
    expect(screen.getByText('Panel alpha:session-1')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '新建工作区标签' }))
    fireEvent.click(screen.getByRole('menuitem', { name: '文件' }))
    expect(screen.queryByText('Panel alpha:session-1')).toBeNull()
    expect(screen.getByText('Files remain available')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '新建工作区标签' }))
    fireEvent.click(screen.getByRole('menuitem', { name: /^加载中/ }))
    await act(async () => {
      await host.setPlugins([])
    })
    expect(screen.queryByText('Panel alpha:session-1')).toBeNull()
    expect(screen.getByText('Files remain available')).toBeVisible()
    expect(contexts[0].signal.aborted).toBe(true)
    await waitFor(() => expect(released).toEqual(['alpha']))
    await act(async () => {
      await host.setPlugins([
        {
          id: 'review',
          generation: 2,
          scopes: ['workspace'],
          services: ['workspace', 'conversation'],
          contributions: [{ id: 'panel', slot: 'workspace', labelKey: 'common.loading', component: Panel }],
          activate: (context) => {
            contexts.push(context)
            context.defer(() => {
              released.push(context.services.workspace!.id)
            })
          }
        }
      ])
    })
    rendered.unmount()
    await waitFor(() => expect(parent.children.size).toBe(0))
    expect(contexts[1].signal.aborted).toBe(true)
    expect(released).toEqual(['alpha', 'alpha'])
    expect(parent.isOpen()).toBe(true)
    await host.close()
  })

  it('puts maximize and collapse at the workspace corner', () => {
    renderWorkspace()

    fireEvent.click(screen.getByRole('button', { name: '最大化' }))
    fireEvent.click(screen.getByRole('button', { name: '关闭侧边栏' }))

    expect(panel.toggleMaximized).toHaveBeenCalledTimes(1)
    expect(panel.close).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('button', { name: '收起任务工作区' })).toBeNull()
  })

  it('keeps files beside a terminal and keeps every terminal session mounted', () => {
    const { container } = renderWorkspace()

    fireEvent.click(screen.getByRole('button', { name: '终端' }))
    fireEvent.click(screen.getByRole('button', { name: '新建终端' }))

    expect(container.querySelectorAll('[data-conversation-terminal-pane]')).toHaveLength(2)
    expect(container.querySelectorAll('[data-workspace-terminal="true"]')).toHaveLength(2)
    expect(container.querySelector('[data-conversation-terminal-pane-active="true"]')).toBeTruthy()
    expect(container.querySelectorAll('[data-workspace-terminal-active="false"]')).toHaveLength(1)

    fireEvent.click(screen.getByRole('button', { name: '新建工作区标签' }))
    fireEvent.click(screen.getByRole('menuitem', { name: '文件' }))

    expect(container.querySelector('[data-workspace-tab="files"]')).toBeTruthy()
    expect(container.querySelector('[data-workspace-tab="terminal"]')).toBeTruthy()
    expect(container.querySelector('[data-workspace-tab-body="files"]')).toBeTruthy()
    expect(container.querySelectorAll('[data-workspace-terminal="true"]')).toHaveLength(2)
    expect(screen.getByText('files-body')).toBeInTheDocument()

    fireEvent.click(container.querySelector('[data-workspace-tab="terminal"]') as HTMLButtonElement)

    expect(container.querySelector('[data-workspace-tab-body="terminal"]')).toBeTruthy()
    expect(container.querySelectorAll('[data-workspace-terminal="true"]')).toHaveLength(2)
    expect(panel.tryOpen).toHaveBeenCalledWith('terminal', { userInitiated: true })
    expect(panel.tryOpen).toHaveBeenCalledWith('files', { userInitiated: true })
  })

  it('opens a built-in editor tab without leaving the workspace', () => {
    function OpenNote() {
      const api = useWorkbenchWorkspaceTabs()
      return (
        <button type="button" onClick={() => api.openEditor('/tmp/workspace/note.txt', 'note.txt')}>
          open-note
        </button>
      )
    }
    const { container } = render(
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchWorkspaceTabsProvider>
          <WorkbenchTaskWorkspace
            agentSessionId="session-1"
            cwd="/tmp/workspace"
            renderBrowser={() => null}
            renderFiles={() => <OpenNote />}
          />
        </WorkbenchWorkspaceTabsProvider>
      </WorkbenchProjectLockedContext>
    )

    fireEvent.click(screen.getByRole('button', { name: '文件' }))
    fireEvent.click(screen.getByRole('button', { name: 'open-note' }))

    expect(container.querySelector('[data-workspace-tab="editor"]')).toHaveTextContent('note.txt')
    expect(container.querySelector('[data-workspace-file-editor="/tmp/workspace/note.txt"]')).toBeTruthy()
    expect(panel.tryOpen).not.toHaveBeenCalledWith('editor', expect.anything())

    fireEvent.click(container.querySelector('[data-workspace-tab="files"]') as HTMLButtonElement)
    fireEvent.click(screen.getByRole('button', { name: 'open-note' }))
    expect(container.querySelectorAll('[data-workspace-tab="editor"]')).toHaveLength(1)

    fireEvent.click(container.querySelector('[data-workspace-tab="files"]') as HTMLButtonElement)
    fireEvent.click(container.querySelector('[data-workspace-tab="editor"]') as HTMLButtonElement)
    expect(container.querySelector('[data-workspace-tab-body="editor"]')).toBeTruthy()
    expect(panel.tryOpen).not.toHaveBeenCalledWith('editor', expect.anything())
  })

  it('opens the document alone when the files pane syncs in the same commit', () => {
    panel.activePanelId = 'files'
    const { container } = render(
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchWorkspaceTabsProvider>
          <OpenDocumentOnMount />
          <WorkbenchTaskWorkspace
            agentSessionId="session-1"
            cwd="/tmp/workspace"
            renderBrowser={() => null}
            renderFiles={() => <div>files-body</div>}
          />
        </WorkbenchWorkspaceTabsProvider>
      </WorkbenchProjectLockedContext>
    )

    expect(container.querySelector('[data-workspace-tab="editor"]')).toHaveTextContent('通知.docx')
    expect(container.querySelector('[data-workspace-tab="files"]')).toBeNull()
    expect(container.querySelector('[data-workspace-tab-body="editor"]')).toBeTruthy()
    expect(container.querySelector('[data-workspace-pane="files"]')).toBeNull()
  })

  it('closes other workspace tabs from the tab context menu', () => {
    const { container } = renderWorkspace()

    fireEvent.click(screen.getByRole('button', { name: '终端' }))
    fireEvent.click(screen.getByRole('button', { name: '新建工作区标签' }))
    fireEvent.click(screen.getByRole('menuitem', { name: '文件' }))
    fireEvent.contextMenu(container.querySelector('[data-workspace-tab="files"]') as HTMLElement)

    expect(screen.getByRole('menu')).toHaveAttribute('data-chrome-tab-context-menu', 'true')
    fireEvent.click(screen.getByRole('menuitem', { name: '关闭其他标签页' }))

    expect(container.querySelector('[data-workspace-tab="files"]')).toBeTruthy()
    expect(container.querySelector('[data-workspace-tab="terminal"]')).toBeNull()
    expect(panel.close).not.toHaveBeenCalled()
  })

  it('shows the launcher after every workspace tab is closed', () => {
    const { container } = renderWorkspace()

    fireEvent.click(screen.getByRole('button', { name: '终端' }))
    fireEvent.contextMenu(container.querySelector('[data-workspace-tab="terminal"]') as HTMLElement)
    fireEvent.click(screen.getByRole('menuitem', { name: '关闭所有标签页' }))

    expect(panel.close).not.toHaveBeenCalled()
    expect(container.querySelector('[data-workspace-launcher="true"]')).toBeTruthy()
    expect(screen.getByRole('button', { name: '文件' })).toBeTruthy()
    expect(screen.getByRole('button', { name: '浏览器' })).toBeTruthy()
  })

  it('closes terminal tabs to the right and can close every terminal tab', () => {
    const { container } = renderWorkspace()

    fireEvent.click(screen.getByRole('button', { name: '终端' }))
    fireEvent.click(screen.getByRole('button', { name: '新建终端' }))
    const first = container.querySelector('[data-conversation-terminal-tab]') as HTMLElement
    fireEvent.contextMenu(first)
    fireEvent.click(screen.getByRole('menuitem', { name: '关闭右侧标签页' }))

    expect(container.querySelectorAll('[data-conversation-terminal-tab]')).toHaveLength(1)

    fireEvent.contextMenu(container.querySelector('[data-conversation-terminal-tab]') as HTMLElement)
    expect(screen.getByRole('menuitem', { name: '关闭其他标签页' })).toBeDisabled()
    expect(screen.getByRole('menuitem', { name: '关闭右侧标签页' })).toBeDisabled()
    fireEvent.click(screen.getByRole('menuitem', { name: '关闭所有标签页' }))

    expect(panel.close).toHaveBeenCalled()
  })

  it('shows the plugin canvas in the right workspace of the armed task', async () => {
    const host = new WorkbenchPluginHost('canvas')
    const parent = await host.openScope({ id: 'main', kind: 'window' })
    const Canvas = () => <div data-workspace-presentation-canvas="true">slide-canvas</div>
    await host.setPlugins([
      {
        id: 'presenton',
        generation: 1,
        scopes: ['workspace'],
        contributions: [
          {
            id: 'task-editor.workbench',
            slot: 'workspace',
            labelKey: 'plugins.title',
            title: '演示文稿',
            scopes: ['workspace'],
            component: Canvas
          }
        ]
      }
    ])
    armPresentationDeck('session-1', { title: '演示文稿', topic: '季度业务回顾', lines: ['页数：自动'] })
    const rendered = render(
      <PluginScopeProvider scope={parent}>
        <WorkbenchProjectLockedContext value={true}>
          <WorkbenchWorkspaceTabsProvider>
            <WorkbenchTaskWorkspace
              agentSessionId="session-1"
              cwd="/tmp/workspace"
              renderBrowser={() => null}
              renderFiles={() => null}
            />
          </WorkbenchWorkspaceTabsProvider>
        </WorkbenchProjectLockedContext>
      </PluginScopeProvider>
    )
    expect(await screen.findByText('slide-canvas')).toBeInTheDocument()
    expect(rendered.container.querySelector('[data-workspace-tab-body="presentation"]')).toBeTruthy()
    expect(rendered.container.querySelector('[data-presentation-status]')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: '新建工作区标签' }))
    expect(screen.getByRole('menuitem', { name: '演示文稿' }).querySelector('svg')).toBeTruthy()
    rendered.unmount()
    await host.close()
  })

  it('shows the presentation work mode in the right workspace of the armed task', () => {
    armPresentationDeck('session-1', { title: '演示文稿', topic: '季度业务回顾', lines: ['页数：自动'] })
    const { container } = renderWorkspace()

    expect(container.querySelector('[data-workspace-tab="presentation"]')).toHaveTextContent('演示文稿')
    expect(container.querySelector('[data-workspace-tab-body="presentation"]')).toHaveTextContent('季度业务回顾')
    expect(container.querySelector('[data-ui="workbench.presentation"]')).not.toHaveTextContent('让想法变成幻灯片')
    expect(container.querySelector('[data-workspace-launcher="true"]')).toBeNull()

    const other = render(
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchWorkspaceTabsProvider>
          <WorkbenchTaskWorkspace
            agentSessionId="other-task"
            cwd="/tmp/workspace"
            renderBrowser={() => null}
            renderFiles={() => null}
          />
        </WorkbenchWorkspaceTabsProvider>
      </WorkbenchProjectLockedContext>
    )
    expect(other.container.querySelector('[data-workspace-tab="presentation"]')).toBeNull()
    expect(other.container.querySelector('[data-workspace-launcher="true"]')).toBeTruthy()
  })

  it('closes the workspace when the last tab closes', () => {
    const { container } = renderWorkspace()

    fireEvent.click(screen.getByRole('button', { name: '终端' }))
    fireEvent.click(container.querySelector('[data-workspace-tab-close="terminal"]') as HTMLButtonElement)

    expect(panel.close).toHaveBeenCalled()
  })
})
