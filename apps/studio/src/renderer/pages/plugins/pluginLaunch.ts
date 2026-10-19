import { type AixAppletTarget, aixWindowViews, findAixWindow, openAixAppletWindow } from '@renderer/services/plugins'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import { HOME_APPLET_SLUG, WORKBENCH_APPLET_IDS, type WorkbenchAppletId } from '../agents/homeApplets'
import { openWorkbenchAppletWindow } from '../agents/openWorkbenchAppletWindow'
import { NOTICE_APPLET_ID } from '../agents/WorkbenchApplets'
import type { CapabilityEntry } from '../capabilities/capabilitiesData'
const NOTICE_SLUG = 'a3s-notice'

/** Catalog slug of a homepage mini app, when this plugin is one. */
export function workbenchAppletForPlugin(slug: string): WorkbenchAppletId | null {
  for (const id of WORKBENCH_APPLET_IDS) {
    if (HOME_APPLET_SLUG[id] === slug) return id
  }
  return null
}

/**
 * A homepage work mode may name its editor, and submitting that mode still opens it.
 * The plugin card does not, when that editor is the package's only window.
 */
function composerEditorOnly(catalogue: AixWorkbenchCatalogue, slug: string): boolean {
  const item = catalogue.packages.find((candidate) => candidate.slug === slug)
  const editors = new Set((item?.composerModes ?? []).flatMap((mode) => (mode.editor ? [mode.editor] : [])))
  if (editors.size === 0) return false
  const windows = aixWindowViews(item)
  return windows.length > 0 && windows.every((view) => editors.has(view.id))
}

/**
 * The shell can open this installed `ui` plugin: a homepage mini app, or an
 * installed AIX package's standalone webview. A package that only extends the
 * homepage composer stays closed here. Other catalog slugs stay closed.
 */
export function launchableUiPlugin(
  entry: Pick<CapabilityEntry, 'slug' | 'surfaces' | 'available'>,
  installed: boolean,
  catalogue?: AixWorkbenchCatalogue
): WorkbenchAppletId | typeof NOTICE_APPLET_ID | AixAppletTarget | null {
  if (!installed || entry.available === false || !entry.surfaces.includes('ui')) return null
  if (entry.slug === NOTICE_SLUG) return NOTICE_APPLET_ID
  const applet = workbenchAppletForPlugin(entry.slug)
  if (applet) return applet
  if (!catalogue || composerEditorOnly(catalogue, entry.slug)) return null
  return findAixWindow(catalogue, entry.slug)
}

/**
 * Open an installed `ui` plugin in its system mini-app window.
 * Skill, mcp, tool, and flow plugins are materialized by install.
 */
export async function openInstalledUiPlugin(
  entry: Pick<CapabilityEntry, 'slug' | 'name' | 'surfaces' | 'applet' | 'available'>,
  installed: boolean,
  catalogue?: AixWorkbenchCatalogue
): Promise<boolean> {
  const target = launchableUiPlugin(entry, installed, catalogue)
  if (!target) return false
  if (typeof target === 'object') return openAixAppletWindow(target.item.slug, target.view.id, entry.name)
  return openWorkbenchAppletWindow(target, entry.name, entry.applet)
}
