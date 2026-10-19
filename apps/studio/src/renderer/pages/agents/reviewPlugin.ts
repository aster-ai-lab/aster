import { useEffect, useState } from 'react'

import { refreshPluginCatalog } from '@renderer/pages/settings/DependenciesSettings/pluginCatalog'
import { readStudioRuntime } from '@shared/studioRuntime'

export interface ReviewPluginState {
  skill: boolean
  ui: boolean
  title: string
}

const CLOSED: ReviewPluginState = { skill: false, ui: false, title: '' }

let preview: ReviewPluginState | null = null

/** Tests pin the projection so the home strip does not wait on the shell. */
export function setReviewPluginForTests(next: ReviewPluginState | null) {
  preview = next
}

export function reviewPluginPreview(): ReviewPluginState | null {
  return preview
}

export async function fetchReviewPlugin(fetchImpl: typeof fetch = globalThis.fetch): Promise<ReviewPluginState> {
  const runtime = readStudioRuntime()
  if (!runtime?.apiBaseUrl || !runtime.token) return CLOSED
  try {
    const response = await fetchImpl(`${runtime.apiBaseUrl.replace(/\/$/, '')}/api/review-plugin`, {
      cache: 'no-store',
      headers: { authorization: `Bearer ${runtime.token}` }
    })
    if (!response.ok) return CLOSED
    const body: unknown = await response.json()
    if (!body || typeof body !== 'object') return CLOSED
    const record = body as { skill?: unknown; ui?: unknown; title?: unknown }
    return {
      skill: record.skill === true,
      ui: record.ui === true,
      title: typeof record.title === 'string' ? record.title : ''
    }
  } catch {
    return CLOSED
  }
}

export function useReviewPlugin(): ReviewPluginState {
  const seeded = reviewPluginPreview()
  const [state, setState] = useState<ReviewPluginState>(seeded ?? CLOSED)
  useEffect(() => {
    if (reviewPluginPreview()) return
    let live = true
    const load = () => {
      void fetchReviewPlugin().then(async (next) => {
        await refreshPluginCatalog()
        if (live) setState(next)
      })
    }
    load()
    window.addEventListener('focus', load)
    return () => {
      live = false
      window.removeEventListener('focus', load)
    }
  }, [])
  return reviewPluginPreview() ?? state
}
