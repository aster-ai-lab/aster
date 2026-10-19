import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { createAixWorkbenchPlugins } from '@renderer/components/plugins/AixWorkbenchRuntime'
import { PluginScopeProvider } from '@renderer/components/plugins/PluginScopeProvider'
import { writeWorkbenchSidebarCollapsed } from '@renderer/layout/pageSidebar'
import { DATA_PACKAGE_ID, DATA_PACKAGE_SLUG, dataWorkbenchCatalogue } from '@renderer/pages/data/dataPackageCatalogue'
import { launchableUiPlugin } from '@renderer/pages/plugins/pluginLaunch'
import { WorkbenchPluginHost } from '@renderer/services/plugins'
import { receiveAppletHomeDocument, resetAppletHomeForTests } from '@renderer/services/plugins/aixAppletHome'
import { publishAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import { aixComposerModeSchema } from '@shared/types/aixComposerMode'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import {
  HomeComposerModeBar,
  requestHomeComposerSend,
  resetHomeComposerModeForTests,
  selectHomeComposerMode,
  submitHomeComposerDraft
} from '../homeComposerMode'
import { armPresentationDeck, resetPresentationDeckForTests, syncPresentationSidebar } from '../presentationDeck'
import { PRESENTON_PACKAGE_ID, PRESENTON_SLUG, PRESENTON_TITLE, withPresentonHomepage } from '../presentonHomepage'
import { WorkbenchProjectLockedContext } from '../workbenchComposerHost'
import { WorkbenchTaskWorkspace } from '../WorkbenchTaskWorkspace'
import { WorkbenchWorkspaceTabsProvider } from '../workbenchWorkspaceTabs'

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

vi.mock('@renderer/components/chat/panes/Shell', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@renderer/components/chat/panes/Shell')>()
  return {
    ...actual,
    RightPanelHeaderControls: () => null,
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
  }
})

const REVISION = `sha256:${'ab'.repeat(32)}`

function pass(name: string) {
  console.log(`presentation.${name}=pass`)
}

function presentonCatalogue(): AixWorkbenchCatalogue {
  return withPresentonHomepage({
    protocolVersion: 1,
    revision: '',
    packages: [
      {
        slug: PRESENTON_SLUG,
        packageId: PRESENTON_PACKAGE_ID,
        generation: 4,
        revision: REVISION,
        containers: [],
        views: [
          {
            id: 'workbench',
            title: 'Presenton 演示文稿',
            description: '',
            icon: 'panels',
            order: 20,
            container: null,
            point: 'studio.applets',
            presentation: 'webview'
          }
        ],
        composerModes: []
      }
    ]
  })
}

function listenForSeed(slug: string, view: string) {
  const channel = new MessageChannel()
  const received = new Promise<unknown>((resolve) => {
    channel.port1.onmessage = (event: MessageEvent) => resolve(event.data)
  })
  receiveAppletHomeDocument(slug, view, channel.port2)
  channel.port1.start()
  channel.port2.start()
  return received
}

describe('presentation mode journey', () => {
  beforeEach(() => {
    resetHomeComposerModeForTests()
    resetAppletHomeForTests()
    resetPresentationDeckForTests()
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    window.localStorage.removeItem('workbench.sidebarCollapsed')
    panel.close.mockClear()
  })

  it('runs the homepage, workspace, and sidebar checks in order', async () => {
    expect(
      aixComposerModeSchema.safeParse({
        id: 'slides',
        contractVersion: 1,
        title: '演示文稿',
        description: '',
        icon: 'panels',
        agent: 'presenton',
        scope: 'workspace',
        context: ['workspace'],
        order: 20,
        task: true
      }).success
    ).toBe(false)

    const hidden = withPresentonHomepage({ protocolVersion: 1, revision: '', packages: [] })
    publishAixCatalogue(hidden)
    const hiddenBar = render(<HomeComposerModeBar />)
    expect(screen.queryByRole('radio', { name: PRESENTON_TITLE })).toBeNull()
    hiddenBar.unmount()
    pass('uninstalled-hides-mode')

    const catalogue = presentonCatalogue()
    publishAixCatalogue(catalogue)
    const plugins = createAixWorkbenchPlugins(catalogue)
    expect(plugins[0]?.contributions.find((item) => item.id.startsWith('task-editor.'))?.title).toBe(PRESENTON_TITLE)
    expect(
      plugins[0]?.contributions.some((item) => item.slot === 'workspace' && item.id.startsWith('task-editor.'))
    ).toBe(true)
    expect(plugins[0]?.contributions.some((item) => item.slot === 'applet' || item.slot === 'activity')).toBe(false)
    render(<HomeComposerModeBar />)
    expect(screen.getByRole('radio', { name: PRESENTON_TITLE })).toBeTruthy()
    fireEvent.click(screen.getByRole('radio', { name: PRESENTON_TITLE }))
    expect(document.querySelector('[data-ui="workbench.composer.controls"]')).toBeNull()
    pass('installed-shows-title')

    const empty = await act(() => requestHomeComposerSend({ surface: 'live', text: '   ', permissionMode: 'default' }))
    expect(empty).toEqual({ handled: true, cleared: false })
    expect(screen.queryByRole('dialog')).toBeNull()
    pass('empty-topic-skips-dialog')

    const invoke = vi.fn(async () => null)
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke } })
    const seeded = listenForSeed(PRESENTON_SLUG, 'workbench')
    let pending!: Promise<{ handled: boolean; cleared: boolean; deck?: { title: string; topic: string } }>
    act(() => {
      pending = requestHomeComposerSend({ surface: 'live', text: '季度业务回顾', permissionMode: 'default' })
    })
    const dialog = screen.getByRole('dialog', { name: PRESENTON_TITLE })
    for (const label of ['页数', '语言', '语气', '详略', '标题页', '补充说明']) {
      expect(within(dialog).getByText(label)).toBeTruthy()
    }
    expect(dialog.querySelector('select, datalist')).toBeNull()
    pass('dialog-has-six-controls')

    fireEvent.click(within(dialog).getByRole('button', { name: '生成' }))
    const sent = await pending
    expect(sent).toMatchObject({
      handled: false,
      cleared: false,
      deck: { title: PRESENTON_TITLE, topic: '季度业务回顾' }
    })
    expect(sent).toHaveProperty('text')
    expect(String((sent as { text?: string }).text)).toContain('季度业务回顾')
    expect(String((sent as { text?: string }).text)).not.toContain('{"step":"outline"')
    expect(invoke).not.toHaveBeenCalled()
    const posted = (await seeded) as { type?: string; seed?: { content?: string; n_slides?: unknown } }
    expect(posted.type).toBe('aix.view.seed')
    expect(posted.seed).toMatchObject({ content: '季度业务回顾', n_slides: null })
    pass('confirm-starts-task')

    const deck = sent.deck
    if (!deck) throw new Error('missing deck')
    armPresentationDeck('task-1', deck)
    const host = new WorkbenchPluginHost('presentation-journey')
    const parent = await host.openScope({ id: 'main', kind: 'window' })
    await host.setPlugins(plugins)
    const workspace = render(
      <PluginScopeProvider scope={parent}>
        <WorkbenchProjectLockedContext value={true}>
          <WorkbenchWorkspaceTabsProvider>
            <WorkbenchTaskWorkspace
              agentSessionId="task-1"
              cwd="/tmp/workspace"
              renderBrowser={() => null}
              renderFiles={() => null}
            />
          </WorkbenchWorkspaceTabsProvider>
        </WorkbenchProjectLockedContext>
      </PluginScopeProvider>
    )
    await waitFor(() => {
      expect(workspace.container.querySelector('[data-workspace-presentation-canvas]')).toBeTruthy()
    })
    expect(workspace.container.querySelector('[data-workspace-tab-body="presentation"]')).toBeTruthy()
    expect(workspace.container.querySelector('[data-presentation-status]')).toBeNull()
    expect(workspace.container).not.toHaveTextContent('让想法变成幻灯片')
    pass('workspace-shows-canvas')
    workspace.unmount()
    await host.close()

    resetPresentationDeckForTests()
    armPresentationDeck('task-1', deck)
    const outline = render(
      <WorkbenchProjectLockedContext value={true}>
        <WorkbenchWorkspaceTabsProvider>
          <WorkbenchTaskWorkspace
            agentSessionId="task-1"
            cwd="/tmp/workspace"
            renderBrowser={() => null}
            renderFiles={() => null}
          />
        </WorkbenchWorkspaceTabsProvider>
      </WorkbenchProjectLockedContext>
    )
    expect(outline.container.querySelector('[data-workspace-presentation-canvas]')).toBeNull()
    expect(outline.container.querySelector('[data-ui="workbench.presentation"]')).toHaveTextContent('季度业务回顾')
    expect(outline.container).not.toHaveTextContent('让想法变成幻灯片')
    pass('outline-without-canvas')

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
    pass('other-task-has-no-tab')
    outline.unmount()
    other.unmount()

    expect(syncPresentationSidebar('task-1', 'task-1', null)).toBe('task-1')
    expect(window.localStorage.getItem('workbench.sidebarCollapsed')).toBe('1')
    writeWorkbenchSidebarCollapsed(false)
    expect(syncPresentationSidebar('task-1', 'task-1', 'task-1')).toBe('task-1')
    expect(window.localStorage.getItem('workbench.sidebarCollapsed')).toBe('0')
    pass('sidebar-collapses-once')

    publishAixCatalogue(dataWorkbenchCatalogue())
    resetHomeComposerModeForTests()
    selectHomeComposerMode(`${DATA_PACKAGE_ID}/analysis`)
    const dataInvoke = vi.fn(async (command: string) => {
      if (command === 'open_aix_applet_window') return 4
      if (command === 'read_aix_applet_home_deck') {
        return { delivery: 4, holdsWork: false, applied: true, seed: { content: '查询销售额' } }
      }
      return null
    })
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke: dataInvoke } })
    await expect(
      submitHomeComposerDraft({ surface: 'live', text: '查询销售额', permissionMode: 'default' })
    ).resolves.toEqual({ handled: true, cleared: true })
    expect(dataInvoke).toHaveBeenCalledWith('open_aix_applet_window', {
      slug: DATA_PACKAGE_SLUG,
      view: 'analysis',
      title: '数据分析',
      seed: JSON.stringify({ content: '查询销售额' })
    })
    pass('data-analysis-opens-window')

    const card = { slug: PRESENTON_SLUG, surfaces: ['ui'] as const, available: true }
    expect(launchableUiPlugin(card, true, catalogue)).toBeNull()
    expect(
      launchableUiPlugin({ slug: DATA_PACKAGE_SLUG, surfaces: ['ui'], available: true }, true, dataWorkbenchCatalogue())
    ).not.toBeNull()
    pass('presenton-card-has-no-open')
  })
})
