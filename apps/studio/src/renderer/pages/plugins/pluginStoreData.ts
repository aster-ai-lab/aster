import type { CapabilityQuery } from '../capabilities/capabilitiesData'

export const PLUGIN_ALL = 'all'
export const PLUGIN_FEATURED = 'featured'

/** The member catalog caps one page at 100. */
export const PLUGIN_PAGE_SIZE = 100

/**
 * This page lists packages the type classifier names as a plugin, applet,
 * agent, skill, connector, tool, flow, or knowledge pack. Agent packages extend
 * the composer and do not need a standalone UI surface.
 */
export function pluginCatalogEntry(entry: {
  builtin?: boolean
  surfaces?: readonly string[]
  applet?: { permissions: readonly string[] } | null
}): boolean {
  if (entry.builtin === true) return false
  const type = pluginTypeKey(entry.surfaces, entry.applet)
  return type !== null
}

export function pluginQuery(input: { view: 'market' | 'installed'; category: string; q: string }): CapabilityQuery {
  const q = input.q.trim()
  const category = input.category !== PLUGIN_ALL && input.category !== PLUGIN_FEATURED ? input.category : undefined
  return {
    kind: 'plugin',
    pageSize: PLUGIN_PAGE_SIZE,
    ...(q ? { q } : {}),
    ...(category ? { category } : {}),
    ...(input.category === PLUGIN_FEATURED ? { featured: true } : {}),
    ...(input.view === 'installed' ? { installed: true } : {})
  }
}

export function pluginTypeKey(
  surfaces: readonly string[] | undefined,
  applet?: { permissions: readonly string[] } | null
): string | null {
  const list = surfaces ?? []
  if (list.includes('ui')) return applet ? 'plugins.type.applet' : 'plugins.type.plugin'
  if (list.includes('agent')) return 'plugins.type.agent'
  if (list.includes('skill')) return 'plugins.type.skill'
  if (list.includes('mcp')) return 'plugins.type.mcp'
  if (list.includes('tool')) return 'plugins.type.tool'
  if (list.includes('flow')) return 'plugins.type.flow'
  if (list.includes('okf')) return 'plugins.type.okf'
  if (list.includes('plugin')) return 'plugins.type.plugin'
  return null
}
