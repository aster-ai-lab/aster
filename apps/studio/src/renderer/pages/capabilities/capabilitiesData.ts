/**
 * 能力扩展 catalog. Connectors, skills, and plugins are capabilities the OS manages and members install.
 * Entries come from the OS (`/v1/api/org/capabilities`) through the Studio shell, which forwards them to the gateway.
 */

import { memberRequest, MemberOrgError } from '@renderer/data/memberOrgs'
import {
  CAPABILITY_CATEGORIES,
  type CapabilityEntry,
  type CapabilitySection
} from '@renderer/services/capabilities/capabilityCatalogue'
import { readStudioRuntime } from '@shared/studioRuntime'

import { forgetInstallPermissionGrant } from '../agents/appletPermissionGrant'
import { customConnectorRecord, type CustomConnectorBody } from './customConnector'
import { rememberPluginInstall } from './pluginInstallSync'

export type {
  AppletContract,
  AppletWindowSize,
  CapabilityCategory,
  CapabilityCategoryCount,
  CapabilityCategoryId,
  CapabilityConfigField,
  CapabilityEntry,
  CapabilityInstall,
  CapabilityKind,
  CapabilityPage,
  CapabilityQuery,
  CapabilitySection,
  CapabilitySectionId
} from '@renderer/services/capabilities/capabilityCatalogue'

export {
  CAPABILITY_CATEGORIES,
  CAPABILITY_PAGE_SIZE,
  capabilityCategoryLabelKey,
  listCapabilities
} from '@renderer/services/capabilities/capabilityCatalogue'

export interface InstallProgress {
  received: number
  total: number
}

export async function installCapability(
  slug: string,
  fetchImpl: typeof fetch = fetch,
  onProgress?: (progress: InstallProgress) => void
): Promise<void> {
  let runtime: ReturnType<typeof readStudioRuntime>
  try {
    runtime = readStudioRuntime()
  } catch {
    throw new MemberOrgError('runtime')
  }
  if (!runtime) throw new MemberOrgError('runtime')
  const response = await fetchImpl(`${runtime.apiBaseUrl}/api/capabilities/installations/${encodeURIComponent(slug)}`, {
    method: 'PUT',
    cache: 'no-store',
    headers: {
      accept: 'application/json, application/x-ndjson',
      authorization: `Bearer ${runtime.token}`
    }
  })
  const contentType = response.headers.get('content-type') ?? ''
  if (contentType.includes('application/x-ndjson') || contentType.includes('text/event-stream')) {
    await readInstallStream(response, onProgress)
    rememberPluginInstall(slug, true)
    forgetInstallPermissionGrant(slug)
    return
  }
  const text = await response.text()
  let payload: unknown = null
  if (text) {
    try {
      payload = JSON.parse(text) as unknown
    } catch {
      throw new MemberOrgError(text || `HTTP ${response.status}`, response.status)
    }
  }
  unwrapInstall(payload, response.status)
  rememberPluginInstall(slug, true)
  forgetInstallPermissionGrant(slug)
}

async function readInstallStream(response: Response, onProgress?: (progress: InstallProgress) => void): Promise<void> {
  const reader = response.body?.getReader()
  if (!reader) throw new MemberOrgError('install stream', response.status)
  const decoder = new TextDecoder()
  let buffer = ''
  let installed = false
  const applyLine = (line: string) => {
    let event: Record<string, unknown>
    try {
      event = JSON.parse(line) as Record<string, unknown>
    } catch {
      throw new MemberOrgError(line, response.status)
    }
    if (typeof event.error === 'string' && event.error) {
      throw new MemberOrgError(event.error, response.status)
    }
    const received = event.received
    const total = event.total
    if (
      typeof received === 'number' &&
      Number.isFinite(received) &&
      typeof total === 'number' &&
      Number.isFinite(total) &&
      total > 0 &&
      received >= 0 &&
      received <= total
    ) {
      onProgress?.({ received, total })
    }
    if (event.installed === true) installed = true
  }
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })
    let newline = buffer.indexOf('\n')
    while (newline >= 0) {
      const line = buffer.slice(0, newline).trim()
      buffer = buffer.slice(newline + 1)
      if (line) applyLine(line)
      newline = buffer.indexOf('\n')
    }
  }
  buffer += decoder.decode()
  const tail = buffer.trim()
  if (tail) applyLine(tail)
  if (!installed) throw new MemberOrgError('install did not finish', response.status)
}

function unwrapInstall(payload: unknown, status: number): void {
  if (payload && typeof payload === 'object' && 'code' in payload && 'data' in payload) {
    const record = payload as { code?: unknown; message?: unknown; success?: unknown }
    const code = typeof record.code === 'number' ? record.code : status
    const ok = (code === 0 || (code >= 200 && code < 300)) && record.success !== false
    if (!ok) {
      const message = typeof record.message === 'string' && record.message ? record.message : `API error ${code}`
      throw new MemberOrgError(message, status >= 400 ? status : code)
    }
    return
  }
  if (status >= 400) throw new MemberOrgError(`HTTP ${status}`, status)
}

export async function uninstallCapability(slug: string, fetchImpl: typeof fetch = fetch): Promise<void> {
  await memberRequest(`/api/capabilities/installations/${encodeURIComponent(slug)}`, { method: 'DELETE' }, fetchImpl)
  rememberPluginInstall(slug, false)
  forgetInstallPermissionGrant(slug)
}

/** Compressed ceiling for one uploaded skill package. The shell enforces the same limit. */
export const SKILL_ARCHIVE_BYTES = 32 * 1024 * 1024

export type BuiltinSkillName = 'ask-form' | 'skill-creator' | 'schedule-task' | 'stc-answer'

function requireShellRuntime() {
  let runtime: ReturnType<typeof readStudioRuntime>
  try {
    runtime = readStudioRuntime()
  } catch {
    throw new MemberOrgError('runtime')
  }
  if (!runtime) throw new MemberOrgError('runtime')
  return runtime
}

async function readSkillResult(response: Response): Promise<unknown> {
  const text = await response.text()
  let payload: unknown = null
  if (text) {
    try {
      payload = JSON.parse(text) as unknown
    } catch {
      throw new MemberOrgError('failed', response.status)
    }
  }
  if (payload && typeof payload === 'object' && 'error' in payload) {
    const code = (payload as { error?: unknown }).error
    if (typeof code === 'string' && code) throw new MemberOrgError(code, response.status)
  }
  if (!response.ok) throw new MemberOrgError('failed', response.status)
  return payload
}

/** Make a bundled skill visible to the agent when that name is still free. */
export async function prepareBuiltinSkill(name: BuiltinSkillName): Promise<void> {
  const runtime = requireShellRuntime()
  const response = await fetch(`${runtime.apiBaseUrl}/api/skills/builtin/${name}`, {
    method: 'POST',
    cache: 'no-store',
    headers: {
      accept: 'application/json',
      authorization: `Bearer ${runtime.token}`
    }
  })
  await readSkillResult(response)
}

/** Install one `.zip` or `.skill` package into the on-device skill directory. */
export async function uploadSkillPackage(file: File): Promise<{ name: string }> {
  const runtime = requireShellRuntime()
  const response = await fetch(`${runtime.apiBaseUrl}/api/skills/upload`, {
    method: 'POST',
    cache: 'no-store',
    headers: {
      accept: 'application/json',
      authorization: `Bearer ${runtime.token}`,
      'content-type': 'application/zip',
      'x-skill-filename': file.name
    },
    body: file
  })
  const payload = await readSkillResult(response)
  const name =
    payload && typeof payload === 'object' && typeof (payload as { name?: unknown }).name === 'string'
      ? (payload as { name: string }).name
      : ''
  if (!name) throw new MemberOrgError('failed', response.status)
  return { name }
}

async function shellCapability(path: string, method: 'GET' | 'PUT' | 'DELETE', body?: string): Promise<unknown> {
  const runtime = requireShellRuntime()
  const response = await fetch(`${runtime.apiBaseUrl}${path}`, {
    method,
    cache: 'no-store',
    headers: {
      accept: 'application/json',
      authorization: `Bearer ${runtime.token}`,
      ...(body ? { 'content-type': 'application/json' } : {})
    },
    ...(method !== 'GET' && body !== undefined ? { body } : {})
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
  if (payload && typeof payload === 'object' && 'error' in payload && response.status >= 400) {
    const code = (payload as { error?: unknown }).error
    throw new MemberOrgError(typeof code === 'string' && code ? code : 'failed', response.status)
  }
  if (!response.ok) throw new MemberOrgError('failed', response.status)
  return payload
}

/** Custom connectors stored on this device, without reading the OS catalog. */
export async function listCustomConnectors(): Promise<CustomConnectorBody[]> {
  const payload = await shellCapability('/api/capabilities/custom', 'GET')
  if (!payload || typeof payload !== 'object' || !('connectors' in payload)) return []
  const rows = (payload as { connectors?: unknown }).connectors
  if (!Array.isArray(rows)) return []
  return rows.flatMap((row) => {
    const parsed = customConnectorRecord(row)
    return parsed ? [parsed] : []
  })
}

/** Save one custom connector through the shell. The shell merges it into the agent session file. */
export async function saveCustomConnector(body: CustomConnectorBody): Promise<void> {
  await shellCapability('/api/capabilities/custom', 'PUT', JSON.stringify(body))
}

/** Remove one custom connector. A catalog install with the same name is left in place. */
export async function removeCustomConnector(name: string): Promise<void> {
  await shellCapability(`/api/capabilities/custom/${encodeURIComponent(name)}`, 'DELETE')
}

/** Case-insensitive substring match on name and description; an empty query returns every entry. */
export function searchCapabilities(entries: readonly CapabilityEntry[], query: string): CapabilityEntry[] {
  const needle = query.trim().toLocaleLowerCase()
  if (!needle) return [...entries]
  return entries.filter((entry) => `${entry.name} ${entry.description}`.toLocaleLowerCase().includes(needle))
}

/** Group entries into display sections: the curated picks first, then non-empty categories in canonical order. */
export function capabilitySections(entries: readonly CapabilityEntry[]): CapabilitySection[] {
  const featured = entries.filter((entry) => entry.featured)
  const sections: CapabilitySection[] = featured.length > 0 ? [{ id: 'featured', entries: featured }] : []
  for (const category of CAPABILITY_CATEGORIES) {
    const matched = entries.filter((entry) => entry.category === category.id)
    if (matched.length > 0) {
      sections.push({ id: category.id, labelKey: category.labelKey, entries: matched })
    }
  }
  return sections
}
