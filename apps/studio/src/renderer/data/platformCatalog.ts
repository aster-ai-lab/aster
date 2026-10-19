import { memberRequest, MemberOrgError } from './memberOrgs'

export type PlatformField = {
  key: string
  value: string
}

export type PlatformItem = {
  id: string
  name: string
  meta: string
  fields: PlatformField[]
  orgId?: string
}

export type PlatformOverview = {
  customers: number
  partners: number
  suppliers: number
  models: number
  services: number
}

function asItem(value: unknown): PlatformItem | null {
  if (!value || typeof value !== 'object') return null
  const row = value as {
    id?: unknown
    name?: unknown
    meta?: unknown
    fields?: unknown
    org_id?: unknown
    orgId?: unknown
  }
  if (typeof row.id !== 'string' || !row.id || typeof row.name !== 'string') return null
  const orgId = typeof row.org_id === 'string' ? row.org_id : typeof row.orgId === 'string' ? row.orgId : undefined
  if (row.fields !== undefined && !Array.isArray(row.fields)) return null
  const fields: PlatformField[] = []
  for (const field of row.fields ?? []) {
    if (!field || typeof field !== 'object') return null
    const entry = field as { key?: unknown; value?: unknown }
    if (
      typeof entry.key !== 'string' ||
      typeof entry.value !== 'string' ||
      fields.some((value) => value.key === entry.key)
    )
      return null
    fields.push({ key: entry.key, value: entry.value })
  }
  return {
    id: row.id,
    name: row.name,
    meta: typeof row.meta === 'string' ? row.meta : '',
    fields,
    ...(orgId ? { orgId } : {})
  }
}

/** A malformed or duplicate row must fail the collection, rather than silently changing totals or hiding resources. */
function decodeItems(data: unknown): PlatformItem[] {
  if (!Array.isArray(data)) throw new MemberOrgError('invalid')
  const ids = new Set<string>()
  return data.map((row) => {
    const item = asItem(row)
    if (!item || ids.has(item.id)) throw new MemberOrgError('invalid')
    ids.add(item.id)
    return item
  })
}

export async function listPlatformItems(
  resource: string,
  kind?: string,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<PlatformItem[]> {
  const query = kind ? `?kind=${encodeURIComponent(kind)}` : ''
  return readItems(`/api/member/platform/${resource}${query}`, fetchImpl)
}

export type PlatformCatalogResource = 'suppliers' | 'models'
export type PlatformCatalogQuery = {
  page: number
  limit: number
  q: string
  filter: string
  brand: string
  sort: string
  direction: 'asc' | 'desc'
}
export type PlatformOffsetPage = {
  rows: PlatformItem[]
  total: number
  page: number
  limit: number
  facets: Record<string, { value: string; count: number }[]>
}

/** Exact bounded owner page; partial arrays and inconsistent totals refuse the read. */
export async function readPlatformCatalogPage(
  resource: PlatformCatalogResource,
  query: PlatformCatalogQuery,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<PlatformOffsetPage> {
  if (
    !Number.isInteger(query.page) ||
    query.page < 1 ||
    query.page > 1_000_000 ||
    ![10, 20, 50, 100].includes(query.limit)
  )
    throw new MemberOrgError('invalid')
  const params = new URLSearchParams({
    page: String(query.page),
    limit: String(query.limit),
    search: query.q.trim(),
    filter: query.filter,
    brand: query.brand,
    sortBy: query.sort,
    sortOrder: query.direction
  })
  const data = await memberRequest(`/api/member/platform/${resource}/page?${params}`, { method: 'GET' }, fetchImpl)
  if (!data || typeof data !== 'object' || Array.isArray(data)) throw new MemberOrgError('invalid')
  const record = data as Record<string, unknown>
  const integer = (value: unknown): value is number =>
    typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
  if (
    !integer(record.total) ||
    !integer(record.page) ||
    record.limit !== query.limit ||
    record.page !== Math.min(query.page, Math.max(1, Math.ceil(record.total / query.limit)))
  )
    throw new MemberOrgError('invalid')
  const rows = decodeItems(record.items)
  if (rows.length !== Math.min(query.limit, Math.max(0, record.total - (record.page - 1) * query.limit)))
    throw new MemberOrgError('invalid')
  if (!record.facets || typeof record.facets !== 'object' || Array.isArray(record.facets))
    throw new MemberOrgError('invalid')
  const facets: PlatformOffsetPage['facets'] = {}
  const expected = resource === 'suppliers' ? ['kind'] : ['state', 'brand']
  const entries = Object.entries(record.facets)
  if (entries.length !== expected.length || entries.some(([key]) => !expected.includes(key)))
    throw new MemberOrgError('invalid')
  for (const [key, values] of entries) {
    if (!Array.isArray(values)) throw new MemberOrgError('invalid')
    const seen = new Set<string>()
    facets[key] = values.map((entry: unknown) => {
      if (!entry || typeof entry !== 'object' || Array.isArray(entry)) throw new MemberOrgError('invalid')
      const { value, count } = entry as Record<string, unknown>
      if (typeof value !== 'string' || !value || seen.has(value) || !integer(count)) throw new MemberOrgError('invalid')
      seen.add(value)
      return { value, count }
    })
  }
  const filterFacet = facets[resource === 'suppliers' ? 'kind' : 'state']
  const filterCount = query.filter
    ? (filterFacet.find((entry) => entry.value === query.filter)?.count ?? 0)
    : filterFacet.reduce((sum, entry) => sum + entry.count, 0)
  if (filterCount !== record.total) throw new MemberOrgError('invalid')
  if (resource === 'models') {
    const brandCount = query.brand
      ? (facets.brand.find((entry) => entry.value === query.brand)?.count ?? 0)
      : facets.brand.reduce((sum, entry) => sum + entry.count, 0)
    if (!Number.isSafeInteger(brandCount) || (query.brand ? brandCount !== record.total : brandCount > record.total))
      throw new MemberOrgError('invalid')
  }
  return { rows, total: record.total, page: record.page, limit: query.limit, facets }
}

export async function listCustomerPeople(
  customerId: string,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<PlatformItem[]> {
  return readItems(`/api/member/platform/customers/${encodeURIComponent(customerId)}/people`, fetchImpl)
}

/** Every model's routes; a model id may contain `/`, so rows carry it in `offering`. */
export async function listModelRoutes(fetchImpl: typeof globalThis.fetch = globalThis.fetch): Promise<PlatformItem[]> {
  return readItems('/api/member/platform/model-routes', fetchImpl)
}

export async function listFunctionReleases(
  assetId: string,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<PlatformItem[]> {
  return readItems(`/api/member/platform/functions/${encodeURIComponent(assetId)}/releases`, fetchImpl)
}

/** A cell row id is `{project}/{environment}/{application}`. */
export async function listCellRevisions(
  cellId: string,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<PlatformItem[]> {
  const path = cellId.split('/').map(encodeURIComponent).join('/')
  return readItems(`/api/member/platform/cells/${path}/revisions`, fetchImpl)
}

/** A conversation id lists its runtimes; an execution id returns that one. */
export async function findAgentRuntimes(
  id: string,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<PlatformItem[]> {
  return readItems(`/api/member/platform/agent-runtimes/${encodeURIComponent(id)}`, fetchImpl)
}

export type PlatformPage = {
  rows: PlatformItem[]
  nextCursor: string | null
}

export async function readAgentRuntimeLogs(
  executionId: string,
  cursor?: string,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<PlatformPage> {
  const query = cursor ? `?cursor=${encodeURIComponent(cursor)}` : ''
  const data = await memberRequest(
    `/api/member/platform/agent-runtimes/${encodeURIComponent(executionId)}/logs${query}`,
    { method: 'GET' },
    fetchImpl
  )
  if (!data || typeof data !== 'object') throw new MemberOrgError('invalid')
  const page = data as { rows?: unknown; next_cursor?: unknown }
  if (!Array.isArray(page.rows)) throw new MemberOrgError('invalid')
  if (page.next_cursor !== null && page.next_cursor !== undefined && typeof page.next_cursor !== 'string')
    throw new MemberOrgError('invalid')
  return {
    rows: decodeItems(page.rows),
    nextCursor: typeof page.next_cursor === 'string' && page.next_cursor ? page.next_cursor : null
  }
}

export const LOG_WINDOWS = ['15m', '1h', '6h', '24h', '7d'] as const
export type LogWindow = (typeof LOG_WINDOWS)[number]

export const LOG_LEVELS = ['all', 'error', 'warn', 'info', 'debug', 'trace'] as const
export type LogLevel = (typeof LOG_LEVELS)[number]

/** Newest entries first; the server returns at most `LOG_PAGE_LIMIT` rows. */
export const LOG_PAGE_LIMIT = 200

export async function listPlatformLogs(
  window: LogWindow,
  level: LogLevel = 'all',
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<PlatformItem[]> {
  const levelQuery = level === 'all' ? '' : `&level=${encodeURIComponent(level)}`
  return readItems(`/api/member/platform/logs?window=${encodeURIComponent(window)}${levelQuery}`, fetchImpl)
}

/** One row per level (`all` included) with a `count` field over the whole window. */
export async function listPlatformLogLevels(
  window: LogWindow,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<PlatformItem[]> {
  return readItems(`/api/member/platform/logs/levels?window=${encodeURIComponent(window)}`, fetchImpl)
}

export const USAGE_WINDOWS = ['7', '30', '90'] as const
export type UsageWindow = (typeof USAGE_WINDOWS)[number]

/** Rows carry a `section` field: `days`, `customers` or `models`. */
export async function listPlatformUsage(
  days: UsageWindow,
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<PlatformItem[]> {
  return readItems(`/api/member/platform/usage?days=${encodeURIComponent(days)}`, fetchImpl)
}

async function readItems(path: string, fetchImpl: typeof globalThis.fetch): Promise<PlatformItem[]> {
  const data = await memberRequest(path, { method: 'GET' }, fetchImpl)
  return decodeItems(data)
}

export async function readPlatformOverview(
  fetchImpl: typeof globalThis.fetch = globalThis.fetch
): Promise<PlatformOverview> {
  const data = await memberRequest('/api/member/platform/overview', { method: 'GET' }, fetchImpl)
  if (!data || typeof data !== 'object') throw new MemberOrgError('invalid')
  const row = data as Partial<PlatformOverview>
  const keys = ['customers', 'partners', 'suppliers', 'models', 'services'] as const
  for (const key of keys) {
    if (typeof row[key] !== 'number' || !Number.isSafeInteger(row[key]) || row[key] < 0)
      throw new MemberOrgError('invalid')
  }
  return row as PlatformOverview
}

/** A ledger this installation does not keep. Retrying cannot change the answer. */
export function isPlatformAbsent(error: unknown): boolean {
  return error instanceof MemberOrgError && error.message.startsWith('platform.absent.')
}

/** Codes the client raises itself; they carry no server wording worth showing. */
const CLIENT_CODES = new Set(['runtime', 'invalid'])

function errorKey(error: unknown): string {
  if (error instanceof TypeError) return 'platform.error.offline'
  if (!(error instanceof MemberOrgError)) return 'platform.error.unknown'
  if (error.message === 'runtime') return 'platform.error.offline'
  if (error.message === 'invalid') return 'platform.error.invalid'
  const status = error.status ?? 0
  if (status === 401 || status === 403) return 'platform.error.denied'
  if (status === 404) return 'platform.error.missing'
  if (status === 429) return 'platform.error.busy'
  if (status >= 500) return 'platform.error.server'
  if (status >= 400) return 'platform.error.rejected'
  return 'platform.error.unknown'
}

/** What went wrong in operator words; the raw server text lives in `platformErrorDetail`. */
export function platformErrorText(error: unknown, translate: (key: string) => string): string {
  if (isPlatformAbsent(error)) {
    return translate((error as MemberOrgError).message)
  }
  return translate(errorKey(error))
}

/** The raw failure, kept for a support ticket. Empty when there is nothing beyond the summary. */
export function platformErrorDetail(error: unknown): string {
  if (isPlatformAbsent(error) || !(error instanceof Error)) return ''
  const message = error.message.trim()
  if (!message || CLIENT_CODES.has(message)) return ''
  const status = error instanceof MemberOrgError ? error.status : undefined
  return status && !message.startsWith('HTTP ') ? `HTTP ${status} · ${message}` : message
}
