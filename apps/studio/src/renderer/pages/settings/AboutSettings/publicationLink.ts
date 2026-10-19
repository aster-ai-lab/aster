import { readStudioRuntime, type StudioRuntimeDescriptor } from '@shared/studioRuntime'

/** Fixed About rows. A later blog post is not one of these. */
export type NoticeSlug = 'terms' | 'privacy' | 'copyright' | 'support'

const ABSOLUTE_HTTP = /^https?:\/\/\S+$/

type NoticeFetch = (input: string, init?: RequestInit) => Promise<Response>

type NoticeLookup = {
  fetchImpl?: NoticeFetch
  /** `null` means there is no shell runtime. Omit it to read the injected descriptor. */
  runtime?: StudioRuntimeDescriptor | null
}

/** Absolute URL of a published notice. Anything else stays closed. */
export async function publishedNoticeUrl(slug: NoticeSlug, lookup: NoticeLookup = {}): Promise<string | null> {
  const runtime = lookup.runtime === undefined ? runtimeOrAbsent() : lookup.runtime
  if (!runtime) return null
  const fetchImpl = lookup.fetchImpl ?? fetch
  try {
    const response = await fetchImpl(`${runtime.apiBaseUrl}/api/publications/${slug}`, {
      headers: { authorization: `Bearer ${runtime.token}` }
    })
    if (!response.ok) return null
    const payload: unknown = await response.json()
    const url = documentUrl(payload)
    return url && ABSOLUTE_HTTP.test(url) ? url : null
  } catch {
    return null
  }
}

function runtimeOrAbsent(): StudioRuntimeDescriptor | null {
  try {
    return readStudioRuntime() ?? null
  } catch {
    return null
  }
}

function documentUrl(payload: unknown): string | null {
  if (!payload || typeof payload !== 'object') return null
  const record = payload as Record<string, unknown>
  const nested = record.data
  const source = nested && typeof nested === 'object' ? (nested as Record<string, unknown>) : record
  return typeof source.url === 'string' ? source.url : null
}
