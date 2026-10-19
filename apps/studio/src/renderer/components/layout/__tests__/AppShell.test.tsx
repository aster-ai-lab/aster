// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { MockUseCacheUtils } from '@test-mocks/renderer/useCache'
import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { MIN_WINDOW_HEIGHT, SECOND_MIN_WINDOW_WIDTH } from '@shared/utils/window'

const mocks = vi.hoisted(() => ({
  closeTab: vi.fn(),
  closeTabs: vi.fn(),
  detachTab: vi.fn(),
  setActiveTab: vi.fn(),
  commandHandlers: new Map<string, { handler: () => void; options?: { enabled?: boolean } }>(),
  ipcHandlers: new Map<string, (value: unknown) => void>(),
  ipcRequest: vi.fn(() => Promise.resolve(false)),
  activeTabId: 'home',
  platformState: { isMac: false },
  tabs: [
    {
      id: 'home',
      isDormant: false,
      title: 'Chat',
      type: 'route' as const,
      url: '/app/chat'
    }
  ],
  showSearchPopup: vi.fn(),
  hideSearchPopup: vi.fn(),
  openTab: vi.fn()
}))

vi.mock('@renderer/hooks/useMacTransparentWindow', () => ({
  default: () => false
}))

vi.mock('@renderer/utils/platform', () => ({
  get isMac() {
    return mocks.platformState.isMac
  }
}))

vi.mock('@renderer/hooks/command', () => ({
  useCommandHandler: (command: string, handler: () => void, options?: { enabled?: boolean }) => {
    mocks.commandHandlers.set(command, { handler, options })
  }
}))

vi.mock('@renderer/ipc', () => ({
  ipcApi: {
    on: vi.fn(() => () => {}),
    request: mocks.ipcRequest
  },
  useIpcOn: (event: string, handler: (value: unknown) => void) => {
    mocks.ipcHandlers.set(event, handler)
  }
}))

vi.mock('@renderer/components/GlobalSearch/GlobalSearchPopup', () => ({
  default: {
    show: mocks.showSearchPopup,
    hide: mocks.hideSearchPopup
  }
}))

vi.mock('../../../hooks/tab', () => ({
  useMainWindowNavigation: vi.fn(),
  useTabs: () => ({
    activeTabId: mocks.activeTabId,
    closeTab: mocks.closeTab,
    closeTabs: mocks.closeTabs,
    detachTab: mocks.detachTab,
    openTab: mocks.openTab,
    pinTab: vi.fn(),
    reorderTabs: vi.fn(),
    setActiveTab: mocks.setActiveTab,
    tabs: mocks.tabs,
    unpinTab: vi.fn(),
    updateTab: vi.fn()
  })
}))

vi.mock('../../app/Sidebar', () => ({
  default: () => <aside data-testid="sidebar" />
}))

vi.mock('../../GlobalSearch/globalSearchGroups', () => ({
  createRecentRouteEntryFromTab: () => null,
  upsertGlobalSearchRecentEntry: (items: unknown[]) => items
}))

vi.mock('../../MiniApp/MiniAppTabsPool', () => ({
  default: () => <div data-testid="mini-app-pool" />
}))

vi.mock('../../ResourceViewSourceProvider', () => ({
  ResourceViewSourceProvider: ({ children }: { children: ReactNode }) => (
    <div data-testid="resource-view-source-provider">{children}</div>
  )
}))

vi.mock('../TabRouter', () => ({
  TabRouter: ({ tab, isActive }: { tab: { id: string }; isActive?: boolean }) => (
    <section data-testid="tab-router" data-tab-id={tab.id} data-active={isActive ? 'true' : 'false'} />
  )
}))

import { AppShell } from '../AppShell'

function contentColumnContaining(root: HTMLElement, node: HTMLElement): HTMLElement {
  const column = Array.from(root.children).find((child) => child.contains(node))
  if (!(column instanceof HTMLElement)) {
    throw new Error('Expected the page to live in the content column beside the sidebar')
  }
  return column
}

function finishSettingsClose() {
  const panel = document.querySelector('.settings-overlay-panel')
  if (!(panel instanceof HTMLElement)) {
    throw new Error('Expected the settings overlay to be closing')
  }
  const event = new Event('animationend', { bubbles: true })
  Object.defineProperty(event, 'animationName', { value: 'settings-panel-out' })
  act(() => {
    panel.dispatchEvent(event)
  })
}

function contentMain(): HTMLElement {
  const main = document.querySelector('main[data-ui="app.content"]')
  if (!(main instanceof HTMLElement)) {
    throw new Error('Expected the content surface')
  }
  return main
}

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  document.documentElement.style.removeProperty('--a3s-traffic-light-inset')
  MockUseCacheUtils.resetMocks()
  mocks.commandHandlers.clear()
  mocks.ipcHandlers.clear()
  mocks.ipcRequest.mockResolvedValue(false)
  mocks.activeTabId = 'home'
  mocks.platformState.isMac = false
  mocks.tabs = [
    {
      id: 'home',
      isDormant: false,
      title: 'Chat',
      type: 'route',
      url: '/app/chat'
    }
  ]
})

describe('AppShell', () => {
  it('owns the resource source provider at the route host boundary', () => {
    render(<AppShell />)

    const provider = screen.getByTestId('resource-view-source-provider')

    expect(provider).toContainElement(screen.getByTestId('tab-router'))
    expect(provider).not.toContainElement(screen.getByTestId('mini-app-pool'))
    expect(provider).not.toContainElement(screen.getByTestId('sidebar'))
    expect(document.querySelector('[data-ui="app.tab-bar"]')).toBeNull()
  })

  it('applies the compact minimum window size for the active chat tab and resets it on leaving', async () => {
    mocks.tabs = [
      ...mocks.tabs,
      {
        id: 'agents',
        isDormant: false,
        title: 'Agents',
        type: 'route',
        url: '/app/agents'
      },
      {
        id: 'files',
        isDormant: false,
        title: 'Files',
        type: 'route',
        url: '/app/files'
      }
    ]

    const { rerender } = render(<AppShell />)

    await waitFor(() => {
      expect(mocks.ipcRequest).toHaveBeenCalledWith('window.main.set_minimum_size', {
        width: SECOND_MIN_WINDOW_WIDTH,
        height: MIN_WINDOW_HEIGHT
      })
    })
    expect(mocks.ipcRequest).not.toHaveBeenCalledWith('window.main.reset_minimum_size')

    // Switching between two compact tabs must not re-issue the IPC pair.
    mocks.ipcRequest.mockClear()
    mocks.activeTabId = 'agents'
    rerender(<AppShell />)
    expect(mocks.ipcRequest).not.toHaveBeenCalledWith('window.main.set_minimum_size', expect.anything())

    mocks.activeTabId = 'files'
    rerender(<AppShell />)
    await waitFor(() => {
      expect(mocks.ipcRequest).toHaveBeenCalledWith('window.main.reset_minimum_size')
    })
  })

  it('opens settings from the settings shortcut and closes it when settings is already open', () => {
    const view = render(<AppShell />)

    act(() => {
      mocks.commandHandlers.get('app.settings.open')?.handler()
    })

    expect(mocks.ipcRequest).toHaveBeenCalledWith('navigation.open_route_in_main', { path: '/settings/general' })

    const settingsTab = {
      id: 'settings',
      isDormant: false,
      title: 'Settings',
      type: 'route' as const,
      url: '/settings/general'
    }
    mocks.tabs = [...mocks.tabs, settingsTab]
    mocks.activeTabId = settingsTab.id
    view.rerender(<AppShell />)

    act(() => {
      mocks.commandHandlers.get('app.settings.open')?.handler()
    })

    expect(document.querySelector('[data-ui="settings.overlay"]')).toHaveAttribute('data-state', 'closing')
  })

  it('opens global search from the shell-level shortcut', () => {
    render(<AppShell />)

    mocks.commandHandlers.get('app.search')?.handler()

    expect(mocks.showSearchPopup).toHaveBeenCalledTimes(1)
  })

  it('keeps the ActivityBar beside the focused Settings tab', () => {
    const settingsTab = {
      id: 'settings',
      isDormant: false,
      title: 'Settings',
      type: 'route' as const,
      url: '/settings/provider'
    }
    mocks.tabs = [...mocks.tabs, settingsTab]
    mocks.activeTabId = settingsTab.id

    render(<AppShell />)

    const overlay = document.querySelector('[data-ui="settings.overlay"]')
    const home = screen.getAllByTestId('tab-router').find((router) => router.dataset.tabId === 'home')
    const settings = screen.getAllByTestId('tab-router').find((router) => router.dataset.tabId === 'settings')

    expect(screen.getByTestId('sidebar')).toBeInTheDocument()
    expect(screen.queryByTestId('macos-settings-titlebar')).not.toBeInTheDocument()
    expect(document.querySelector('[data-ui="app.tab-bar"]')).toBeNull()
    expect(overlay).toHaveAttribute('data-state', 'open')
    expect(overlay).toContainElement(settings ?? null)
    expect(overlay).not.toContainElement(home ?? null)
    const workspaceLayer = document.querySelector('[data-ui="app.workspace-layer"]')
    expect(workspaceLayer).toHaveClass('isolate')
    expect(workspaceLayer).toContainElement(home ?? null)
    expect(workspaceLayer).not.toContainElement(overlay)
    expect(workspaceLayer?.nextElementSibling).toBe(overlay)
    expect(home).toHaveAttribute('data-active', 'true')
    expect(contentMain()).toContainElement(overlay)
  })

  it('keeps Settings in the same icon-rail frame as other pages', () => {
    mocks.platformState.isMac = true
    MockUseCacheUtils.setPersistCacheValue('ui.sidebar.width', 60)
    const settingsTab = {
      id: 'settings',
      isDormant: false,
      title: 'Settings',
      type: 'route' as const,
      url: '/settings/provider'
    }
    mocks.tabs = [...mocks.tabs, settingsTab]
    mocks.activeTabId = settingsTab.id

    render(<AppShell />)

    const content = document.querySelector('main')
    const card = content?.parentElement?.parentElement
    const column = card?.parentElement

    expect(screen.getByTestId('sidebar')).toBeInTheDocument()
    expect(screen.getByTestId('macos-traffic-light-spacer')).toBeInTheDocument()
    expect(screen.queryByTestId('macos-settings-titlebar')).not.toBeInTheDocument()
    expect(card).toHaveAttribute('data-ui', 'app.content-frame')
    expect(card).toHaveClass('rounded-[10px]', 'border-[0.5px]')
    expect(column).toHaveClass('activity-bar-canvas', 'pt-1', 'pr-1', 'pb-1')
    expect(column).not.toHaveClass('pl-1')
    expect(content).toHaveAttribute('data-ui', 'app.content')
  })

  it('restores the tab that was active before Settings when the focused tab closes', () => {
    const workspaceTabs = [
      { id: 'first', isDormant: false, title: 'First', type: 'route' as const, url: '/app/chat' },
      { id: 'second', isDormant: false, title: 'Second', type: 'route' as const, url: '/app/files' },
      { id: 'third', isDormant: false, title: 'Third', type: 'route' as const, url: '/app/notes' }
    ]
    const settingsTab = {
      id: 'settings',
      isDormant: false,
      title: 'Settings',
      type: 'route' as const,
      url: '/settings/provider'
    }
    mocks.tabs = workspaceTabs
    mocks.activeTabId = 'first'
    const view = render(<AppShell />)

    mocks.tabs = [...workspaceTabs, settingsTab]
    mocks.activeTabId = settingsTab.id
    view.rerender(<AppShell />)

    act(() => {
      mocks.commandHandlers.get('tab.close')?.handler()
    })
    expect(mocks.closeTabs).not.toHaveBeenCalled()
    expect(document.querySelector('[data-ui="settings.overlay"]')).toHaveAttribute('data-state', 'closing')

    finishSettingsClose()

    expect(mocks.closeTabs).toHaveBeenCalledWith([settingsTab.id], 'first')
    expect(mocks.closeTab).not.toHaveBeenCalled()
  })

  it('restores the most recently accessed workspace tab when Settings is restored active', () => {
    const settingsTab = {
      id: 'settings',
      isDormant: false,
      lastAccessTime: 400,
      title: 'Settings',
      type: 'route' as const,
      url: '/settings/provider'
    }
    mocks.tabs = [
      { id: 'home', isDormant: true, lastAccessTime: 300, title: 'Home', type: 'route', url: '/app/chat' },
      { id: 'files', isDormant: true, lastAccessTime: 100, title: 'Files', type: 'route', url: '/app/files' },
      settingsTab,
      {
        id: 'translate',
        isDormant: true,
        lastAccessTime: 200,
        title: 'Translate',
        type: 'route',
        url: '/app/translate'
      }
    ]
    mocks.activeTabId = settingsTab.id

    render(<AppShell />)

    act(() => {
      mocks.commandHandlers.get('tab.close')?.handler()
    })
    finishSettingsClose()
    expect(mocks.closeTabs).toHaveBeenCalledWith([settingsTab.id], 'home')
  })

  it('blocks and dismisses global search while the Settings tab is focused', () => {
    mocks.tabs = [
      ...mocks.tabs,
      {
        id: 'settings',
        isDormant: false,
        title: 'Settings',
        type: 'route',
        url: '/settings/provider'
      }
    ]
    mocks.activeTabId = 'settings'

    render(<AppShell />)
    mocks.commandHandlers.get('app.search')?.handler()

    expect(mocks.showSearchPopup).not.toHaveBeenCalled()
    expect(mocks.hideSearchPopup).toHaveBeenCalledTimes(1)
  })

  it('keeps the Windows and Linux page inside the content column beside the sidebar', () => {
    const { container } = render(<AppShell />)

    const root = container.firstElementChild
    const sidebar = screen.getByTestId('sidebar')
    const main = contentMain()
    const tabRouter = screen.getByTestId('tab-router')

    if (!(root instanceof HTMLElement)) {
      throw new Error('Expected AppShell to render a root and content column')
    }

    const contentColumn = contentColumnContaining(root, main)

    expect(sidebar.parentElement).toBe(root)
    expect(contentColumn.parentElement).toBe(root)
    expect(contentColumn).toContainElement(main)
    expect(contentColumn).toContainElement(tabRouter)
    expect(contentColumn).toHaveAttribute('data-tauri-drag-region', 'true')
    expect(contentColumn.querySelector('[data-ui="app.tab-bar"]')).toBeNull()
    expect(contentColumn.querySelector('main')).toHaveAttribute('data-ui', 'app.content')
    expect(Array.from(root.children)).toEqual([sidebar, contentColumn])
  })

  it('places the image preview layer above the shell, outside the workspace isolate', () => {
    const { container } = render(<AppShell />)
    const portal = document.querySelector('[data-ui="app.preview-portal"]')
    const shell = container.firstElementChild

    expect(portal).toHaveClass('pointer-events-none', 'fixed', 'inset-0', 'z-30')
    expect(shell).not.toContainElement(portal)
    expect(document.querySelector('[data-ui="app.workspace-layer"]')).not.toContainElement(portal)
  })

  it('keeps the macOS traffic lights in the left column beside the tab/content column', () => {
    mocks.platformState.isMac = true

    const { container } = render(<AppShell />)

    const root = container.firstElementChild
    const sidebar = screen.getByTestId('sidebar')
    const main = contentMain()
    const tabRouter = screen.getByTestId('tab-router')
    const trafficLightSpacer = screen.getByTestId('macos-traffic-light-spacer')
    const trafficLightDragRegion = screen.getByTestId('macos-traffic-light-drag-region')
    const leftColumn = sidebar.parentElement

    if (!(root instanceof HTMLElement) || !(leftColumn instanceof HTMLElement)) {
      throw new Error('Expected AppShell to render macOS left and content columns')
    }

    const contentColumn = contentColumnContaining(root, main)

    expect(trafficLightDragRegion.parentElement).toBe(root)
    expect(trafficLightDragRegion).toHaveClass('absolute', 'top-0', 'left-0', 'h-11')
    expect(trafficLightDragRegion.style.width).toBe(
      'max(env(titlebar-area-x, 0px), var(--a3s-traffic-light-inset, 0px))'
    )
    expect(trafficLightDragRegion).not.toHaveClass('w-[env(titlebar-area-x)]')
    expect(leftColumn.parentElement).toBe(root)
    expect(leftColumn).not.toHaveClass('min-w-[88px]')
    expect(contentColumn.parentElement).toBe(root)
    expect(Array.from(leftColumn.children)).toEqual([trafficLightSpacer, sidebar])
    expect(contentColumn).toContainElement(main)
    expect(contentColumn).toContainElement(tabRouter)
    expect(contentColumn.querySelector('[data-ui="app.tab-bar"]')).toBeNull()
    expect(Array.from(root.children)).toEqual([trafficLightDragRegion, leftColumn, contentColumn])
  })

  it('reserves the traffic-light gutter when the shell set the inset without an Electron platform', () => {
    document.documentElement.style.setProperty('--a3s-traffic-light-inset', '80px')

    render(<AppShell />)

    expect(screen.getByTestId('macos-traffic-light-spacer')).toBeInTheDocument()
    expect(screen.getByTestId('macos-traffic-light-drag-region').style.width).toBe(
      'max(env(titlebar-area-x, 0px), var(--a3s-traffic-light-inset, 0px))'
    )
  })

  it('keeps the traffic-light gutter when fullscreen status is not a boolean', async () => {
    mocks.platformState.isMac = true
    mocks.ipcRequest.mockResolvedValue({ homePath: '', resourcesPath: '', pendingNotifications: [] })

    render(<AppShell />)

    expect(await screen.findByTestId('macos-traffic-light-spacer')).toBeInTheDocument()
    expect(screen.getByTestId('macos-traffic-light-drag-region')).toBeInTheDocument()
  })

  it('removes macOS traffic light placeholders when the window is fullscreen', async () => {
    mocks.platformState.isMac = true
    mocks.ipcRequest.mockResolvedValue(true)

    const { container } = render(<AppShell />)

    await waitFor(() => {
      expect(screen.queryByTestId('macos-traffic-light-spacer')).toBeNull()
    })

    const root = container.firstElementChild
    const sidebar = screen.getByTestId('sidebar')
    const main = contentMain()

    if (!(root instanceof HTMLElement)) {
      throw new Error('Expected AppShell to render a root and content column')
    }

    const contentColumn = contentColumnContaining(root, main)

    expect(mocks.ipcRequest).toHaveBeenCalledWith('window.is_full_screen')
    expect(screen.queryByTestId('macos-traffic-light-drag-region')).toBeNull()
    expect(sidebar.parentElement?.children).toHaveLength(1)
    expect(contentColumn.parentElement).toBe(root)
    expect(contentColumn.querySelector('[data-ui="app.tab-bar"]')).toBeNull()
  })

  it('updates macOS traffic light placeholders from fullscreen events', async () => {
    mocks.platformState.isMac = true

    render(<AppShell />)

    expect(await screen.findByTestId('macos-traffic-light-spacer')).toBeInTheDocument()

    act(() => {
      mocks.ipcHandlers.get('window.fullscreen_changed')?.(true)
    })

    await waitFor(() => {
      expect(screen.queryByTestId('macos-traffic-light-spacer')).toBeNull()
    })

    expect(screen.queryByTestId('macos-traffic-light-drag-region')).toBeNull()

    act(() => {
      mocks.ipcHandlers.get('window.fullscreen_changed')?.(false)
    })

    expect(await screen.findByTestId('macos-traffic-light-spacer')).toBeInTheDocument()
    expect(screen.getByTestId('macos-traffic-light-drag-region')).toBeInTheDocument()
  })

  it('clears the split state when the last mini-app tab closes', () => {
    MockUseCacheUtils.setCacheValue('mini_app.split_open', true)
    MockUseCacheUtils.setCacheValue('mini_app.split_id', 'right-app')
    mocks.tabs = [
      ...mocks.tabs,
      { id: 'mini-left', isDormant: false, title: 'Left', type: 'route', url: '/app/mini-app/left-app' }
    ]
    mocks.activeTabId = 'mini-left'

    render(<AppShell />)
    mocks.commandHandlers.get('tab.close')?.handler()

    // A surviving split would reopen the next mini app straight into the stale
    // pane and keep `right-app` pinned in the keep-alive pool.
    expect(MockUseCacheUtils.getCacheValue('mini_app.split_open')).toBe(false)
    expect(MockUseCacheUtils.getCacheValue('mini_app.split_id')).toBe('')
    expect(mocks.closeTab).toHaveBeenCalledWith('mini-left')
  })

  it('keeps the split state while another mini-app tab is still open', () => {
    MockUseCacheUtils.setCacheValue('mini_app.split_open', true)
    MockUseCacheUtils.setCacheValue('mini_app.split_id', 'right-app')
    mocks.tabs = [
      ...mocks.tabs,
      { id: 'mini-left', isDormant: false, title: 'Left', type: 'route', url: '/app/mini-app/left-app' },
      { id: 'mini-other', isDormant: false, title: 'Other', type: 'route', url: '/app/mini-app/other-app' }
    ]
    mocks.activeTabId = 'mini-left'

    render(<AppShell />)
    mocks.commandHandlers.get('tab.close')?.handler()

    // The remaining mini-app tab still renders the split, so collapsing it here
    // would drop the pane out from under the user.
    expect(MockUseCacheUtils.getCacheValue('mini_app.split_open')).toBe(true)
    expect(MockUseCacheUtils.getCacheValue('mini_app.split_id')).toBe('right-app')
  })

  it('leaves non mini-app tab closes untouched', () => {
    MockUseCacheUtils.setCacheValue('mini_app.split_open', true)
    MockUseCacheUtils.setCacheValue('mini_app.split_id', 'right-app')
    mocks.tabs = [...mocks.tabs, { id: 'files', isDormant: false, title: 'Files', type: 'route', url: '/app/files' }]

    render(<AppShell />)
    mocks.commandHandlers.get('tab.close')?.handler()

    expect(MockUseCacheUtils.getCacheValue('mini_app.split_open')).toBe(true)
    expect(MockUseCacheUtils.getCacheValue('mini_app.split_id')).toBe('right-app')
    expect(mocks.closeTab).toHaveBeenCalledWith('home')
  })

  it('cycles tabs via command handlers', () => {
    mocks.tabs = [
      ...mocks.tabs,
      { id: 'tab2', isDormant: false, title: 'Tab 2', type: 'route', url: '/app/files' },
      { id: 'tab3', isDormant: false, title: 'Tab 3', type: 'route', url: '/app/agents' }
    ]

    // home -> next -> tab2
    const { rerender } = render(<AppShell />)
    mocks.commandHandlers.get('tab.next')?.handler()
    expect(mocks.setActiveTab).toHaveBeenCalledWith('tab2')

    // tab3 -> next -> home
    mocks.activeTabId = 'tab3'
    rerender(<AppShell />)
    mocks.setActiveTab.mockClear()
    mocks.commandHandlers.get('tab.next')?.handler()
    expect(mocks.setActiveTab).toHaveBeenCalledWith('home')

    // tab2 -> prev -> home
    mocks.activeTabId = 'tab2'
    rerender(<AppShell />)
    mocks.setActiveTab.mockClear()
    mocks.commandHandlers.get('tab.prev')?.handler()
    expect(mocks.setActiveTab).toHaveBeenCalledWith('home')

    // home -> prev -> tab3
    mocks.activeTabId = 'home'
    rerender(<AppShell />)
    mocks.setActiveTab.mockClear()
    mocks.commandHandlers.get('tab.prev')?.handler()
    expect(mocks.setActiveTab).toHaveBeenCalledWith('tab3')
  })

  it('disables tab cycling commands when there is no reachable next tab', () => {
    const { rerender } = render(<AppShell />)

    expect(mocks.commandHandlers.get('tab.next')?.options).toEqual({ enabled: false })
    expect(mocks.commandHandlers.get('tab.prev')?.options).toEqual({ enabled: false })

    mocks.tabs = [...mocks.tabs, { id: 'tab2', isDormant: false, title: 'Tab 2', type: 'route', url: '/app/files' }]
    mocks.activeTabId = 'missing'
    rerender(<AppShell />)

    expect(mocks.commandHandlers.get('tab.next')?.options).toEqual({ enabled: false })
    expect(mocks.commandHandlers.get('tab.prev')?.options).toEqual({ enabled: false })

    mocks.activeTabId = 'home'
    rerender(<AppShell />)

    expect(mocks.commandHandlers.get('tab.next')?.options).toEqual({ enabled: true })
    expect(mocks.commandHandlers.get('tab.prev')?.options).toEqual({ enabled: true })
  })

  it('closes the active tab from the tab-close shortcut', () => {
    mocks.tabs = [...mocks.tabs, { id: 'tab2', isDormant: false, title: 'Tab 2', type: 'route', url: '/app/files' }]
    mocks.activeTabId = 'tab2'

    render(<AppShell />)
    mocks.commandHandlers.get('tab.close')?.handler()

    expect(mocks.closeTab).toHaveBeenCalledWith('tab2')
  })

  it('closes the window from the tab-close shortcut when only the last tab remains', () => {
    render(<AppShell />)
    mocks.commandHandlers.get('tab.close')?.handler()

    // Closing the final tab would strand the shell in the empty Launchpad state;
    // the shortcut must close the window instead, like the last-tab close in browsers.
    expect(mocks.ipcRequest).toHaveBeenCalledWith('window.close')
    expect(mocks.closeTab).not.toHaveBeenCalled()
  })

  it('disables the tab-close shortcut when no tab is active', () => {
    mocks.activeTabId = 'missing'

    render(<AppShell />)

    expect(mocks.commandHandlers.get('tab.close')?.options).toEqual({ enabled: false })
  })

  it('floats the icon-rail content card on the rail gray without a left gutter', () => {
    MockUseCacheUtils.setPersistCacheValue('ui.sidebar.width', 64)

    render(<AppShell />)

    const content = document.querySelector('main')
    const card = content?.parentElement?.parentElement
    const column = card?.parentElement
    expect(card).toHaveAttribute('data-ui', 'app.content-frame')
    expect(card).toHaveClass('rounded-[10px]', 'border-[0.5px]')
    expect(card?.querySelector('[data-ui="app.tab-bar"]')).toBeNull()
    expect(column).toHaveAttribute('data-tauri-drag-region', 'true')
    expect(column).toHaveClass('activity-bar-canvas', 'pt-1', 'pr-1', 'pb-1')
    expect(column).not.toHaveClass('p-1', 'pl-1')
    expect(content?.parentElement).not.toHaveClass('pr-2', 'pb-2')
    expect(content).not.toHaveClass('rounded-tr-[12px]', 'rounded-br-[12px]', 'rounded-[12px]')
  })

  it('keeps the content frame when the sidebar is wide', () => {
    MockUseCacheUtils.setPersistCacheValue('ui.sidebar.width', 240)

    render(<AppShell />)

    const content = document.querySelector('main')
    expect(content).toHaveClass('rounded-[12px]', 'border-[0.5px]')
    expect(content?.parentElement).toHaveClass('pb-2')
  })

  it('does not put a launchpad control on the icon-rail content card', () => {
    MockUseCacheUtils.setPersistCacheValue('ui.sidebar.width', 52)

    render(<AppShell />)

    const card = document.querySelector('[data-ui="app.content-frame"]')
    expect(card).toBeTruthy()
    expect(screen.queryByRole('button', { name: '启动台' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-launchpad-button]')).toBeNull()
    expect(document.querySelector('[data-ui="app.tab-bar"]')).toBeNull()
  })

  it('does not add a second launchpad control when the sidebar is expanded', () => {
    MockUseCacheUtils.setPersistCacheValue('ui.sidebar.width', 240)

    render(<AppShell />)

    expect(screen.queryByRole('button', { name: '启动台' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-launchpad-button]')).toBeNull()
    expect(document.querySelector('[data-ui="app.tab-bar"]')).toBeNull()
  })
})
