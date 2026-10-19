import { readAppletRuntime, type AppletRuntimeDescriptor } from '@shared/appletRuntime'

export type AppletChannel = 'release' | 'trial'

export type AppletField = {
  name: string
  kind: 'text' | 'textarea'
  label: string
  required: boolean
  max: number
}

export type PinnedAppletPackage = {
  appId: string
  version: string
  digest: string
  source: 'cache' | 'network'
  body: string
  channel: AppletChannel
  updateReady: Promise<boolean>
}

type CachedPackage = {
  version: string
  digest: string
  body: string
}

type CacheFile = Record<string, Partial<Record<AppletChannel, CachedPackage>>>

export const APPLET_PACKAGE_CACHE_PREFIX = 'a3s.applet.packages.v1'

export function appletPackageCacheKey(tenant: string): string {
  return `${APPLET_PACKAGE_CACHE_PREFIX}:${tenant}`
}

const WAIT_MS = 1000
const VERSION = /^[0-9]+\.[0-9]+\.[0-9]+$/
const DIGEST = /^[a-f0-9]{64}$/

const FALLBACK_FIELDS: AppletField[] = [
  { name: 'title', kind: 'text', label: '', required: true, max: 80 },
  { name: 'detail', kind: 'textarea', label: '', required: true, max: 2000 }
]

/**
 * Open a published applet package.
 * A cached release opens immediately. A newer release is stored for the next
 * open and does not replace the package already pinned. The first open waits
 * for the release bytes, at most 1s; a late download can fill the cache only.
 */
export async function loadAppletPackage(
  appId: string,
  fetchImpl: typeof fetch = globalThis.fetch,
  waitMs = WAIT_MS,
  storage: Pick<Storage, 'getItem' | 'setItem'> = localStorage,
  channel: AppletChannel = 'release'
): Promise<PinnedAppletPackage | null> {
  const runtime = readAppletRuntime()
  if (!runtime || runtime.appId !== appId) return null

  const cached = await verifiedCache(storage, runtime.tenant, appId, channel)
  if (cached) {
    const updateReady = revalidate(runtime, appId, channel, cached.digest, fetchImpl, storage).catch(() => false)
    return pin(appId, cached, 'cache', channel, updateReady)
  }

  let timer: ReturnType<typeof setTimeout> | undefined
  const timeout = new Promise<null>((resolve) => {
    timer = setTimeout(() => resolve(null), waitMs)
  })
  const downloaded = downloadChannel(runtime, appId, channel, fetchImpl, storage).catch(() => null)
  const result = await Promise.race([downloaded, timeout])
  if (timer !== undefined) clearTimeout(timer)
  return result
}

export function fieldsFromPackage(body: string, appId: string): AppletField[] | null {
  let parsed: { appId?: unknown; fields?: unknown }
  try {
    parsed = JSON.parse(body) as { appId?: unknown; fields?: unknown }
  } catch {
    return null
  }
  if (parsed.appId !== appId) return null
  if (parsed.fields === undefined) return FALLBACK_FIELDS
  if (!Array.isArray(parsed.fields) || parsed.fields.length === 0 || parsed.fields.length > 8) return null
  const fields: AppletField[] = []
  for (const item of parsed.fields) {
    if (!item || typeof item !== 'object') return null
    const field = item as Record<string, unknown>
    if (typeof field.name !== 'string' || !/^[a-z][a-z0-9]{0,31}$/.test(field.name)) return null
    if (field.kind !== 'text' && field.kind !== 'textarea') return null
    if (typeof field.label !== 'string' || field.label.trim() === '' || field.label.length > 40) return null
    if (typeof field.required !== 'boolean') return null
    if (typeof field.max !== 'number' || !Number.isInteger(field.max) || field.max < 1 || field.max > 2000) return null
    if (fields.some((existing) => existing.name === field.name)) return null
    fields.push({
      name: field.name,
      kind: field.kind,
      label: field.label,
      required: field.required,
      max: field.max
    })
  }
  return fields
}

export async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes)
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}

async function verifiedCache(
  storage: Pick<Storage, 'getItem' | 'setItem'>,
  tenant: string,
  appId: string,
  channel: AppletChannel
): Promise<CachedPackage | null> {
  const record = readCache(storage, tenant)[appId]?.[channel]
  if (!record || record.version === '' || record.digest === '' || record.body === '') return null
  const digest = await sha256Hex(new TextEncoder().encode(record.body))
  if (digest !== record.digest) return null
  let parsed: { appId?: unknown; version?: unknown }
  try {
    parsed = JSON.parse(record.body) as { appId?: unknown; version?: unknown }
  } catch {
    return null
  }
  if (parsed.appId !== appId || parsed.version !== record.version) return null
  return record
}

async function revalidate(
  runtime: AppletRuntimeDescriptor,
  appId: string,
  channel: AppletChannel,
  digest: string,
  fetchImpl: typeof fetch,
  storage: Pick<Storage, 'getItem' | 'setItem'>
): Promise<boolean> {
  const meta = await readChannel(runtime, appId, channel, fetchImpl)
  if (!meta || meta.digest === digest) return false
  const saved = await downloadChannel(runtime, appId, channel, fetchImpl, storage)
  return saved != null && saved.digest !== digest
}

async function downloadChannel(
  runtime: AppletRuntimeDescriptor,
  appId: string,
  channel: AppletChannel,
  fetchImpl: typeof fetch,
  storage: Pick<Storage, 'getItem' | 'setItem'>
): Promise<PinnedAppletPackage | null> {
  const meta = await readChannel(runtime, appId, channel, fetchImpl)
  if (!meta) return null
  const body = await readBody(runtime, `/api/applet-runtime/${appId}/package/${channel}`, fetchImpl)
  if (!body) return null
  const digest = await sha256Hex(new TextEncoder().encode(body))
  if (digest !== meta.digest) return null
  let parsed: { appId?: unknown; version?: unknown }
  try {
    parsed = JSON.parse(body) as { appId?: unknown; version?: unknown }
  } catch {
    return null
  }
  if (parsed.appId !== appId || parsed.version !== meta.version) return null
  const cached = { version: meta.version, digest, body }
  writeCache(storage, runtime.tenant, appId, channel, cached)
  return pin(appId, cached, 'network', channel, Promise.resolve(false))
}

async function readChannel(
  runtime: AppletRuntimeDescriptor,
  appId: string,
  channel: AppletChannel,
  fetchImpl: typeof fetch
): Promise<{ version: string; digest: string } | null> {
  const body = await readBody(runtime, `/api/applet-runtime/${appId}`, fetchImpl)
  if (!body) return null
  try {
    const parsed = JSON.parse(body) as {
      release?: { version?: unknown; digest?: unknown } | null
      trial?: { version?: unknown; digest?: unknown } | null
    }
    const slot = parsed[channel]
    if (!slot || typeof slot.version !== 'string' || typeof slot.digest !== 'string') return null
    if (!VERSION.test(slot.version) || !DIGEST.test(slot.digest)) return null
    return { version: slot.version, digest: slot.digest }
  } catch {
    return null
  }
}

async function readBody(
  runtime: AppletRuntimeDescriptor,
  path: string,
  fetchImpl: typeof fetch
): Promise<string | null> {
  const response = await fetchImpl(`${runtime.apiBaseUrl}${path}`, {
    method: 'GET',
    cache: 'no-store',
    headers: {
      accept: 'application/json',
      authorization: `Bearer ${runtime.appletTicket}`
    }
  })
  if (!response.ok) return null
  return response.text()
}

function pin(
  appId: string,
  cached: CachedPackage,
  source: PinnedAppletPackage['source'],
  channel: AppletChannel,
  updateReady: Promise<boolean>
): PinnedAppletPackage {
  return { appId, version: cached.version, digest: cached.digest, source, body: cached.body, channel, updateReady }
}

function readCache(storage: Pick<Storage, 'getItem' | 'setItem'>, tenant: string): CacheFile {
  const raw = storage.getItem(appletPackageCacheKey(tenant))
  if (!raw) return {}
  try {
    const parsed = JSON.parse(raw) as unknown
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {}
    return parsed as CacheFile
  } catch {
    return {}
  }
}

function writeCache(
  storage: Pick<Storage, 'getItem' | 'setItem'>,
  tenant: string,
  appId: string,
  channel: AppletChannel,
  record: CachedPackage
): void {
  const all = readCache(storage, tenant)
  const entry = all[appId] ?? {}
  entry[channel] = record
  all[appId] = entry
  storage.setItem(appletPackageCacheKey(tenant), JSON.stringify(all))
}
