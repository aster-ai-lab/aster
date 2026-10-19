/**
 * Connector values the member types on this device. The catalog never stores them.
 * Saving must reach the shell (`PUT /api/capabilities/config`), which writes
 * `connector-config.json` and merges the values into `mcp.json` headers — without
 * that write the connector cannot authenticate. localStorage only keeps a copy
 * so the form survives a failed save; it is never the saved state itself.
 */

import { memberRequest } from '@renderer/data/memberOrgs'

const STORAGE_KEY = 'a3s.connector.config.v1'

type Store = Record<string, Record<string, string>>

function readStore(): Store {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw) as unknown
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {}
    return parsed as Store
  } catch {
    return {}
  }
}

function writeStore(store: Store) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(store))
  } catch {
    // The form still holds the values for this open dialog.
  }
}

function asValues(value: unknown): Record<string, string> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {}
  const values: Record<string, string> = {}
  for (const [key, raw] of Object.entries(value as Record<string, unknown>)) {
    if (typeof raw !== 'string' || !raw) continue
    values[key] = raw
  }
  return values
}

export function readConnectorConfig(slug: string): Record<string, string> {
  return asValues(readStore()[slug])
}

function remember(slug: string, values: Record<string, string>) {
  const store = readStore()
  if (Object.keys(values).length === 0) delete store[slug]
  else store[slug] = values
  writeStore(store)
}

async function putRemote(slug: string, values: Record<string, string>) {
  await memberRequest(
    `/api/capabilities/config/${encodeURIComponent(slug)}`,
    { method: 'PUT', body: JSON.stringify({ values }) },
    fetch
  )
}

/** Shell copy when the route exists, otherwise the on-device form copy. */
export async function loadConnectorConfig(slug: string): Promise<Record<string, string>> {
  const local = readConnectorConfig(slug)
  try {
    const data = await memberRequest('/api/capabilities/config', { method: 'GET' }, fetch)
    if (!data || typeof data !== 'object' || Array.isArray(data)) return local
    const remote = asValues((data as Record<string, unknown>)[slug])
    if (Object.keys(remote).length > 0) {
      remember(slug, remote)
      return remote
    }
    if (Object.keys(local).length > 0) void putRemote(slug, local).catch(() => undefined)
  } catch {
    // The running shell may not expose the local route yet.
  }
  return local
}

export async function saveConnectorConfig(slug: string, values: Record<string, string>): Promise<void> {
  const clean = asValues(values)
  remember(slug, clean)
  // Propagates on failure: a save the shell never received is not a save, and
  // the caller must tell the member. The remembered copy keeps the form filled.
  await putRemote(slug, clean)
}

export async function clearConnectorConfig(slug: string): Promise<void> {
  remember(slug, {})
  try {
    await putRemote(slug, {})
  } catch {
    // Uninstall already succeeded. A later save rewrites the device file.
  }
}
