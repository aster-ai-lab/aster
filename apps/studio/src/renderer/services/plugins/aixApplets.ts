import type { AixWorkbenchCatalogue, AixWorkbenchPackage, AixWorkbenchView } from '@shared/types/aixWorkbench'

import { encodeAixWorkbenchRouteKey } from './aixWorkbenchIdentity'

export interface AixAppletTarget {
  readonly item: AixWorkbenchPackage
  readonly view: AixWorkbenchView
  /** Verified editor or container page key. */
  readonly page: string
}

function target(item: AixWorkbenchPackage, view: AixWorkbenchView): AixAppletTarget {
  const container = view.container !== null
  const key = encodeAixWorkbenchRouteKey({
    package: item,
    kind: container ? 'container' : 'view',
    localId: view.container ?? view.id
  })
  return { item, view, page: `aix.${item.slug}/${container ? 'page' : 'view'}.${key}` }
}

/** A package's mini app views: `studio.applets` without a view container, in declaration order. */
export function aixAppletViews(item: AixWorkbenchPackage | undefined): readonly AixWorkbenchView[] {
  return item?.views.filter((view) => view.point === 'studio.applets' && view.container === null) ?? []
}

/** The installed mini app a package opens, or a named one of its mini apps. */
export function findAixApplet(catalogue: AixWorkbenchCatalogue, slug: string, viewId?: string): AixAppletTarget | null {
  const item = catalogue.packages.find((candidate) => candidate.slug === slug)
  const view = aixAppletViews(item).find((candidate) => viewId === undefined || candidate.id === viewId)
  return item && view ? target(item, view) : null
}

/** Standalone windows use verified editor/container pages; custom editors require a document context. */
export function aixWindowViews(item: AixWorkbenchPackage | undefined): readonly AixWorkbenchView[] {
  return (
    item?.views.filter(
      (view) =>
        (view.container === null &&
          view.presentation !== 'tree' &&
          (view.point === 'studio.applets' || view.point === 'studio.editor')) ||
        (view.point === 'studio.sidebar' &&
          item.containers.some((container) => container.id === view.container && container.location === 'activitybar'))
    ) ?? []
  )
}

export function findAixWindow(catalogue: AixWorkbenchCatalogue, slug: string, viewId?: string): AixAppletTarget | null {
  const item = catalogue.packages.find((candidate) => candidate.slug === slug)
  const views = aixWindowViews(item)
  const view =
    viewId === undefined
      ? (views.find((candidate) => item?.composerModes?.some((mode) => mode.editor === candidate.id)) ??
        views.find((candidate) => candidate.container === null) ??
        views[0])
      : views.find((candidate) => candidate.id === viewId)
  return item && view ? target(item, view) : null
}

/** The mini app behind a published `applet` contribution's page. */
export function aixAppletForPage(catalogue: AixWorkbenchCatalogue, pluginId: string, pageId: string) {
  if (!pluginId.startsWith('aix.')) return null
  const item = catalogue.packages.find((candidate) => `aix.${candidate.slug}` === pluginId)
  if (!item) return null
  for (const view of aixAppletViews(item)) {
    const found = target(item, view)
    if (found.page === `${pluginId}/${pageId}`) return found
  }
  return null
}

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

/** Open an installed AIX mini app in its own system window. Returns false outside the Tauri shell. */
export async function openAixAppletWindow(slug: string, view: string, title: string): Promise<boolean> {
  const opened = await openAixAppletHome(slug, view, title)
  return opened.opened
}

/**
 * Open the mini app and stage one homepage seed.
 * `delivery` is the shell's id for that seed. Zero means the seed was not stored.
 */
export async function openAixAppletHome(
  slug: string,
  view: string,
  title: string,
  seed?: unknown
): Promise<{ opened: boolean; delivery: number }> {
  const invoke = tauriInvoke()
  if (!invoke) return { opened: false, delivery: 0 }
  const args: Record<string, unknown> = { slug, view, title }
  if (seed !== undefined) args.seed = JSON.stringify(seed)
  try {
    const delivery = await invoke('open_aix_applet_window', args)
    return { opened: true, delivery: typeof delivery === 'number' ? delivery : 0 }
  } catch {
    return { opened: false, delivery: 0 }
  }
}
