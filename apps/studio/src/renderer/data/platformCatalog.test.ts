import { afterEach, describe, expect, it, vi } from 'vitest'

import { MemberOrgError } from './memberOrgs'
import {
  findAgentRuntimes,
  listCellRevisions,
  listCustomerPeople,
  listFunctionReleases,
  listModelRoutes,
  listPlatformItems,
  listPlatformLogLevels,
  listPlatformLogs,
  listPlatformUsage,
  platformErrorDetail,
  platformErrorText,
  readAgentRuntimeLogs,
  readPlatformOverview,
  readPlatformCatalogPage
} from './platformCatalog'

const runtime = { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) }

function respondWith(rows: unknown) {
  return vi.fn<(url: Parameters<typeof globalThis.fetch>[0]) => Promise<globalThis.Response>>(
    async () => new globalThis.Response(JSON.stringify({ code: 0, data: rows }), { status: 200 })
  )
}

afterEach(() => {
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
})

describe('platformCatalog', () => {
  const query = { page: 3, limit: 10, q: 'a%_\\&', filter: '', brand: '', sort: 'name', direction: 'desc' as const }
  it('reads bounded pages, preserves server clamp and encodes literal search parameters', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const fetcher = respondWith({
      items: [{ id: 'last', name: 'Last row' }],
      total: 11,
      page: 2,
      limit: 10,
      facets: { kind: [{ value: 'vendor', count: 11 }] }
    })
    const page = await readPlatformCatalogPage('suppliers', query, fetcher)
    expect(page).toMatchObject({ total: 11, page: 2, rows: [{ id: 'last' }] })
    const url = new URL(String(fetcher.mock.calls[0][0]))
    expect(url.pathname).toBe('/api/member/platform/suppliers/page')
    expect(url.searchParams.get('search')).toBe(query.q)
    expect(url.searchParams.get('sortOrder')).toBe('desc')
  })
  it.each([
    { items: [], total: 21, page: 3, limit: 10, facets: { kind: [] } },
    { items: [], total: 0, page: 3, limit: 10, facets: { kind: [] } },
    { items: [], total: 0, page: 1, limit: 50, facets: { kind: [] } },
    { items: [], total: -1, page: 1, limit: 10, facets: { kind: [] } },
    { items: [], total: Number.MAX_SAFE_INTEGER + 1, page: 1, limit: 10, facets: { kind: [] } },
    { items: [], total: 0, page: 1, limit: 10, facets: {} },
    {
      items: [{ id: 'last', name: 'Last row' }],
      total: 11,
      page: 2,
      limit: 10,
      facets: { kind: [{ value: 'vendor', count: 10 }] }
    },
    { items: [], total: 0, page: 1, limit: 10, facets: { kind: [{ value: 'vendor', count: -1 }] } },
    {
      items: [],
      total: 0,
      page: 1,
      limit: 10,
      facets: {
        kind: [
          { value: 'vendor', count: 0 },
          { value: 'vendor', count: 0 }
        ]
      }
    }
  ])('refuses inconsistent page/facet data: %j', async (page) => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    await expect(readPlatformCatalogPage('suppliers', query, respondWith(page) as typeof fetch)).rejects.toThrow(
      'invalid'
    )
  })
  it.each([
    [
      { id: 'ok', name: 'Valid' },
      { id: 3, name: 'Broken' }
    ],
    [
      { id: 'same', name: 'A' },
      { id: 'same', name: 'B' }
    ],
    [{ id: 'bad', name: 'Bad', fields: [{ key: 'state', value: 3 }] }],
    [
      {
        id: 'bad',
        name: 'Bad',
        fields: [
          { key: 'state', value: 'on' },
          { key: 'state', value: 'off' }
        ]
      }
    ]
  ])('refuses malformed collections instead of reporting a partial total: %j', async (...rows) => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    await expect(listPlatformItems('suppliers', undefined, respondWith(rows) as typeof fetch)).rejects.toThrow(
      'invalid'
    )
  })

  it('refuses malformed cursor metadata and incomplete overview totals', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    await expect(
      readAgentRuntimeLogs('exec', undefined, respondWith({ rows: [], next_cursor: 4 }) as typeof fetch)
    ).rejects.toThrow('invalid')
    await expect(readPlatformOverview(respondWith({ customers: 1 }) as typeof fetch)).rejects.toThrow('invalid')
  })
  it('names each failure in operator words and keeps the raw text as detail', () => {
    const say = (key: string) => key
    const cases: [unknown, string, string][] = [
      [new MemberOrgError('platform.absent.tickets', 404), 'platform.absent.tickets', ''],
      [new MemberOrgError('runtime'), 'platform.error.offline', ''],
      [new TypeError('Failed to fetch'), 'platform.error.offline', 'Failed to fetch'],
      [new MemberOrgError('session expired', 401), 'platform.error.denied', 'HTTP 401 · session expired'],
      [new MemberOrgError('HTTP 404', 404), 'platform.error.missing', 'HTTP 404'],
      [
        new MemberOrgError('window must be one of 15m', 400),
        'platform.error.rejected',
        'HTTP 400 · window must be one of 15m'
      ],
      [new MemberOrgError('slow down', 429), 'platform.error.busy', 'HTTP 429 · slow down'],
      [new MemberOrgError('pool timed out', 503), 'platform.error.server', 'HTTP 503 · pool timed out'],
      [new MemberOrgError('invalid'), 'platform.error.invalid', ''],
      ['boom', 'platform.error.unknown', '']
    ]
    for (const [error, text, detail] of cases) {
      expect(platformErrorText(error, say)).toBe(text)
      expect(platformErrorDetail(error)).toBe(detail)
    }
  })

  it('reads logs for the chosen time window', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const fetchImpl = respondWith([{ id: 'log-1', name: 'boot', fields: [{ key: 'level', value: 'info' }] }])
    const fetcher = fetchImpl as unknown as typeof globalThis.fetch
    const rows = await listPlatformLogs('24h', 'all', fetcher)
    await listPlatformLogs('1h', 'error', fetcher)
    await listPlatformLogLevels('7d', fetcher)
    expect(fetchImpl.mock.calls.map((call) => call[0])).toEqual([
      'http://127.0.0.1:9/api/member/platform/logs?window=24h',
      'http://127.0.0.1:9/api/member/platform/logs?window=1h&level=error',
      'http://127.0.0.1:9/api/member/platform/logs/levels?window=7d'
    ])
    expect(rows).toEqual([{ id: 'log-1', name: 'boot', meta: '', fields: [{ key: 'level', value: 'info' }] }])
  })

  it('keeps the kind query for faceted resources', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const fetchImpl = respondWith([])
    await listPlatformItems('fleet', 'fleet', fetchImpl)
    expect(fetchImpl.mock.calls[0][0]).toBe('http://127.0.0.1:9/api/member/platform/fleet?kind=fleet')
  })

  it('reads the people under one customer', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const fetchImpl = respondWith([{ id: 'person-1', name: '张三', fields: [{ key: 'status', value: 'active' }] }])
    const rows = await listCustomerPeople('customer-1', fetchImpl)
    expect(fetchImpl.mock.calls[0][0]).toBe('http://127.0.0.1:9/api/member/platform/customers/customer-1/people')
    expect(rows.map((row) => row.name)).toEqual(['张三'])
  })

  it('reads function releases, cell revisions and runtime lookups by id', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const fetchImpl = respondWith([])
    const fetcher = fetchImpl as unknown as typeof globalThis.fetch
    await listFunctionReleases('fn-1', fetcher)
    await listCellRevisions('p-1/e-1/a-1', fetcher)
    await findAgentRuntimes('conv-1', fetcher)
    await listModelRoutes(fetcher)
    await listPlatformUsage('90', fetcher)
    expect(fetchImpl.mock.calls.map((call) => call[0])).toEqual([
      'http://127.0.0.1:9/api/member/platform/functions/fn-1/releases',
      'http://127.0.0.1:9/api/member/platform/cells/p-1/e-1/a-1/revisions',
      'http://127.0.0.1:9/api/member/platform/agent-runtimes/conv-1',
      'http://127.0.0.1:9/api/member/platform/model-routes',
      'http://127.0.0.1:9/api/member/platform/usage?days=90'
    ])
  })

  it('pages runtime logs with the cursor the server returned', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const fetchImpl = respondWith({ rows: [{ id: '1', name: 'model_output', fields: [] }], next_cursor: 'c 2' })
    const fetcher = fetchImpl as unknown as typeof globalThis.fetch
    const first = await readAgentRuntimeLogs('exec-1', undefined, fetcher)
    expect(first.nextCursor).toBe('c 2')
    expect(first.rows.map((row) => row.name)).toEqual(['model_output'])
    await readAgentRuntimeLogs('exec-1', first.nextCursor ?? undefined, fetcher)
    expect(fetchImpl.mock.calls.map((call) => call[0])).toEqual([
      'http://127.0.0.1:9/api/member/platform/agent-runtimes/exec-1/logs',
      'http://127.0.0.1:9/api/member/platform/agent-runtimes/exec-1/logs?cursor=c%202'
    ])
  })
})
