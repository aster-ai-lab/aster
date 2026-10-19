/**
 * Frozen descriptor the Tauri shell injects into an office applet window.
 * It carries a ticket for that one applet. It does not carry the shell bearer.
 */
export const APPLET_PERMISSIONS = ['user', 'model', 'knowledge'] as const

export type AppletPermission = (typeof APPLET_PERMISSIONS)[number]

export interface AppletRuntimeDescriptor {
  apiBaseUrl: string
  appletTicket: string
  appId: string
  tenant: string
  /** Empty when the package declares no permissions. */
  permissions: AppletPermission[]
}

const LOOPBACK_ORIGIN = /^http:\/\/127\.0\.0\.1:\d{1,5}$/
const TICKET = /^[a-f0-9]{64}$/
const APP_ID =
  /^(approval|leave|travel|expense|meeting|seal|notice|pdf|review|inbox|plan|probation|clock|correction|ai-resource)$/
const TENANT = /^[a-z0-9.-]{1,64}$/

export function parseAppletRuntime(value: unknown): AppletRuntimeDescriptor | undefined {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return undefined
  const record = value as Record<string, unknown>
  if (typeof record.apiBaseUrl !== 'string' || !LOOPBACK_ORIGIN.test(record.apiBaseUrl)) return undefined
  if (typeof record.appletTicket !== 'string' || !TICKET.test(record.appletTicket)) return undefined
  if (typeof record.appId !== 'string' || !APP_ID.test(record.appId)) return undefined
  if (typeof record.tenant !== 'string' || !TENANT.test(record.tenant)) return undefined
  if ('token' in record) return undefined
  const permissions = record.permissions === undefined ? [] : parsePermissionList(record.permissions)
  if (!permissions) return undefined
  return {
    apiBaseUrl: record.apiBaseUrl,
    appletTicket: record.appletTicket,
    appId: record.appId,
    tenant: record.tenant,
    permissions
  }
}

/** Canonical permission list. `null` when a value is outside the closed set or repeated. */
export function parsePermissionList(value: unknown): AppletPermission[] | null {
  const names = Array.isArray(value)
    ? value
    : typeof value === 'string'
      ? value.split(',').filter((item) => item !== '')
      : null
  if (!names) return null
  const permissions: AppletPermission[] = []
  for (const name of names) {
    if (typeof name !== 'string' || !APPLET_PERMISSIONS.includes(name as AppletPermission)) return null
    if (permissions.includes(name as AppletPermission)) return null
    permissions.push(name as AppletPermission)
  }
  return APPLET_PERMISSIONS.filter((name) => permissions.includes(name))
}

/**
 * Permissions for this applet page.
 * The shell puts them on the page URL so a later open can change them.
 * A page opened by an older shell falls back to the frozen runtime.
 */
export function readAppletPermissions(location: Pick<Location, 'search'> = window.location): AppletPermission[] {
  const params = new URLSearchParams(location.search)
  if (params.has('permissions')) {
    return parsePermissionList(params.get('permissions') ?? '') ?? []
  }
  return readAppletRuntime()?.permissions ?? []
}

/** Absent or invalid descriptor means this page cannot call the applet runtime. */
export function readAppletRuntime(host: typeof globalThis = globalThis): AppletRuntimeDescriptor | undefined {
  const value = (host as typeof host & { __A3S_APPLET_RUNTIME__?: unknown }).__A3S_APPLET_RUNTIME__
  if (value === undefined) return undefined
  return parseAppletRuntime(value)
}
