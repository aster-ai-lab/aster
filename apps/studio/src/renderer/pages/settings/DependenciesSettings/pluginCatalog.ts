import { useSyncExternalStore } from 'react'

import { readStudioRuntime } from '@shared/studioRuntime'

import type { PluginRecord } from './pluginContext'

export interface PluginCatalog {
  status: 'idle' | 'ready' | 'unavailable'
  plugins: readonly PluginRecord[]
  directory: string | null
}

const EMPTY: PluginCatalog = { status: 'idle', plugins: [], directory: null }
let catalog: PluginCatalog = EMPTY
let inflight: Promise<void> | null = null
const listeners = new Set<() => void>()

function emit(next: PluginCatalog) {
  catalog = next
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function usePluginCatalog(): PluginCatalog {
  return useSyncExternalStore(
    subscribe,
    () => catalog,
    () => EMPTY
  )
}

export function resetPluginCatalogForTests() {
  catalog = EMPTY
  inflight = null
}

export function setPluginCatalogForTests(next: PluginCatalog) {
  catalog = next
  inflight = null
  for (const listener of listeners) listener()
}

interface CatalogRoot {
  path: string
  source: 'installed' | 'user'
}

function asPlugin(value: unknown): PluginRecord | null {
  if (!value || typeof value !== 'object') return null
  const record = value as Record<string, unknown>
  if (typeof record.name !== 'string' || !record.name) return null
  return {
    name: record.name,
    description: typeof record.description === 'string' ? record.description : '',
    source: record.source === 'installed' ? 'installed' : 'user'
  }
}

function asRoot(value: unknown): CatalogRoot | null {
  if (!value || typeof value !== 'object') return null
  const record = value as Record<string, unknown>
  if (typeof record.path !== 'string' || !record.path) return null
  return { path: record.path, source: record.source === 'installed' ? 'installed' : 'user' }
}

export function catalogPublishesReview(catalog: PluginCatalog): boolean {
  return catalog.status === 'ready' && catalog.plugins.some((plugin) => plugin.name === 'review')
}

/** Read the skill directories again after the review package is linked or removed. */
export function refreshPluginCatalog(): Promise<void> {
  if (inflight) return inflight
  catalog = { ...catalog, status: 'idle' }
  return ensurePluginCatalog()
}

/** Load the installed set once. Later callers share the same snapshot. */
export function ensurePluginCatalog(): Promise<void> {
  if (catalog.status === 'ready' || inflight) return inflight ?? Promise.resolve()
  const runtime = readStudioRuntime()
  if (!runtime) {
    emit({ status: 'unavailable', plugins: [], directory: null })
    return Promise.resolve()
  }
  inflight = fetch(`${runtime.apiBaseUrl.replace(/\/$/, '')}/api/local-skills`, {
    headers: { authorization: `Bearer ${runtime.token}` }
  })
    .then(async (response) => {
      if (!response.ok) {
        emit({ status: 'unavailable', plugins: [], directory: null })
        return
      }
      const body: unknown = await response.json()
      const record = body && typeof body === 'object' ? (body as { skills?: unknown; roots?: unknown }) : {}
      const plugins = Array.isArray(record.skills) ? record.skills.flatMap((item) => asPlugin(item) ?? []) : []
      const roots = Array.isArray(record.roots) ? record.roots.flatMap((item) => asRoot(item) ?? []) : []
      emit({
        status: 'ready',
        plugins,
        directory: roots.find((root) => root.source === 'installed')?.path ?? roots[0]?.path ?? null
      })
    })
    .catch(() => {
      emit({ status: 'unavailable', plugins: [], directory: null })
    })
    .finally(() => {
      inflight = null
    })
  return inflight
}
