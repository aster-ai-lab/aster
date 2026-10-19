import { MemberOrgError } from '@renderer/data/memberOrgs'
import { readStudioRuntime } from '@shared/studioRuntime'

/** Surfaces the shell materializes onto disk when a plugin installs. */
const MATERIALIZED_SURFACES = ['skill', 'mcp', 'tool', 'flow'] as const

export function pluginNeedsMaterialization(surfaces: readonly string[]): boolean {
  return surfaces.some((surface) => (MATERIALIZED_SURFACES as readonly string[]).includes(surface))
}

export interface PluginCallableSurface {
  kind: string
  id: string
}

/** Reads the surfaces the shell can call for one installed plugin. */
export async function readPluginCallable(slug: string, expectedDigest?: string): Promise<PluginCallableSurface[]> {
  const runtime = readStudioRuntime()
  if (!runtime) throw new MemberOrgError('runtime')
  const response = await fetch(`${runtime.apiBaseUrl}/api/capabilities/${encodeURIComponent(slug)}/callable`, {
    method: 'GET',
    cache: 'no-store',
    headers: {
      accept: 'application/json',
      authorization: `Bearer ${runtime.token}`
    }
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
  if (!response.ok) throw new MemberOrgError('failed', response.status)
  if (
    expectedDigest &&
    (!payload ||
      typeof payload !== 'object' ||
      !('packageDigest' in payload) ||
      payload.packageDigest !== expectedDigest)
  ) {
    throw new MemberOrgError('not_materialized')
  }
  if (!payload || typeof payload !== 'object' || !('surfaces' in payload)) return []
  const rows = (payload as { surfaces?: unknown }).surfaces
  if (!Array.isArray(rows)) return []
  return rows.flatMap((row) => {
    if (!row || typeof row !== 'object') return []
    const record = row as { kind?: unknown; id?: unknown }
    if (typeof record.kind !== 'string' || typeof record.id !== 'string' || !record.id) return []
    return [{ kind: record.kind, id: record.id }]
  })
}

/** Refuses when a declared skill, mcp, tool, or flow surface is not callable. */
export async function assertPluginMaterialized(
  slug: string,
  surfaces: readonly string[],
  expectedDigest?: string
): Promise<void> {
  const ready = await readPluginCallable(slug, expectedDigest)
  for (const surface of surfaces) {
    if (!(MATERIALIZED_SURFACES as readonly string[]).includes(surface)) continue
    if (!ready.some((row) => row.kind === surface)) throw new MemberOrgError('not_materialized')
  }
}
