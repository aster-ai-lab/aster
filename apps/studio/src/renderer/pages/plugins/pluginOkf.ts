import { MemberOrgError } from '@renderer/data/memberOrgs'
import { readStudioRuntime } from '@shared/studioRuntime'

export interface PluginOkfPage {
  path: string
  title: string
}

export function pluginShipsOkf(entry: { installed: boolean; surfaces?: readonly string[] }): boolean {
  return entry.installed && (entry.surfaces ?? []).includes('okf')
}

export async function listPluginOkfPages(slug: string): Promise<PluginOkfPage[]> {
  const payload = await okfRequest(`/api/capabilities/${encodeURIComponent(slug)}/okf`, 'GET')
  if (!payload || typeof payload !== 'object' || !('pages' in payload)) return []
  const rows = (payload as { pages?: unknown }).pages
  if (!Array.isArray(rows)) return []
  return rows.flatMap((row) => {
    if (!row || typeof row !== 'object') return []
    const record = row as { path?: unknown; title?: unknown }
    if (typeof record.path !== 'string' || !record.path.startsWith('okf/') || !record.path.endsWith('.md')) {
      return []
    }
    const title = typeof record.title === 'string' && record.title.trim() ? record.title : record.path
    return [{ path: record.path, title }]
  })
}

export async function readPluginOkfPage(
  slug: string,
  path: string
): Promise<{ path: string; title: string; markdown: string }> {
  const payload = await okfRequest(
    `/api/capabilities/${encodeURIComponent(slug)}/okf/page?path=${encodeURIComponent(path)}`,
    'GET'
  )
  if (!payload || typeof payload !== 'object') throw new MemberOrgError('invalid')
  const record = payload as { path?: unknown; title?: unknown; markdown?: unknown }
  if (record.path !== path || typeof record.markdown !== 'string') throw new MemberOrgError('invalid')
  return {
    path,
    title: typeof record.title === 'string' && record.title.trim() ? record.title : path,
    markdown: record.markdown
  }
}

/** Copy packaged OKF pages into the agent wiki. Existing local files stay. */
export async function seedPluginOkf(slug: string, directory: string): Promise<number> {
  try {
    const payload = await okfRequest(
      `/api/capabilities/${encodeURIComponent(slug)}/okf/seed`,
      'POST',
      JSON.stringify({ directory })
    )
    return payload && typeof payload === 'object' && typeof (payload as { seeded?: unknown }).seeded === 'number'
      ? (payload as { seeded: number }).seeded
      : 0
  } catch (error) {
    if (error instanceof MemberOrgError && error.message === 'missing_plugin') return 0
    throw error
  }
}

/** Persist one page and rebuild the local a3s-vec index. Returns how many pages were indexed. */
export async function savePluginOkfPage(input: {
  slug: string
  path: string
  markdown: string
  workspaceWiki?: string
}): Promise<number> {
  const payload = await okfRequest(
    `/api/capabilities/${encodeURIComponent(input.slug)}/okf/page`,
    'PUT',
    JSON.stringify({
      path: input.path,
      markdown: input.markdown,
      ...(input.workspaceWiki ? { workspaceWiki: input.workspaceWiki } : {})
    })
  )
  const indexed =
    payload && typeof payload === 'object' && typeof (payload as { indexed?: unknown }).indexed === 'number'
      ? (payload as { indexed: number }).indexed
      : 0
  return indexed
}

async function okfRequest(path: string, method: 'GET' | 'PUT' | 'POST', body?: string): Promise<unknown> {
  const runtime = readStudioRuntime()
  if (!runtime) throw new MemberOrgError('runtime')
  const response = await fetch(`${runtime.apiBaseUrl}${path}`, {
    method,
    cache: 'no-store',
    headers: {
      accept: 'application/json',
      authorization: `Bearer ${runtime.token}`,
      ...(body ? { 'content-type': 'application/json' } : {})
    },
    body
  })
  const text = await response.text()
  let payload: unknown = null
  if (text) {
    try {
      payload = JSON.parse(text) as unknown
    } catch {
      throw new MemberOrgError('failed', response.status)
    }
  }
  if (payload && typeof payload === 'object' && 'code' in payload && 'data' in payload) {
    const record = payload as { code?: unknown; data?: unknown; success?: unknown; message?: unknown }
    const code = typeof record.code === 'number' ? record.code : response.status
    const ok = (code === 0 || (code >= 200 && code < 300)) && record.success !== false
    if (!ok) {
      const message = typeof record.message === 'string' && record.message ? record.message : 'failed'
      throw new MemberOrgError(message, response.status)
    }
    payload = record.data
  }
  if (payload && typeof payload === 'object' && 'error' in payload && response.status >= 400) {
    const code = (payload as { error?: unknown }).error
    throw new MemberOrgError(typeof code === 'string' && code ? code : 'failed', response.status)
  }
  if (!response.ok) throw new MemberOrgError('failed', response.status)
  return payload
}
