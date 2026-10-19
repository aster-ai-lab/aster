import { useEffect, useRef } from 'react'

import { DATA_PACKAGE_SLUG } from '@renderer/services/data/dataPackageCatalogue'
import { receiveAppletHomeDocument } from '@renderer/services/plugins'

const ANALYSIS_VIEW_ID = 'analysis'

export function DataPackageSurface({ kind }: { kind: 'browser' | 'analysis' }) {
  const rootRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const root = rootRef.current
    if (!root) return undefined
    let cancelled = false
    let unmount = () => {}
    void import('../../../../../use-registry/packages/data/ui/shared/mount.mjs').then((mod) => {
      if (cancelled || !root.isConnected) return
      unmount = mod.mount(root, kind) ?? (() => {})
    })
    return () => {
      cancelled = true
      unmount()
      root.replaceChildren()
    }
  }, [kind])

  return <div ref={rootRef} className="flex h-full min-h-0 w-full flex-1 flex-col" />
}

export function DataBrowserSurface() {
  return <DataPackageSurface kind="browser" />
}

/** Analysis workspace hosted in the data plugin's system window. */
export function DataAnalysisSurface() {
  useEffect(() => {
    const channel = new MessageChannel()
    const guest = channel.port2
    const accept = (event: MessageEvent) => {
      const data = event.data as { type?: unknown; delivery?: unknown; seed?: unknown } | null
      if (!data || data.type !== 'aix.view.seed' || typeof data.delivery !== 'number') return
      guest.postMessage({
        protocolVersion: 1,
        type: 'aix.view.home',
        delivery: data.delivery,
        applied: true,
        holdsWork: false,
        seed: data.seed
      })
    }
    guest.addEventListener('message', accept)
    guest.start()
    receiveAppletHomeDocument(DATA_PACKAGE_SLUG, ANALYSIS_VIEW_ID, channel.port1)
    channel.port1.start()
    return () => {
      guest.removeEventListener('message', accept)
      channel.port1.close()
      guest.close()
    }
  }, [])
  return <DataPackageSurface kind="analysis" />
}
