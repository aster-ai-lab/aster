import { type AppletPermission, parsePermissionList } from '@shared/appletRuntime'

import { APPLET_PROFILE_GRANT_KEY, profileGrantAllows } from './loadAppletProfile'

export const APPLET_PERMISSION_GRANT_KEY = 'a3s.applet.permission-grant.v1'

type GrantStorage = Pick<Storage, 'getItem' | 'setItem'>

const APPLET_IDS_BY_SLUG: Record<string, string> = {
  'a3s-review': 'review',
  'a3s-approval': 'approval',
  'a3s-leave': 'leave',
  'a3s-travel': 'travel',
  'a3s-expense': 'expense',
  'a3s-meeting': 'meeting',
  'a3s-seal': 'seal',
  'a3s-notice': 'notice',
  'a3s-pdf': 'pdf'
}

/** A package with no declared set still asks for the member's name and photo. */
export function entryPermissions(declared: readonly AppletPermission[]): AppletPermission[] {
  return declared.length > 0 ? [...declared] : ['user']
}

/** The stored value for one applet. It changes when the declared set changes. */
export function permissionGrantValue(permissions: readonly AppletPermission[]): string {
  return permissions.join('\n')
}

/**
 * True when this applet was allowed for exactly this permission set.
 * A previous allow of only user information still covers a package that asks only for that.
 * A denial is not stored, so the next open asks again.
 */
export function permissionGrantMatches(
  appId: string,
  permissions: readonly AppletPermission[],
  storage: GrantStorage = localStorage
): boolean {
  if (permissions.length === 0) return true
  try {
    const raw = storage.getItem(APPLET_PERMISSION_GRANT_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as unknown
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed) && appId in parsed) {
        return (parsed as Record<string, unknown>)[appId] === permissionGrantValue(permissions)
      }
    }
  } catch {
    /* a broken record asks again */
  }
  return permissions.length === 1 && permissions[0] === 'user' && profileGrantAllows(appId, storage)
}

export function allowPermissionGrant(
  appId: string,
  permissions: readonly AppletPermission[],
  storage: GrantStorage = localStorage
): void {
  const accepted = parsePermissionList([...permissions])
  if (!accepted || accepted.length === 0) return
  let current: Record<string, string> = {}
  try {
    const raw = storage.getItem(APPLET_PERMISSION_GRANT_KEY)
    const parsed = raw ? (JSON.parse(raw) as unknown) : {}
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      for (const [key, value] of Object.entries(parsed)) {
        if (typeof value === 'string') current[key] = value
      }
    }
  } catch {
    current = {}
  }
  current[appId] = permissionGrantValue(accepted)
  try {
    storage.setItem(APPLET_PERMISSION_GRANT_KEY, JSON.stringify(current))
  } catch {
    /* private mode: this open still proceeds */
  }
}

/** Drop a remembered allow. The next open after install or uninstall asks again. */
export function forgetPermissionGrant(appId: string, storage: GrantStorage = localStorage): void {
  try {
    const raw = storage.getItem(APPLET_PERMISSION_GRANT_KEY)
    if (raw) {
      const parsed = JSON.parse(raw) as unknown
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        const current = { ...(parsed as Record<string, unknown>) }
        delete current[appId]
        storage.setItem(APPLET_PERMISSION_GRANT_KEY, JSON.stringify(current))
      }
    }
  } catch {
    /* a broken record asks again */
  }
  try {
    const raw = storage.getItem(APPLET_PROFILE_GRANT_KEY)
    if (!raw) return
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) return
    storage.setItem(APPLET_PROFILE_GRANT_KEY, JSON.stringify(parsed.filter((item) => item !== appId)))
  } catch {
    /* a broken record asks again */
  }
}

export function forgetInstallPermissionGrant(slug: string, storage: GrantStorage = localStorage): void {
  const appId = APPLET_IDS_BY_SLUG[slug]
  if (!appId) return
  forgetPermissionGrant(appId, storage)
}
