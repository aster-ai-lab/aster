import type { ReactNode } from 'react'
import { useCallback, useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { BrowserChrome } from '@renderer/components/BrowserChrome'
import { BrowserOverlays } from '@renderer/components/BrowserOverlays'
import type { WebviewAnnotationSavedPayload } from '@renderer/components/WebviewAnnotationControls'
import { WebviewImportBanner } from '@renderer/components/WebviewImportBanner'
import type { WebviewTag } from '@renderer/host/webviewTypes'
import {
  agentBrowserRuntimeService,
  type AgentBrowserRuntimeService
} from '@renderer/services/AgentBrowserRuntimeService'
import type { WebviewAnnotationTarget } from '@shared/types/webviewAnnotation'
import { WebviewSecurityProfile } from '@shared/utils/webviewSecurity'

import {
  hostUsesA3sWebview,
  workspaceBrowserNavigation,
  workspaceBrowserUrl,
  type WorkspaceBrowserNav
} from './workspaceBrowser'

const HISTORY_POLL_MS = 400

export interface SessionBrowserViewProps {
  runtime?: AgentBrowserRuntimeService
  sessionId: string
  initialUrl?: string
  securityProfile: WebviewSecurityProfile
  onNavigate: (url: string) => void
  target: WebviewAnnotationTarget
  isHostActive: boolean
  reloadKey?: number | string
  toolbarActions?: ReactNode
  onAnnotationSaved?: (payload: WebviewAnnotationSavedPayload) => void
}

export function SessionBrowserView({
  runtime: browserRuntime = agentBrowserRuntimeService,
  sessionId,
  initialUrl,
  securityProfile,
  onNavigate,
  target,
  isHostActive,
  reloadKey,
  toolbarActions,
  onAnnotationSaved
}: SessionBrowserViewProps) {
  const { t } = useTranslation()
  const resource = useSyncExternalStore(browserRuntime.subscribe, () => browserRuntime.get(sessionId))
  const configuredSource = useRef<string | undefined>(undefined)
  useEffect(() => {
    const key = `${sessionId}:${securityProfile}:${initialUrl}`
    if (configuredSource.current !== key || !browserRuntime.get(sessionId)) {
      configuredSource.current = key
      browserRuntime.ensure(sessionId, initialUrl, securityProfile)
    }
  }, [browserRuntime, sessionId, initialUrl, securityProfile])
  useEffect(() => {
    browserRuntime.update(sessionId, { reloadKey })
  }, [browserRuntime, sessionId, reloadKey, resource?.sessionId])
  const [anchor, setAnchor] = useState<HTMLDivElement | null>(null)
  useLayoutEffect(() => {
    browserRuntime.update(sessionId, { anchor: isHostActive ? anchor : null })
    return () => browserRuntime.update(sessionId, { anchor: null })
  }, [browserRuntime, sessionId, anchor, isHostActive, resource?.sessionId])
  const webviewRef = useRef<WebviewTag | null>(null)
  const [webviewRevision, setWebviewRevision] = useState(0)
  useLayoutEffect(() => {
    webviewRef.current = resource?.guest ?? null
    setWebviewRevision((revision) => revision + 1)
  }, [resource?.guest])
  const activeProfile = resource?.securityProfile ?? securityProfile
  const isReady = resource?.ready ?? false
  const embedded = hostUsesA3sWebview()
  const [history, setHistory] = useState({ canGoBack: false, canGoForward: false })
  const historyTicket = useRef(0)
  const applyNavigation = useCallback(
    (state: WorkspaceBrowserNav | null) => {
      if (!state) return
      setHistory((current) =>
        current.canGoBack === state.canGoBack && current.canGoForward === state.canGoForward
          ? current
          : { canGoBack: state.canGoBack, canGoForward: state.canGoForward }
      )
      const url = workspaceBrowserUrl(state.url)
      browserRuntime.update(sessionId, {
        ...(url ? { url } : {}),
        ...(state.title !== null ? { title: state.title } : {}),
        loading: state.loading
      })
    },
    [browserRuntime, sessionId]
  )
  useEffect(() => {
    if (!embedded || !isHostActive) return
    const ticket = historyTicket.current + 1
    historyTicket.current = ticket
    const read = () => {
      void workspaceBrowserNavigation('state').then((state) => {
        if (historyTicket.current === ticket) applyNavigation(state)
      })
    }
    read()
    const timer = window.setInterval(read, HISTORY_POLL_MS)
    return () => {
      historyTicket.current += 1
      window.clearInterval(timer)
    }
  }, [applyNavigation, embedded, isHostActive])
  const moveHistory = useCallback(
    (action: 'back' | 'forward' | 'stop') => {
      const ticket = historyTicket.current
      void workspaceBrowserNavigation(action).then((state) => {
        if (historyTicket.current !== ticket) return
        applyNavigation(state)
        window.setTimeout(() => {
          if (historyTicket.current !== ticket) return
          void workspaceBrowserNavigation('state').then((next) => {
            if (historyTicket.current === ticket) applyNavigation(next)
          })
        }, 200)
      })
    },
    [applyNavigation]
  )
  const setOverlayInset = useCallback(
    (overlayInset: number) => {
      browserRuntime.update(sessionId, { overlayInset })
    },
    [browserRuntime, sessionId]
  )

  return (
    <BrowserChrome
      webviewRef={webviewRef}
      webviewRevision={webviewRevision}
      initialUrl={resource?.url ?? initialUrl ?? 'about:blank'}
      pageTitle={resource?.title ?? ''}
      historyEnabled={activeProfile === WebviewSecurityProfile.AgentBrowser}
      onNavigate={onNavigate}
      isWebviewReady={isReady}
      isHostActive={isHostActive}
      onReload={
        embedded
          ? () => browserRuntime.update(sessionId, { reloadKey: Date.now(), loading: true, failed: false })
          : undefined
      }
      loading={Boolean(resource?.loading)}
      onStop={embedded ? () => moveHistory('stop') : undefined}
      tooltipPlacement={embedded ? 'top' : undefined}
      onMenuHeight={embedded ? setOverlayInset : undefined}
      navigationControls={
        embedded
          ? {
              canGoBack: history.canGoBack,
              canGoForward: history.canGoForward,
              onBack: () => moveHistory('back'),
              onForward: () => moveHistory('forward')
            }
          : undefined
      }
      target={target}
      onAnnotationSaved={onAnnotationSaved}
      toolbarActions={toolbarActions}
      banner={activeProfile === WebviewSecurityProfile.AgentBrowser && <WebviewImportBanner />}>
      <div ref={setAnchor} className="h-full w-full" />
      {resource?.overlays &&
        createPortal(
          <BrowserOverlays
            webviewRef={webviewRef}
            targetId={target.id}
            isReady={isReady}
            isLoading={resource.loading}
            errorMessage={
              resource.failed
                ? t(
                    activeProfile === WebviewSecurityProfile.AgentHtmlArtifact
                      ? 'webview.navigation.load_failed'
                      : 'webview.browser.load_failed'
                  )
                : undefined
            }
          />,
          resource.overlays
        )}
    </BrowserChrome>
  )
}
