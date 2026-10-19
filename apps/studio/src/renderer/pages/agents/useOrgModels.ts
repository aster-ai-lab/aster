import { useEffect, useState } from 'react'

import { fetchOrgModels, type OrgModelCatalog } from '@renderer/data/orgModels'

export type OrgModelsPhase = 'idle' | 'loading' | 'ready' | 'error'

const EMPTY: OrgModelCatalog = { items: [] }

/** The organization's published models while `enabled`; refetched when `revision` changes. */
export function useOrgModels({ enabled, revision = 0 }: { enabled: boolean; revision?: number }) {
  const [catalog, setCatalog] = useState<OrgModelCatalog>(EMPTY)
  const [phase, setPhase] = useState<OrgModelsPhase>('idle')

  useEffect(() => {
    if (!enabled) return
    const controller = new AbortController()
    setPhase('loading')
    fetchOrgModels(controller.signal).then(
      (next) => {
        if (controller.signal.aborted) return
        setCatalog(next)
        setPhase('ready')
      },
      () => {
        if (controller.signal.aborted) return
        setCatalog(EMPTY)
        setPhase('error')
      }
    )
    return () => controller.abort()
  }, [enabled, revision])

  return { catalog, phase }
}
