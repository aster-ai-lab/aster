import { useCallback, useEffect, useMemo, useRef, useState, type AnimationEvent } from 'react'

import { useCache, usePersistCache } from '@data/hooks/useCache'
import { useCommandHandler } from '@renderer/hooks/command'
import { useTabs } from '@renderer/hooks/tab'
import useMacTransparentWindow from '@renderer/hooks/useMacTransparentWindow'
import { useNativeFullscreen } from '@renderer/hooks/useNativeFullscreen'
import { ipcApi } from '@renderer/ipc'
import { SettingsLeaveContext } from '@renderer/pages/settings/settingsLeave'
import { openSettingsTab } from '@renderer/services/mainWindowNavigation'
import { miniAppIdFromTabUrl } from '@renderer/utils/miniAppKeepAlive'
import { getDefaultRouteTitle, isPageTitledRoute } from '@renderer/utils/routeTitle'
import { temporarilyHiddenActivityBarReplacement } from '@renderer/utils/sidebar'
import { cn } from '@renderer/utils/style'
import { isSettingsPath } from '@shared/data/types/settingsPath'
import { MIN_WINDOW_HEIGHT, SECOND_MIN_WINDOW_WIDTH } from '@shared/utils/window'

import { BossRoomRuntime } from '../app/BossRoomRuntime'
import { MessageAttentionRuntime } from '../app/MessageAttentionRuntime'
import Sidebar from '../app/Sidebar'
import { createRecentRouteEntryFromTab, recordGlobalSearchRecentEntry } from '../GlobalSearch/globalSearchGroups'
import GlobalSearchPopup from '../GlobalSearch/GlobalSearchPopup'
import MiniAppTabsPool from '../MiniApp/MiniAppTabsPool'
import { PluginWorkbenchPanels, PluginWorkbenchSidebar } from '../plugins/PluginWorkbenchLayout'
import { ResourceViewSourceProvider } from '../ResourceViewSourceProvider'
import { getSidebarLayout } from '../Sidebar'
import { ShellPreviewPortalProvider } from './shellPreviewPortal'
import { TabRouter } from './TabRouter'
import { hasNativeTrafficLightInset, MACOS_TRAFFIC_LIGHT_INSET } from './trafficLights'

import './settingsOverlay.css'

// Routes whose pages stay usable below the global minimum window width.
const isCompactMinWidthRoute = (url?: string): boolean =>
  !!url && (url.startsWith('/app/chat') || url.startsWith('/app/agents'))

const prefersReducedMotion = (): boolean =>
  typeof window !== 'undefined' && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches === true

export const AppShell = () => {
  const [previewPortal, setPreviewPortal] = useState<HTMLElement | null>(null)
  const capturePreviewPortal = useCallback((node: HTMLElement | null) => {
    if (node) setPreviewPortal(node)
  }, [])
  const isMacTransparentWindow = useMacTransparentWindow()
  const { tabs, activeTabId, setActiveTab, closeTab, closeTabs, updateTab } = useTabs()
  const activeTab = useMemo(() => tabs.find((tab) => tab.id === activeTabId), [activeTabId, tabs])

  useEffect(() => {
    for (const tab of tabs) {
      if (tab.type !== 'route') continue
      const nextUrl = temporarilyHiddenActivityBarReplacement(tab.url)
      if (!nextUrl) continue
      updateTab(tab.id, {
        url: nextUrl,
        title: getDefaultRouteTitle(nextUrl),
        icon: undefined,
        metadata: undefined,
        lastAccessTime: Date.now()
      })
    }
  }, [tabs, updateTab])
  const canCycleTabs = tabs.length > 1 && !!activeTab
  const canCloseTab = !!activeTab
  const isSettingsTabActive = isSettingsPath(activeTab?.url)
  const [sidebarWidth] = usePersistCache('ui.sidebar.width')
  const isIconRail = getSidebarLayout(sidebarWidth) === 'icon'
  const previousWorkspaceTabIdRef = useRef<string | undefined>(undefined)
  if (activeTab && !isSettingsTabActive) {
    previousWorkspaceTabIdRef.current = activeTab.id
  } else if (isSettingsTabActive && !previousWorkspaceTabIdRef.current) {
    previousWorkspaceTabIdRef.current = tabs.reduce<(typeof tabs)[number] | undefined>((latest, tab) => {
      if (isSettingsPath(tab.url)) return latest
      return !latest || (tab.lastAccessTime ?? 0) > (latest.lastAccessTime ?? 0) ? tab : latest
    }, undefined)?.id
  }
  const isFullscreen = useNativeFullscreen()
  const [splitOpen, setSplitOpen] = useCache('mini_app.split_open')
  const [, setSplitMiniAppId] = useCache('mini_app.split_id')

  // Split state is window-wide and does not follow the last mini-app tab out, so
  // the next mini app would open into a stale split with its app still pooled.
  const clearSplitWithLastMiniAppTab = useCallback(
    (id: string, url: string | undefined) => {
      if (!splitOpen || !miniAppIdFromTabUrl(url)) return
      const hasOtherMiniAppTab = tabs.some(
        (candidate) => candidate.id !== id && miniAppIdFromTabUrl(candidate.url) !== null
      )
      if (hasOtherMiniAppTab) return
      setSplitOpen(false)
      setSplitMiniAppId('')
    },
    [setSplitMiniAppId, setSplitOpen, splitOpen, tabs]
  )

  const [settingsClosing, setSettingsClosing] = useState(false)
  const settingsCloseRequestedRef = useRef(false)
  const closeSettingsTab = useCallback(
    (id: string) => {
      settingsCloseRequestedRef.current = false
      setSettingsClosing(false)
      closeTabs([id], previousWorkspaceTabIdRef.current)
    },
    [closeTabs]
  )
  const handleCloseTab = useCallback(
    (id: string) => {
      const tab = tabs.find((candidate) => candidate.id === id)
      if (isSettingsPath(tab?.url)) {
        if (!settingsCloseRequestedRef.current && !prefersReducedMotion()) {
          settingsCloseRequestedRef.current = true
          setSettingsClosing(true)
          return
        }
        closeSettingsTab(id)
        return
      }
      clearSplitWithLastMiniAppTab(id, tab?.url)
      closeTab(id)
    },
    [clearSplitWithLastMiniAppTab, closeSettingsTab, closeTab, tabs]
  )

  const handleOpenGlobalSearch = useCallback(() => {
    if (isSettingsTabActive) return
    void GlobalSearchPopup.show()
  }, [isSettingsTabActive])

  // Pinned tabs join the same flat cycle, matching Chrome / VS Code Ctrl+Tab.
  const cycleTab = useCallback(
    (direction: 'next' | 'prev') => {
      if (tabs.length <= 1) return
      const currentIndex = tabs.findIndex((t) => t.id === activeTabId)
      if (currentIndex === -1) return

      const offset = direction === 'next' ? 1 : -1
      const nextIndex = (currentIndex + offset + tabs.length) % tabs.length
      setActiveTab(tabs[nextIndex].id)
    },
    [tabs, activeTabId, setActiveTab]
  )

  const handleCloseActiveTab = useCallback(() => {
    if (!activeTabId) return
    // Closing the last tab would strand the shell in the empty Launchpad state;
    // browsers close the window for the final tab (Safari / Chrome convention).
    if (tabs.length === 1) {
      void ipcApi.request('window.close')
      return
    }
    handleCloseTab(activeTabId)
  }, [activeTabId, handleCloseTab, tabs])

  useCommandHandler('app.search', handleOpenGlobalSearch)
  useCommandHandler('tab.close', handleCloseActiveTab, { enabled: canCloseTab })
  useCommandHandler('tab.next', () => cycleTab('next'), { enabled: canCycleTabs })
  useCommandHandler('tab.prev', () => cycleTab('prev'), { enabled: canCycleTabs })

  useEffect(() => {
    if (isSettingsTabActive) {
      GlobalSearchPopup.hide()
      return
    }
    settingsCloseRequestedRef.current = false
    setSettingsClosing(false)
  }, [isSettingsTabActive])

  // The compact minimum tracks the active tab's route here, at window level.
  // It must not live in the pages themselves: they sit inside <Activity>, whose
  // hide/show re-runs mount effects, so a per-page []-dep effect re-issues this
  // IPC pair on every tab switch.
  const activeTabAllowsCompactWidth = isCompactMinWidthRoute(activeTab?.url)
  useEffect(() => {
    if (!activeTabAllowsCompactWidth) return
    void ipcApi.request('window.main.set_minimum_size', { width: SECOND_MIN_WINDOW_WIDTH, height: MIN_WINDOW_HEIGHT })
    return () => {
      void ipcApi.request('window.main.reset_minimum_size')
    }
  }, [activeTabAllowsCompactWidth])

  const recordRouteVisit = useCallback((tab: typeof activeTab, lastAccessTime = tab?.lastAccessTime) => {
    if (!tab) return

    const entry = createRecentRouteEntryFromTab(tab, lastAccessTime)
    if (!entry) return

    recordGlobalSearchRecentEntry(entry)
  }, [])

  useEffect(() => {
    recordRouteVisit(activeTab)
  }, [activeTab, recordRouteVisit])

  // Sync internal navigation back to tab state. For route-titled tabs we also
  // refresh the title and clear the per-entity icon (it was supplied for a
  // specific URL, e.g. a mini-app logo on /app/mini-app/<id>, and no longer
  // applies once the user navigates elsewhere inside the tab). Chat / agent
  // tabs are page-titled — their HomePage/AgentPage owns title + icon (topic /
  // session name + assistant / agent emoji), so we only sync the url and leave
  // title/icon alone, or navigating between topics would wipe them.
  const handleUrlChange = (tabId: string, url: string) => {
    const isPageTitled = isPageTitledRoute(url)
    const tab = tabs.find((candidate) => candidate.id === tabId)
    const patch = isPageTitled
      ? { url, lastAccessTime: Date.now() }
      : {
          url,
          title: getDefaultRouteTitle(url),
          icon: undefined,
          lastAccessTime: Date.now(),
          metadata: undefined
        }
    updateTab(tabId, patch)

    if (tab) {
      recordRouteVisit({ ...tab, ...patch }, Date.now())
    }
  }

  const leaveSettings = useCallback(() => {
    if (!activeTabId) return
    handleCloseTab(activeTabId)
  }, [activeTabId, handleCloseTab])

  const toggleSettings = useCallback(() => {
    if (isSettingsTabActive) {
      leaveSettings()
      return
    }
    openSettingsTab()
  }, [isSettingsTabActive, leaveSettings])

  useCommandHandler('app.settings.open', toggleSettings)

  const settingsPhase = !isSettingsTabActive ? 'hidden' : settingsClosing ? 'closing' : 'open'
  const underlayTabId = settingsPhase === 'hidden' ? undefined : previousWorkspaceTabIdRef.current
  const finishSettingsClose = (event: AnimationEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return
    if (event.animationName !== 'settings-panel-out') return
    if (!activeTabId || !isSettingsTabActive) return
    closeSettingsTab(activeTabId)
  }
  const visibleTabs = tabs.filter((tab) => tab.type === 'route' && (!tab.isDormant || tab.id === underlayTabId))

  const contentArea = (
    <div className={cn('flex min-h-0 min-w-0 flex-1 flex-col', !isIconRail && 'pr-2 pb-2')}>
      <main
        data-ui="app.content"
        className={cn(
          'relative min-h-0 flex-1 overflow-hidden',
          isIconRail ? 'bg-background' : 'rounded-[12px] border-[0.5px] border-border bg-background'
        )}>
        {/* Isolate so in-flow z-1000 chrome (the workspace rail) cannot paint over settings. */}
        <div data-ui="app.workspace-layer" className="relative isolate h-full min-h-0 w-full">
          {/* Route tabs. Settings is not one of them: it floats over the page that was already open. */}
          <ResourceViewSourceProvider>
            {visibleTabs
              .filter((tab) => settingsPhase === 'hidden' || !isSettingsPath(tab.url))
              .map((tab) => (
                <TabRouter
                  key={tab.id}
                  tab={tab}
                  isActive={tab.id === activeTabId || tab.id === underlayTabId}
                  previewPortal={tab.id === activeTabId}
                  onUrlChange={(url) => handleUrlChange(tab.id, url)}
                />
              ))}
          </ResourceViewSourceProvider>

          {/* MiniApp keep-alive WebView pool — global, shared across modes */}
          <MiniAppTabsPool />
        </div>

        {settingsPhase !== 'hidden' && activeTab && isSettingsPath(activeTab.url) && (
          <div data-ui="settings.overlay" data-state={settingsPhase} className="settings-overlay absolute inset-0 z-40">
            <div className="settings-overlay-scrim absolute inset-0" />
            <div
              className="settings-overlay-panel absolute inset-0 flex min-h-0 flex-col overflow-hidden"
              onAnimationEnd={finishSettingsClose}>
              <TabRouter
                key={activeTab.id}
                tab={activeTab}
                isActive
                previewPortal
                onUrlChange={(url) => handleUrlChange(activeTab.id, url)}
              />
            </div>
          </div>
        )}
      </main>
      <PluginWorkbenchPanels />
    </div>
  )

  const contentSurface = isIconRail ? (
    <div
      data-ui="app.content-frame"
      // This window's drawn corner is 14px. The card is inset 4px, so 10px
      // keeps the two curves on the same center.
      className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-[10px] border-[0.5px] border-border bg-background">
      {contentArea}
    </div>
  ) : (
    contentArea
  )

  const contentColumn = (
    <div
      // Self only: drag.js ignores clicks on children. The native strip still
      // uses this box so buttons that sit in the top 44px stay clickable.
      data-tauri-drag-region="true"
      className={cn(
        'flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden',
        // Top, right, and bottom stay inset 4px on the rail gray. The left side
        // meets the rail, so the gray beside an icon matches the gray on its left.
        isIconRail && 'activity-bar-canvas pt-1 pr-1 pb-1'
      )}>
      {contentSurface}
    </div>
  )

  const shell = !hasNativeTrafficLightInset() ? (
    <div
      className={cn(
        'flex h-screen w-screen flex-row overflow-hidden text-foreground',
        isMacTransparentWindow ? 'bg-transparent' : 'bg-sidebar'
      )}>
      <Sidebar isFullscreen={isFullscreen} />
      <PluginWorkbenchSidebar />
      {contentColumn}
    </div>
  ) : (
    <div
      className={cn(
        'relative flex h-screen w-screen flex-row overflow-hidden text-foreground',
        isMacTransparentWindow ? 'bg-transparent' : 'bg-sidebar'
      )}>
      {!isFullscreen && (
        <div
          aria-hidden="true"
          data-testid="macos-traffic-light-drag-region"
          style={{ width: MACOS_TRAFFIC_LIGHT_INSET }}
          className="pointer-events-none absolute top-0 left-0 h-11 [-webkit-app-region:drag]"
        />
      )}
      <div className="flex h-full min-h-0 shrink-0 flex-col [&>#app-sidebar]:min-h-0 [&>#app-sidebar]:flex-1">
        {!isFullscreen && (
          <div
            aria-hidden="true"
            data-testid="macos-traffic-light-spacer"
            data-tauri-drag-region="true"
            className="h-11 shrink-0 [-webkit-app-region:drag]"
          />
        )}
        <Sidebar isFullscreen={isFullscreen} />
      </div>
      <PluginWorkbenchSidebar />
      {contentColumn}
    </div>
  )

  return (
    <SettingsLeaveContext value={leaveSettings}>
      <ShellPreviewPortalProvider container={previewPortal}>
        <MessageAttentionRuntime />
        <BossRoomRuntime />
        {shell}
        {/* Above the ActivityBar. Hidden tabs do not portal into this node. */}
        <div
          ref={capturePreviewPortal}
          data-ui="app.preview-portal"
          className="pointer-events-none fixed inset-0 z-30"
        />
      </ShellPreviewPortalProvider>
    </SettingsLeaveContext>
  )
}
