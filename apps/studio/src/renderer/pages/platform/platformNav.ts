export const PLATFORM_MENUS = [
  'overview',
  'customers',
  'channels',
  'suppliers',
  'orders',
  'tickets',
  'services',
  'models',
  'workloads',
  'gateway',
  'fleet',
  'logs',
  'usage',
  'system',
  'signIn'
] as const

export type PlatformMenuId = (typeof PLATFORM_MENUS)[number]

export const PLATFORM_GROUPS: {
  id: 'overview' | 'customers' | 'business' | 'catalog' | 'runtime' | 'observe' | 'system'
  ids: PlatformMenuId[]
}[] = [
  { id: 'overview', ids: ['overview'] },
  { id: 'customers', ids: ['customers', 'channels', 'suppliers'] },
  { id: 'business', ids: ['orders', 'tickets'] },
  { id: 'catalog', ids: ['services', 'models'] },
  { id: 'runtime', ids: ['workloads', 'gateway', 'fleet'] },
  { id: 'observe', ids: ['logs', 'usage'] },
  { id: 'system', ids: ['system', 'signIn'] }
]

export const SERVICE_FACETS = ['agents', 'applets', 'connectors', 'skills', 'plugins'] as const
export const WORKLOAD_FACETS = ['workflows', 'functions', 'runtimes', 'cells'] as const
export const ORDER_FACETS = ['orders', 'subscriptions'] as const
export const FLEET_FACETS = ['fleet', 'topology'] as const
export const SYSTEM_FACETS = ['apps', 'bootstrap', 'runtime', 'debug', 'reference'] as const
export const REFERENCE_FACETS = ['industries', 'chains', 'tags', 'regions', 'organs', 'parks', 'universities'] as const
export const SIGN_IN_FACETS = ['oauth', 'captcha', 'sms'] as const
export const GATEWAY_FACETS = ['routes', 'platform', 'standards'] as const
export const USAGE_FACETS = ['customers', 'models'] as const
export const SMS_VIEWS = ['profiles', 'providers', 'signatures', 'templates', 'scenarios', 'records'] as const

export const PLATFORM_FACET_NAV: Partial<Record<PlatformMenuId, { facets: readonly string[]; prefix: string }>> = {
  orders: { facets: ORDER_FACETS, prefix: 'platform.orders' },
  services: { facets: SERVICE_FACETS, prefix: 'platform.services' },
  workloads: { facets: WORKLOAD_FACETS, prefix: 'platform.workloads' },
  gateway: { facets: GATEWAY_FACETS, prefix: 'platform.gateway' },
  fleet: { facets: FLEET_FACETS, prefix: 'platform.fleet' },
  usage: { facets: USAGE_FACETS, prefix: 'platform.usage' },
  system: { facets: SYSTEM_FACETS, prefix: 'platform.system' },
  signIn: { facets: SIGN_IN_FACETS, prefix: 'platform.signIn' }
}

/** Sidebar entries that own a sub-menu: menus with sections, and the sections that nest a third level. */
export const PLATFORM_EXPANDABLE: ReadonlySet<string> = new Set([
  ...Object.keys(PLATFORM_FACET_NAV),
  'system.reference',
  'signIn.sms'
])

const VIEW_KEY = 'a3s.studio.platform.view:v1'
const VIEW_EVENT = 'a3s-studio-platform-view'

type StoredView = { menu?: unknown; facets?: Record<string, unknown>; expanded?: unknown }
let transientView: StoredView | undefined

function readView(): StoredView {
  if (transientView) return transientView
  try {
    const raw = globalThis.localStorage?.getItem(VIEW_KEY)
    const parsed: unknown = raw ? JSON.parse(raw) : null
    return parsed && typeof parsed === 'object' ? (parsed as StoredView) : {}
  } catch {
    return {}
  }
}

function writeView(next: StoredView) {
  try {
    if (!globalThis.localStorage) throw new Error('storage unavailable')
    globalThis.localStorage.setItem(VIEW_KEY, JSON.stringify(next))
    transientView = undefined
  } catch {
    // Storage is optional: private windows and full quotas only lose the remembered view.
    transientView = next
  }
  globalThis.window?.dispatchEvent(new Event(VIEW_EVENT))
}

/** Keep the sidebar and in-page segments in sync, including another window's preference update. */
export function subscribePlatformView(listener: () => void) {
  const onStorage = (event: StorageEvent) => {
    if (event.key === VIEW_KEY || event.key === null) listener()
  }
  globalThis.window?.addEventListener(VIEW_EVENT, listener)
  globalThis.window?.addEventListener('storage', onStorage)
  return () => {
    globalThis.window?.removeEventListener(VIEW_EVENT, listener)
    globalThis.window?.removeEventListener('storage', onStorage)
  }
}

/** The menu the operator left last time; anything outside the closed set falls back to the overview. */
export function readPlatformMenu(): PlatformMenuId {
  const menu = readView().menu
  return PLATFORM_MENUS.find((id) => id === menu) ?? 'overview'
}

/** Opening a menu also expands its sub-menu, so the reader lands on the section they came for. */
export function writePlatformMenu(menu: PlatformMenuId) {
  const view = readView()
  writeView({ ...view, menu, expanded: [...withExpanded(readExpanded(view), menu, true)] })
}

function readExpanded(view: StoredView): ReadonlySet<string> {
  if (!Array.isArray(view.expanded)) return new Set([readPlatformMenu()].filter((id) => PLATFORM_EXPANDABLE.has(id)))
  return new Set(view.expanded.filter((id): id is string => typeof id === 'string' && PLATFORM_EXPANDABLE.has(id)))
}

function withExpanded(current: ReadonlySet<string>, key: string, open: boolean) {
  const next = new Set(current)
  if (open && PLATFORM_EXPANDABLE.has(key)) next.add(key)
  else next.delete(key)
  return next
}

/** Sub-menus the operator left open. Before any choice, only the current menu's sub-menu is open. */
export function readPlatformExpanded(): ReadonlySet<string> {
  return readExpanded(readView())
}

export function writePlatformExpanded(key: string, open: boolean) {
  const view = readView()
  writeView({ ...view, expanded: [...withExpanded(readExpanded(view), key, open)] })
}

/** The segment last chosen inside one page, checked against that page's own closed set. */
export function readPlatformFacet<T extends string>(scope: string, facets: readonly T[], fallback: T = facets[0]): T {
  const value = readView().facets?.[scope]
  return facets.find((facet) => facet === value) ?? fallback
}

export function writePlatformFacet(scope: string, facet: string) {
  const view = readView()
  writeView({ ...view, facets: { ...view.facets, [scope]: facet } })
}
