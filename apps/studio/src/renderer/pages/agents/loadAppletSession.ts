import { readAppletRuntime } from '@shared/appletRuntime'

export type AppletSessionLoad =
  | { status: 'ready'; displayName: string }
  | { status: 'signed_out' }
  | { status: 'unavailable' }

const APPLET_SESSION_WAIT_MS = 1000

/**
 * Load the signed-in member for an applet window.
 * A profile that arrives after the 1s cap does not change this result.
 */
export async function loadAppletSession(
  appId: string,
  fetchImpl: typeof fetch = globalThis.fetch,
  waitMs = APPLET_SESSION_WAIT_MS
): Promise<AppletSessionLoad> {
  const runtime = readAppletRuntime()
  if (!runtime || runtime.appId !== appId) return { status: 'unavailable' }

  let timer: ReturnType<typeof setTimeout> | undefined
  const timeout = new Promise<AppletSessionLoad>((resolve) => {
    timer = setTimeout(() => resolve({ status: 'unavailable' }), waitMs)
  })
  const request = readProfile(runtime.apiBaseUrl, runtime.appletTicket, appId, fetchImpl)
  const result = await Promise.race([request, timeout])
  if (timer !== undefined) clearTimeout(timer)
  return result
}

async function readProfile(
  apiBaseUrl: string,
  ticket: string,
  appId: string,
  fetchImpl: typeof fetch
): Promise<AppletSessionLoad> {
  try {
    const response = await fetchImpl(`${apiBaseUrl}/api/applet-runtime/${appId}/session`, {
      method: 'GET',
      cache: 'no-store',
      headers: {
        accept: 'application/json',
        authorization: `Bearer ${ticket}`
      }
    })
    if (!response.ok) return { status: 'unavailable' }
    const data = (await response.json()) as { signedIn?: unknown; displayName?: unknown }
    if (!data || typeof data !== 'object') return { status: 'unavailable' }
    if (data.signedIn === true) {
      return { status: 'ready', displayName: cleanDisplayName(data.displayName) }
    }
    if (data.signedIn === false) return { status: 'signed_out' }
    return { status: 'unavailable' }
  } catch {
    return { status: 'unavailable' }
  }
}

function cleanDisplayName(value: unknown): string {
  if (typeof value !== 'string') return ''
  const trimmed = value.trim()
  if (trimmed.length > 80 || [...trimmed].some((ch) => ch.charCodeAt(0) < 32)) return ''
  return trimmed
}
