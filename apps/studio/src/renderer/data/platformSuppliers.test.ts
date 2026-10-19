import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  createSupplier,
  deleteSupplier,
  readSupplier,
  readSupplierAccess,
  readSupplierRelations,
  resolveSupplierCreate,
  updateSupplier,
  type SupplierWrite
} from './platformSuppliers'

const id = '11111111-1111-4111-8111-111111111111'
const intent = '22222222-2222-4222-8222-222222222222'
const record = {
  id,
  name: 'Acme',
  kind: 'vendor',
  protocol: 'openai',
  baseUrl: 'https://upstream.example/v1',
  hasKey: true,
  keyHint: '…test',
  dataRegion: null,
  retainsData: null,
  originVendors: ['exact vendor'],
  models: ['exact/model'],
  enabled: true,
  routable: true,
  createdAt: 100,
  updatedAt: 101
}
const input: SupplierWrite = {
  name: 'Acme',
  kind: 'vendor',
  protocol: 'openai',
  baseUrl: record.baseUrl,
  dataRegion: null,
  retainsData: null,
  originVendors: [],
  models: [],
  enabled: true,
  credential: { action: 'keep' }
}
function response(data: unknown, status = 200) {
  return vi.fn<typeof fetch>(
    async () => new Response(status === 204 ? null : JSON.stringify({ code: 0, data }), { status })
  )
}
beforeEach(() =>
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) },
    configurable: true
  })
)
afterEach(() => {
  vi.useRealTimers()
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
})

describe('supplier member transport', () => {
  const relatedQuery = { kind: 'models' as const, page: 1, limit: 20, search: '', model: '' }
  const related = {
    supplierId: id,
    kind: 'models',
    items: [{ id: 'exact/model', name: 'exact/model', state: null, upstreamModels: [], dependencyCount: 3 }],
    total: 1,
    page: 1,
    limit: 20,
    revision: 101,
    dependencyTotal: 3
  }
  it('reads only the addressed related page and strips unapproved private metadata', async () => {
    const fetcher = response({
      ...related,
      secret: 'private',
      items: [{ ...related.items[0], routes: ['private-route'], apiKey: 'private-key' }]
    })
    expect(await readSupplierRelations(id, { ...relatedQuery, search: "%_'/" }, fetcher)).toEqual(related)
    const [url, init] = fetcher.mock.calls[0]
    const parsed = new URL(String(url))
    expect(parsed.pathname).toBe(`/api/member/platform/suppliers/${id}/models/page`)
    expect(parsed.searchParams.get('search')).toBe("%_'/")
    expect(init?.method).toBe('GET')
  })
  it.each([
    { ...related, supplierId: intent },
    { ...related, kind: 'dependencies' },
    { ...related, total: -1 },
    { ...related, total: 21 },
    { ...related, page: 0 },
    { ...related, limit: 50 },
    { ...related, revision: -1 },
    { ...related, total: 2, items: [related.items[0], related.items[0]] },
    { ...related, items: [{ ...related.items[0], dependencyCount: 4 }] },
    { ...related, items: [{ ...related.items[0], upstreamModels: ['foreign'] }] }
  ])('refuses unrelated or inconsistent related-page metadata', async (value) => {
    await expect(readSupplierRelations(id, relatedQuery, response(value))).rejects.toThrow('invalid')
  })
  it('accepts corrected last pages and only approved dependency fields with exact model filtering', async () => {
    const query = { kind: 'dependencies' as const, page: 9, limit: 10, search: '', model: 'undeclared/exact%_' }
    const dependency = {
      id: 'offering/name',
      name: 'Model',
      state: 'draft',
      upstreamModels: [query.model],
      dependencyCount: 0
    }
    const page = {
      ...related,
      kind: 'dependencies',
      page: 3,
      limit: 10,
      total: 21,
      dependencyTotal: 33,
      items: [dependency]
    }
    const fetcher = response(page)
    expect(await readSupplierRelations(id, query, fetcher)).toEqual(page)
    expect(new URL(String(fetcher.mock.calls[0][0])).searchParams.get('model')).toBe(query.model)
    await expect(readSupplierRelations(id, query, response({ ...page, dependencyTotal: 20 }))).rejects.toThrow(
      'invalid'
    )
    await expect(
      readSupplierRelations(id, query, response({ ...page, items: [{ ...dependency, state: null }] }))
    ).rejects.toThrow('invalid')
  })
  it('returns only approved read metadata and refuses another addressed record', async () => {
    const value = await readSupplier(
      id,
      response({ ...record, apiKey: 'must-not-enter-cache', sealed: 'must-not-enter-cache' })
    )
    expect(value).toEqual(record)
    await expect(readSupplier(id, response({ ...record, id: intent }))).rejects.toThrow('invalid')
  })
  it.each([
    { ...record, updatedAt: -1 },
    { ...record, updatedAt: Number.MAX_SAFE_INTEGER + 1 },
    { ...record, kind: 'unknown' },
    { ...record, hasKey: 'yes' },
    { ...record, models: [1] }
  ])('rejects malformed authoritative metadata', async (value) => {
    await expect(readSupplier(id, response(value))).rejects.toThrow('invalid')
  })
  it('requires explicit complete access and keeps create/update/delete independent', async () => {
    await expect(
      readSupplierAccess(response({ read: true, create: false, update: true, delete: false }))
    ).resolves.toEqual({ read: true, create: false, update: true, delete: false })
    await expect(readSupplierAccess(response({ read: true, create: true }))).rejects.toThrow('invalid')
    await expect(
      readSupplierAccess(response({ read: false, create: true, update: true, delete: true }))
    ).rejects.toThrow('invalid')
  })
  it('sends the durable create intent and reviewed update/remove revisions through the member proxy', async () => {
    const created = response({ id, revision: 100 }, 201)
    await createSupplier(intent, input, created)
    const [url, init] = created.mock.calls[0]
    expect(String(url)).toBe('http://127.0.0.1:9/api/member/platform/suppliers')
    expect(JSON.parse(String(init?.body))).toEqual({ intentId: intent, supplier: input })
    const edited = response(record)
    await updateSupplier(id, 100, { ...input, credential: { action: 'clear' } }, edited)
    expect(JSON.parse(String(edited.mock.calls[0][1]?.body))).toMatchObject({
      revision: 100,
      supplier: { credential: { action: 'clear' } }
    })
    const removed = response(null, 204)
    await deleteSupplier(id, 101, removed)
    expect(JSON.parse(String(removed.mock.calls[0][1]?.body))).toEqual({ revision: 101 })
  })
  it('refuses fabricated completion and unchanged update revisions', async () => {
    await expect(createSupplier(intent, input, response({ id, revision: 100 }, 202))).rejects.toThrow('invalid')
    await expect(updateSupplier(id, 101, input, response(record))).rejects.toThrow('invalid')
    await expect(deleteSupplier(id, 101, response(null))).rejects.toThrow('invalid')
  })
  it('resolves an addressed create intent without resending its payload or trusting extra metadata', async () => {
    const fetcher = response({ state: 'created', id, revision: 100, apiKey: 'discard-me' })
    await expect(resolveSupplierCreate(intent, fetcher)).resolves.toEqual({ state: 'created', id, revision: 100 })
    const [url, init] = fetcher.mock.calls[0]
    expect(String(url)).toBe(`http://127.0.0.1:9/api/member/platform/suppliers/intents/${intent}/resolve`)
    expect(init?.method).toBe('POST')
    expect(JSON.parse(String(init?.body))).toEqual({})
    await expect(resolveSupplierCreate(intent, response({ state: 'canceled' }))).resolves.toEqual({ state: 'canceled' })
  })
  it('refuses an unconfirmed or malformed recovery result and invalid intent before dispatch', async () => {
    for (const result of [
      { state: 'pending' },
      { state: 'created', id, revision: -1 },
      { state: 'created', id: '00000000-0000-0000-0000-000000000000', revision: 100 },
      { state: 'created', id }
    ]) {
      await expect(resolveSupplierCreate(intent, response(result))).rejects.toThrow('invalid')
    }
    await expect(resolveSupplierCreate(intent, response({ state: 'canceled' }, 202))).rejects.toThrow('invalid')
    const fetcher = response({ state: 'canceled' })
    await expect(resolveSupplierCreate('../other', fetcher)).rejects.toThrow('invalid')
    expect(fetcher).not.toHaveBeenCalled()
  })
  it('bounds the whole response read when headers arrive but the body stalls', async () => {
    vi.useFakeTimers()
    const stalled = vi.fn<typeof fetch>(
      async (_url, init) =>
        new Response(
          new ReadableStream({
            start(controller) {
              init?.signal?.addEventListener('abort', () => controller.error(new DOMException('Aborted', 'AbortError')))
            }
          })
        )
    )
    const rejected = expect(readSupplier(id, stalled)).rejects.toMatchObject({ name: 'AbortError' })
    await vi.advanceTimersByTimeAsync(12_000)
    await rejected
  })
})
