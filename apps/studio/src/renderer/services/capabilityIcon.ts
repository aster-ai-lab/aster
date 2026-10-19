import { readStudioRuntime } from '@shared/studioRuntime'

/** The member API form of a catalog icon path the server issued, or undefined for any other value. */
export function catalogIconPath(value: unknown): string | undefined {
  if (typeof value !== 'string' || value.includes('..') || value.includes('\\')) return undefined
  const path = value
    .replace(/^\/v1\/api\/org\/capabilities\//, '/api/capabilities/')
    .replace(/^\/api\/org\/capabilities\//, '/api/capabilities/')
  return path.startsWith('/api/capabilities/') ? path : undefined
}

/** The catalog icon is an authenticated API path, not an address an image element can load. */
async function fetchCapabilityIcon(
  iconUrl: string,
  signal?: AbortSignal
): Promise<{ bytes: ArrayBuffer; type: string } | null> {
  try {
    const runtime = readStudioRuntime()
    if (!runtime) return null
    const response = await fetch(`${runtime.apiBaseUrl}${iconUrl}`, {
      headers: { authorization: `Bearer ${runtime.token}` },
      signal,
      cache: 'force-cache'
    })
    if (!response.ok) return null
    const type = (response.headers.get('content-type') ?? '').split(';')[0].trim()
    return { bytes: await response.arrayBuffer(), type }
  } catch {
    return null
  }
}

export async function capabilityIconObjectUrl(iconUrl: string, signal: AbortSignal): Promise<string | null> {
  const icon = await fetchCapabilityIcon(iconUrl, signal)
  return icon ? URL.createObjectURL(new Blob([icon.bytes], { type: icon.type })) : null
}

const iconDataUrls = new Map<string, Promise<string | null>>()

/** A self-contained image address for a catalog icon. It survives storage on an installed agent. */
export function capabilityIconDataUrl(iconUrl: string): Promise<string | null> {
  const cached = iconDataUrls.get(iconUrl)
  if (cached) return cached
  const pending = fetchCapabilityIcon(iconUrl).then((icon) => {
    if (!icon || !icon.type.startsWith('image/')) return null
    const bytes = new Uint8Array(icon.bytes)
    let binary = ''
    for (let at = 0; at < bytes.length; at += 0x8000) {
      binary += String.fromCharCode(...bytes.subarray(at, at + 0x8000))
    }
    return `data:${icon.type};base64,${btoa(binary)}`
  })
  iconDataUrls.set(iconUrl, pending)
  void pending.then((url) => {
    if (!url) iconDataUrls.delete(iconUrl)
  })
  return pending
}
