import { readAppletRuntime } from '@shared/appletRuntime'

export const APPLET_PROFILE_GRANT_KEY = 'a3s.applet.profile-grant.v1'

export type AppletProfile = {
  displayName: string
  avatar: string
}

/** The member already allowed this applet to show their name and photo. */
export function profileGrantAllows(appId: string, storage: Pick<Storage, 'getItem'> = localStorage): boolean {
  try {
    const raw = storage.getItem(APPLET_PROFILE_GRANT_KEY)
    if (!raw) return false
    const parsed = JSON.parse(raw) as unknown
    return Array.isArray(parsed) && parsed.some((item) => item === appId)
  } catch {
    return false
  }
}

/** Remember an allow. A denial is not stored, so the next open asks again. */
export function allowProfileGrant(appId: string, storage: Pick<Storage, 'getItem' | 'setItem'> = localStorage): void {
  let current: string[] = []
  try {
    const raw = storage.getItem(APPLET_PROFILE_GRANT_KEY)
    const parsed = raw ? (JSON.parse(raw) as unknown) : []
    if (Array.isArray(parsed)) {
      current = parsed.filter((item): item is string => typeof item === 'string' && item !== appId)
    }
  } catch {
    current = []
  }
  current.push(appId)
  try {
    storage.setItem(APPLET_PROFILE_GRANT_KEY, JSON.stringify(current))
  } catch {
    /* private mode: this open still shows the profile */
  }
}

/**
 * Name and photo for an applet ticket.
 * The phone and the session token are dropped even if the payload carries them.
 */
export async function loadAppletProfile(
  appId: string,
  fetchImpl: typeof fetch = globalThis.fetch
): Promise<AppletProfile | null> {
  const runtime = readAppletRuntime()
  if (!runtime || runtime.appId !== appId) return null
  try {
    const response = await fetchImpl(`${runtime.apiBaseUrl}/api/applet-runtime/${appId}/profile`, {
      method: 'GET',
      cache: 'no-store',
      headers: {
        accept: 'application/json',
        authorization: `Bearer ${runtime.appletTicket}`
      }
    })
    if (!response.ok) return null
    const data = (await response.json()) as { displayName?: unknown; avatar?: unknown }
    if (!data || typeof data !== 'object') return null
    return {
      displayName: cleanDisplayName(data.displayName),
      avatar: cleanAvatar(data.avatar)
    }
  } catch {
    return null
  }
}

function cleanDisplayName(value: unknown): string {
  if (typeof value !== 'string') return ''
  const trimmed = value.trim()
  if (trimmed.length > 80 || [...trimmed].some((ch) => ch.charCodeAt(0) < 32)) return ''
  return trimmed
}

function cleanAvatar(value: unknown): string {
  if (typeof value !== 'string') return ''
  const trimmed = value.trim()
  if (!trimmed.startsWith('data:image/') || !trimmed.includes(';base64,') || trimmed.length > 400_000) return ''
  if ([...trimmed].some((ch) => ch.charCodeAt(0) < 32)) return ''
  return trimmed
}
