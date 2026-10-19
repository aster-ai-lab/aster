import { useLayoutEffect, useRef, useSyncExternalStore } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import type { AgentBrowserRuntimeService } from '@renderer/services/AgentBrowserRuntimeService'

import { anchorPresented } from './WebviewSurface'
import {
  launchWorkspaceBrowser,
  readNoticeWebviewCover,
  subscribeNoticeWebviewCover,
  workspaceBrowserUrl,
  type WorkspaceBrowserPlacement
} from './workspaceBrowser'

const logger = loggerService.withContext('A3sWorkspaceBrowser')

/**
 * Workspace browser for the Tauri shell.
 * The page is a child webview on the main window, placed over this anchor.
 */
export function A3sWorkspaceBrowser({
  sessionId,
  runtime
}: {
  sessionId: string
  runtime: AgentBrowserRuntimeService
}) {
  const { t } = useTranslation()
  const resource = useSyncExternalStore(runtime.subscribe, () => runtime.get(sessionId))
  const anchor = resource?.anchor ?? null
  const sourceUrl = resource?.sourceUrl ?? ''
  const reloadKey = reloadToken(resource?.reloadKey)
  const overlayInset = Math.max(0, resource?.overlayInset ?? 0)
  const covered = useSyncExternalStore(subscribeNoticeWebviewCover, readNoticeWebviewCover)
  const seen = useRef(false)
  const placed = useRef<string | null>(null)
  const last = useRef<WorkspaceBrowserPlacement | null>(null)
  const generation = useRef(0)

  useLayoutEffect(() => {
    const publish = (placement: WorkspaceBrowserPlacement) => {
      const key = placementKey(placement)
      if (placed.current === key) return
      const previous = last.current
      placed.current = key
      last.current = placement
      const addressChanged = previous?.url !== placement.url
      const opening = placement.visible && (addressChanged || previous?.reloadKey !== placement.reloadKey)
      const ticket = generation.current + 1
      generation.current = ticket
      // A move keeps the address and the load state the page already has.
      runtime.update(sessionId, {
        failed: false,
        ...(opening ? { loading: true } : {}),
        ...(addressChanged ? { url: placement.url } : {})
      })
      void launchWorkspaceBrowser(sessionId, placement).then(
        () => {
          if (generation.current !== ticket) return
          runtime.update(sessionId, {
            ready: true,
            failed: false,
            ...(addressChanged ? { url: placement.url } : {})
          })
        },
        () => {
          if (generation.current !== ticket) return
          placed.current = null
          logger.error('Could not open the workspace page')
          runtime.update(sessionId, { ready: true, loading: false, failed: true })
        }
      )
    }

    if (!anchor) {
      const previous = last.current
      if (seen.current && previous?.visible) publish({ ...previous, visible: false })
      return
    }

    const sync = () => {
      const visible = anchorPresented(anchor) && !covered
      if (visible) seen.current = true
      if (!seen.current) return
      const launchUrl = workspaceBrowserUrl(sourceUrl)
      if (!launchUrl) {
        runtime.update(sessionId, { ready: true, loading: false, failed: false })
        const previous = last.current
        if (previous?.visible) publish({ ...previous, visible: false })
        return
      }
      const rect = anchor.getBoundingClientRect()
      const height = Math.round(rect.height)
      const inset = Math.min(Math.round(overlayInset), Math.max(0, height - 48))
      publish({
        url: launchUrl,
        x: Math.round(rect.x),
        y: Math.round(rect.y) + inset,
        width: Math.round(rect.width),
        height: height - inset,
        visible,
        reloadKey
      })
    }

    sync()
    const observer = new MutationObserver(sync)
    const nodes: HTMLElement[] = []
    for (let node: HTMLElement | null = anchor; node; node = node.parentElement) nodes.push(node)
    for (const node of nodes) {
      observer.observe(node, {
        attributes: true,
        attributeFilter: ['style', 'hidden', 'data-right-pane-mode', 'data-right-pane-phase']
      })
    }
    const resizeObserver = typeof ResizeObserver === 'function' ? new ResizeObserver(sync) : undefined
    resizeObserver?.observe(anchor)
    window.addEventListener('resize', sync)
    document.addEventListener('scroll', sync, true)
    return () => {
      observer.disconnect()
      resizeObserver?.disconnect()
      window.removeEventListener('resize', sync)
      document.removeEventListener('scroll', sync, true)
    }
  }, [anchor, covered, overlayInset, reloadKey, runtime, sessionId, sourceUrl])

  if (!anchor || !resource) return null
  const launchUrl = workspaceBrowserUrl(sourceUrl)
  const phase = resource.failed ? 'unavailable' : !resource.ready ? 'opening' : launchUrl ? 'opened' : 'idle'
  const labelKey =
    phase === 'unavailable'
      ? 'webview.browser.a3s_unavailable'
      : phase === 'opening'
        ? 'webview.browser.a3s_opening'
        : 'webview.browser.a3s_idle'
  return createPortal(
    <div
      role={phase === 'unavailable' ? 'alert' : phase === 'opened' ? undefined : 'status'}
      aria-hidden={phase === 'opened' ? true : undefined}
      data-workspace-browser="a3s-webview"
      data-workspace-browser-phase={phase}
      className="grid h-full w-full place-items-center bg-white px-6 text-center text-[13px] text-[#60646c]">
      {phase === 'opened' ? null : t(labelKey)}
    </div>,
    anchor
  )
}

function reloadToken(value: unknown): number {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value)
    if (Number.isFinite(parsed)) return parsed
  }
  return 0
}

function placementKey(placement: WorkspaceBrowserPlacement): string {
  return [
    placement.url,
    placement.x,
    placement.y,
    placement.width,
    placement.height,
    placement.visible ? 1 : 0,
    placement.reloadKey
  ].join('\n')
}
