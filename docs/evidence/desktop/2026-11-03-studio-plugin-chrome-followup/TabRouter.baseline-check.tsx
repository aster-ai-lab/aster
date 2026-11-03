import { DialogPortalContainerProvider, PortalContainerProvider } from '@aster-ai-lab/ui'
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { Activity } from 'react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { RouteErrorFallback } from '@renderer/components/layout/RouteErrorFallback'
import { ForegroundImagePreviewPortal } from '@renderer/components/layout/shellPreviewPortal'
import { TabIdProvider } from '@renderer/components/layout/TabIdProvider'
import { routeTree } from '@renderer/routeTree.gen'
import { cancelPageNavigation, finishPageNavigation, startPageNavigation } from '/Users/roylin/code/aster/.tmp/studio-plugin-chrome-followup/before/src/renderer/services/pagePerformance.ts'
import type { AppRouter } from '@renderer/types/router'
import type { Tab } from '@shared/data/cache/cacheValueTypes'

import { PageEntry, ToWorkPageLoading } from '/Users/roylin/code/aster/apps/studio/src/renderer/components/layout/PageEntry.tsx'

function RoutePending() {
  const { t } = useTranslation()
  return <ToWorkPageLoading label={t('common.loading')} />
}

// The annotation keeps this in step with the registered `AppRouter`: options that change the
// router's type fail here instead of silently diverging from what pages are typed against.
const createTabRouter = (url: string): AppRouter =>
  createRouter({
    routeTree,
    history: createMemoryHistory({ initialEntries: [url] }),
    // defaultErrorComponent contains a route render error to its tab; without it the
    // error bubbles to the window-level boundary and tears down the whole window.
    defaultErrorComponent: RouteErrorFallback,
    defaultPendingComponent: RoutePending,
    defaultPendingMs: 120,
    defaultPendingMinMs: 0
  })

interface TabRouterProps {
  tab: Tab
  isActive: boolean
  onUrlChange: (url: string) => void
  /** Foreground tab only. A background tab keeps its preview inside the tab. */
  previewPortal?: boolean
}

/**
 * TabRouter - Independent MemoryRouter for each Tab
 *
 * Each tab maintains its own router instance with isolated history,
 * enabling true KeepAlive behavior via React 19's Activity component.
 */
export const TabRouter = ({ tab, isActive, onUrlChange, previewPortal = false }: TabRouterProps) => {
  // Create independent router instance per tab (only once)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const router = useMemo(() => createTabRouter(tab.url), [tab.id])

  // External retargets update tab.url before an async route can replace the outgoing page.
  // Cover that interval so teardown effects cannot repaint stale page loading UI.
  const [resolvedHref, setResolvedHref] = useState(tab.url)
  const transitionUrl = tab.url !== resolvedHref ? tab.url : null

  // Sync internal navigation back to tab state
  useEffect(() => {
    const unsubscribe = import.meta.env.DEV
      ? router.subscribe('onBeforeNavigate', ({ toLocation }) => {
          startPageNavigation(tab.id, toLocation.href, toLocation.pathname)
        })
      : undefined
    return () => {
      unsubscribe?.()
      cancelPageNavigation(tab.id)
    }
  }, [router, tab.id])

  useEffect(() => {
    return router.subscribe('onResolved', ({ toLocation }) => {
      const nextHref = toLocation.href
      finishPageNavigation(tab.id, nextHref)
      setResolvedHref(nextHref)
      if (nextHref !== tab.url) {
        onUrlChange(nextHref)
      }
    })
  }, [router, tab.id, tab.url, onUrlChange])

  // Navigate when tab.url changes externally (e.g., from Sidebar)
  useEffect(() => {
    const currentHref = router.state.location.href
    if (tab.url !== currentHref) {
      // Split path and query: a query-bearing href string in `to` loses the
      // search through validateSearch round-trips (route-dependent), so pass
      // the parsed query as the structured `search` param instead
      const [pathname, search] = tab.url.split('?')
      void router.navigate({
        to: pathname,
        search: search ? Object.fromEntries(new URLSearchParams(search)) : undefined
      })
    }
  }, [router, tab.url])

  const [tabPortalContainer, setTabPortalContainer] = useState<HTMLElement | null>(null)
  // Latch the captured node across Activity hide/show: a hidden tab detaches the ref
  // (node === null) while its DOM node lives on, and clearing the container would
  // un-scope a still-open overlay/PageSidePanel to a full-window document.body portal.
  const captureTabPortalContainer = useCallback((node: HTMLElement | null) => {
    if (node) setTabPortalContainer(node)
  }, [])

  return (
    <ForegroundImagePreviewPortal active={previewPortal}>
      <Activity mode={isActive ? 'visible' : 'hidden'}>
        <TabIdProvider tabId={tab.id}>
          {/* This tab's content root is the portal target for overlays and PageSidePanel
            scoped to the tab (`relative` anchors the scoped panel's absolute layout), so a
            background tab's still-open surface stays hidden with its owning tab. */}
          <div
            ref={captureTabPortalContainer}
            aria-hidden={!isActive || undefined}
            inert={!isActive || undefined}
            className="relative flex h-full min-h-0 w-full flex-1 flex-col">
            <PortalContainerProvider container={tabPortalContainer}>
              <DialogPortalContainerProvider container={tabPortalContainer}>
                <RouterProvider router={router} />
              </DialogPortalContainerProvider>
            </PortalContainerProvider>
            {transitionUrl && (
              <div data-testid="tab-route-transition-cover" className="absolute inset-0 z-50 bg-card" aria-busy="true">
                <PageEntry ready={false} id="route-transition" />
              </div>
            )}
          </div>
        </TabIdProvider>
      </Activity>
    </ForegroundImagePreviewPortal>
  )
}
