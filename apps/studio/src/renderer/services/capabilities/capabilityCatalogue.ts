import { projectAgentParameterSchema } from '@a3s-lab/intelligent-ui/openui'

import { memberRequest, MemberOrgError } from '@renderer/data/memberOrgs'
import { aixDeclarationFromUnknown, type AixAgentDeclaration } from '@renderer/services/agents/aixAgentRuntime'
import { catalogIconPath } from '@renderer/services/capabilityIcon'

export type CapabilityKind = 'connector' | 'skill' | 'plugin'

export type CapabilityCategoryId =
  | 'office'
  | 'creative'
  | 'design'
  | 'code'
  | 'business'
  | 'productivity'
  | 'science'
  | 'education'
  | 'search'
  | 'data'
  | 'other'

/** `featured` is the pseudo-section that shows every category with the curated picks on top. */
export type CapabilitySectionId = 'featured' | CapabilityCategoryId

/** A value the member types on this device. The catalog sends the shape, never a value. */
export interface CapabilityConfigField {
  key: string
  description: string
  required: boolean
  secret: boolean
}

/** How the shell materializes an installed connector. */
export interface CapabilityInstall {
  transport: 'stdio' | 'streamable_http' | 'sse'
  url?: string
  command?: string
  args: string[]
}

export interface CapabilityEntry {
  packageType?: 'native' | 'aix'
  available?: boolean
  slug: string
  kind: CapabilityKind
  category: CapabilityCategoryId
  /** Proper noun rendered as-is; never translated. */
  name: string
  description: string
  /** Tile color for the letter shown when the catalog has no logo. */
  color: string
  featured: boolean
  /** First-party office applet. It installs when opened and cannot be removed. */
  builtin?: boolean
  installed: boolean
  /** Shell path, for example `/api/capabilities/notion/icon?v=`. */
  iconUrl?: string
  /** Shell path for a plugin cover, for example `/api/capabilities/a3s-review/cover?v=`. */
  coverUrl?: string
  /** Present for connectors the catalog could describe. */
  install?: CapabilityInstall
  /** Empty when the connector needs nothing from the member. */
  config: CapabilityConfigField[]
  /** Skill package version. Absent when the catalog did not declare one. */
  version?: string
  /** Archive offered by the catalog, compared with the device's active archive. */
  packageDigest?: string
  /** Skill shortcuts. Absent when the skill declares none. */
  prompts?: string[]
  /** Package surfaces. `ui` plus an applet contract is a mini app; other `ui` packages are plugins. */
  surfaces: string[]
  /** Present when the package declares a mini-app window or permissions. */
  applet?: AppletContract
  /** Closed agent call schema. Absent when the package did not declare one. */
  parametersSchema?: string
  /** Edge, cloud, or remote run. Absent when the package did not declare one. */
  agentRuntime?: AixAgentDeclaration
}

export interface AppletWindowSize {
  defaultWidth: number
  defaultHeight: number
  maxWidth: number
  maxHeight: number
}

export interface AppletContract {
  window?: AppletWindowSize
  permissions: Array<'user' | 'model' | 'knowledge'>
}

export interface CapabilityCategory {
  id: CapabilityCategoryId
  labelKey: string
}

export interface CapabilitySection {
  id: CapabilitySectionId
  /** Category sections carry their label key; the featured section's label depends on the active kind. */
  labelKey?: string
  entries: CapabilityEntry[]
}

export const CAPABILITY_CATEGORIES: readonly CapabilityCategory[] = [
  { id: 'office', labelKey: 'capabilities.category.office' },
  { id: 'creative', labelKey: 'capabilities.category.creative' },
  { id: 'design', labelKey: 'capabilities.category.design' },
  { id: 'code', labelKey: 'capabilities.category.code' },
  { id: 'business', labelKey: 'capabilities.category.business' },
  { id: 'productivity', labelKey: 'capabilities.category.productivity' },
  { id: 'science', labelKey: 'capabilities.category.science' },
  { id: 'education', labelKey: 'capabilities.category.education' },
  { id: 'search', labelKey: 'capabilities.category.search' },
  { id: 'data', labelKey: 'capabilities.category.data' },
  { id: 'other', labelKey: 'capabilities.category.other' }
]

export const CAPABILITY_PAGE_SIZE = 24

export interface CapabilityQuery {
  kind: CapabilityKind
  q?: string
  /** Stored category id from the catalog. Not limited to the client enum. */
  category?: string
  featured?: boolean
  installed?: boolean
  page?: number
  pageSize?: number
}

/** One category that has enabled rows. Order is the catalog's, most used first. */
export interface CapabilityCategoryCount {
  id: string
  count: number
}

export interface CapabilityPage {
  items: CapabilityEntry[]
  total: number
  page: number
  pageSize: number
  categories: CapabilityCategoryCount[]
}

const CATEGORY_IDS = new Set<string>(CAPABILITY_CATEGORIES.map((category) => category.id))

/** One page of enabled capabilities. The shell forwards this to the gateway. */
export async function listCapabilities(
  query: CapabilityQuery,
  fetchImpl: typeof fetch = fetch
): Promise<CapabilityPage> {
  const params = new URLSearchParams()
  params.set('kind', query.kind)
  const q = query.q?.trim()
  if (q) params.set('q', [...q].slice(0, 120).join(''))
  if (query.category) params.set('category', query.category)
  if (query.featured) params.set('featured', 'true')
  if (query.installed) params.set('installed', 'true')
  params.set('page', String(query.page && query.page > 0 ? query.page : 1))
  params.set('pageSize', String(query.pageSize && query.pageSize > 0 ? query.pageSize : CAPABILITY_PAGE_SIZE))
  const data = await memberRequest(`/api/capabilities?${params.toString()}`, { method: 'GET' }, fetchImpl)
  if (!data || typeof data !== 'object' || Array.isArray(data)) throw new MemberOrgError('invalid')
  const record = data as Record<string, unknown>
  const items = Array.isArray(record.items)
    ? record.items.flatMap((row) => {
        const entry = asEntry(row)
        return entry ? [entry] : []
      })
    : []
  return {
    items,
    total: typeof record.total === 'number' && record.total >= 0 ? record.total : items.length,
    page: typeof record.page === 'number' && record.page > 0 ? record.page : 1,
    pageSize: typeof record.pageSize === 'number' && record.pageSize > 0 ? record.pageSize : CAPABILITY_PAGE_SIZE,
    categories: asCategories(record.categories)
  }
}

/** Known shelf ids keep their label key. Anything else is shown as the backend sent it. */
export function capabilityCategoryLabelKey(id: string): string | undefined {
  return CAPABILITY_CATEGORIES.find((category) => category.id === id)?.labelKey
}

function asCategories(value: unknown): CapabilityCategoryCount[] {
  if (!Array.isArray(value)) return []
  const seen = new Set<string>()
  const categories: CapabilityCategoryCount[] = []
  for (const row of value) {
    if (!row || typeof row !== 'object') continue
    const record = row as Record<string, unknown>
    const id = typeof record.id === 'string' ? record.id.trim() : ''
    if (!id || seen.has(id) || id === 'all' || id === 'featured') continue
    const count = typeof record.count === 'number' && Number.isFinite(record.count) ? record.count : 0
    if (count <= 0) continue
    seen.add(id)
    categories.push({ id, count })
  }
  return categories
}

function asEntry(value: unknown): CapabilityEntry | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.slug !== 'string' || !row.slug) return null
  if (row.kind !== 'connector' && row.kind !== 'skill' && row.kind !== 'plugin') return null
  if (row.kind === 'plugin' && !pluginInstallOk(row.install)) return null
  if (typeof row.name !== 'string' || !row.name.trim()) return null
  const category = typeof row.category === 'string' && CATEGORY_IDS.has(row.category) ? row.category : 'other'
  const packageType =
    row.kind === 'plugin' ? ((row.install as { packageType?: 'native' | 'aix' }).packageType ?? 'native') : undefined
  const install = asInstall(row.install)
  const iconUrl = iconPath(row.iconUrl)
  const coverUrl = iconPath(row.coverUrl)
  return {
    slug: row.slug,
    kind: row.kind,
    category: category as CapabilityCategoryId,
    name: row.name,
    description: typeof row.description === 'string' ? row.description : '',
    color: typeof row.color === 'string' && /^#[0-9A-Fa-f]{6}$/.test(row.color) ? row.color : '#171717',
    featured: row.featured === true,
    builtin: row.builtin === true,
    installed: row.installed === true,
    ...(row.kind === 'plugin' ? { packageDigest: (row.install as { digest: string }).digest } : {}),
    config: asConfig(row.config),
    ...(iconUrl ? { iconUrl } : {}),
    ...(coverUrl ? { coverUrl } : {}),
    ...(install ? { install } : {}),
    ...asVersion(row.version),
    ...asPrompts(row.prompts),
    surfaces: asSurfaces(row.surfaces),
    ...asParameterSchema(row.parametersSchema),
    ...asAgentRuntime(row.agentRuntime),
    ...(packageType ? { packageType, available: (row.install as { available?: boolean }).available !== false } : {}),
    ...asApplet(row.applet)
  }
}

const APPLET_PERMISSIONS = ['user', 'model', 'knowledge'] as const

function asApplet(value: unknown): { applet: AppletContract } | Record<string, never> {
  if (value == null) return {}
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {}
  const row = value as Record<string, unknown>
  const permissions = asAppletPermissions(row.permissions)
  if (!permissions) return {}
  if (row.window == null && permissions.length === 0) return {}
  const window = row.window == null ? undefined : asAppletWindow(row.window)
  if (row.window != null && !window) return {}
  return { applet: { permissions, ...(window ? { window } : {}) } }
}

function asAppletPermissions(value: unknown): AppletContract['permissions'] | null {
  if (value == null) return []
  if (!Array.isArray(value)) return null
  const permissions: AppletContract['permissions'] = []
  for (const item of value) {
    if (typeof item !== 'string' || !APPLET_PERMISSIONS.includes(item as AppletContract['permissions'][number])) {
      return null
    }
    if (permissions.includes(item as AppletContract['permissions'][number])) return null
    permissions.push(item as AppletContract['permissions'][number])
  }
  return APPLET_PERMISSIONS.filter((name) => permissions.includes(name))
}

function asAppletWindow(value: unknown): AppletWindowSize | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const row = value as Record<string, unknown>
  const defaultWidth = pixel(row.defaultWidth, 320, 3840)
  const defaultHeight = pixel(row.defaultHeight, 400, 2160)
  const maxWidth = pixel(row.maxWidth, 320, 3840)
  const maxHeight = pixel(row.maxHeight, 400, 2160)
  if (defaultWidth == null || defaultHeight == null || maxWidth == null || maxHeight == null) return null
  if (defaultWidth > maxWidth || defaultHeight > maxHeight) return null
  return { defaultWidth, defaultHeight, maxWidth, maxHeight }
}

function pixel(value: unknown, min: number, max: number): number | null {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < min || value > max) return null
  return value
}

const SURFACE_IDS = new Set(['skill', 'mcp', 'tool', 'ui', 'flow', 'okf', 'agent', 'plugin'])

function asSurfaces(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  const surfaces: string[] = []
  for (const item of value) {
    if (typeof item !== 'string' || !SURFACE_IDS.has(item) || surfaces.includes(item)) continue
    surfaces.push(item)
    if (surfaces.length >= 8) break
  }
  return surfaces
}

function asVersion(value: unknown): { version: string } | Record<string, never> {
  if (typeof value !== 'string') return {}
  const version = value.trim().slice(0, 64)
  return version ? { version } : {}
}

function asAgentRuntime(value: unknown) {
  const agentRuntime = aixDeclarationFromUnknown(value)
  return agentRuntime ? { agentRuntime } : {}
}

function asParameterSchema(value: unknown): { parametersSchema: string } | Record<string, never> {
  if (typeof value !== 'string' || value.length === 0 || value.length > 65536) return {}
  return projectAgentParameterSchema(value).ok ? { parametersSchema: value } : {}
}

function asPrompts(value: unknown): { prompts: string[] } | Record<string, never> {
  if (!Array.isArray(value)) return {}
  const prompts: string[] = []
  for (const item of value) {
    if (typeof item !== 'string') continue
    const prompt = item
      .trim()
      .replace(/[\u0000-\u001f]/g, '')
      .slice(0, 120)
    if (!prompt || prompts.includes(prompt)) continue
    prompts.push(prompt)
    if (prompts.length >= 8) break
  }
  return prompts.length > 0 ? { prompts } : {}
}

const CONFIG_KEY = /^[A-Za-z_][A-Za-z0-9_-]{0,63}$/

function asConfig(value: unknown): CapabilityConfigField[] {
  if (!Array.isArray(value)) return []
  const fields: CapabilityConfigField[] = []
  const seen = new Set<string>()
  for (const row of value) {
    if (!row || typeof row !== 'object') continue
    const record = row as Record<string, unknown>
    const key = typeof record.key === 'string' ? record.key : ''
    if (!CONFIG_KEY.test(key) || seen.has(key)) continue
    seen.add(key)
    fields.push({
      key,
      description: typeof record.description === 'string' ? record.description : '',
      required: record.required === true,
      secret: record.secret === true
    })
    if (fields.length >= 32) break
  }
  return fields
}

function pluginInstallOk(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false
  const row = value as Record<string, unknown>
  if (row.type !== 'plugin') return false
  if (row.available !== undefined && typeof row.available !== 'boolean') return false
  if (row.packageType !== undefined && row.packageType !== 'native' && row.packageType !== 'aix') return false
  if (row.packageType === 'aix' && (typeof row.packageId !== 'string' || !/^use\/[^/]+\/[^/]+$/.test(row.packageId)))
    return false
  if (typeof row.digest !== 'string' || !/^sha256:[a-f0-9]{64}$/.test(row.digest)) return false
  if (typeof row.size !== 'number' || !Number.isFinite(row.size) || row.size <= 0) return false
  return packageIdOk(row.packageId)
}

function packageIdOk(value: unknown): boolean {
  if (typeof value !== 'string' || value.length < 3 || value.length > 200) return false
  const segments = value.split('/')
  if (segments.length < 2) return false
  return segments.every(
    (segment) => segment.length > 0 && segment !== '.' && segment !== '..' && /^[A-Za-z0-9._-]+$/.test(segment)
  )
}

function asInstall(value: unknown): CapabilityInstall | undefined {
  if (!value || typeof value !== 'object') return undefined
  const row = value as Record<string, unknown>
  if (row.type !== 'mcp') return undefined
  const args = Array.isArray(row.args)
    ? row.args.filter((item): item is string => typeof item === 'string').slice(0, 32)
    : []
  if (row.transport === 'stdio' && typeof row.command === 'string' && row.command.trim()) {
    return { transport: 'stdio', command: row.command, args }
  }
  if (
    (row.transport === 'streamable_http' || row.transport === 'sse') &&
    typeof row.url === 'string' &&
    /^https?:\/\//.test(row.url)
  ) {
    return { transport: row.transport, url: row.url, args: [] }
  }
  return undefined
}

const iconPath = catalogIconPath
