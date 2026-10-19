import * as z from 'zod'

import { memberRequest, MemberOrgError } from './memberOrgs'

const id = z
  .string()
  .uuid()
  .refine((value) => value !== '00000000-0000-0000-0000-000000000000')
const revision = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER)
export const SUPPLIER_KINDS = ['vendor', 'relay', 'self_hosted'] as const
export const SUPPLIER_PROTOCOLS = ['openai', 'anthropic'] as const
const recordSchema = z.object({
  id,
  name: z.string(),
  kind: z.enum(SUPPLIER_KINDS),
  protocol: z.enum(SUPPLIER_PROTOCOLS),
  baseUrl: z.string().url(),
  hasKey: z.boolean(),
  keyHint: z.string().nullable(),
  dataRegion: z.string().nullable(),
  retainsData: z.boolean().nullable(),
  originVendors: z.array(z.string()),
  models: z.array(z.string()),
  enabled: z.boolean(),
  routable: z.boolean(),
  createdAt: revision,
  updatedAt: revision
})
const accessSchema = z.object({ read: z.literal(true), create: z.boolean(), update: z.boolean(), delete: z.boolean() })
const createdSchema = z.object({ id, revision })
const resolutionSchema = z.discriminatedUnion('state', [
  createdSchema.extend({ state: z.literal('created') }),
  z.object({ state: z.literal('canceled') })
])

export type SupplierRecord = z.infer<typeof recordSchema>
export type SupplierAccess = z.infer<typeof accessSchema>
export type SupplierCreated = z.infer<typeof createdSchema>
export type SupplierCreateResolution = z.infer<typeof resolutionSchema>
export type SupplierRelationsQuery = {
  kind: 'models' | 'dependencies'
  page: number
  limit: number
  search: string
  model: string
}
const relationSchema = z.object({
  id: z.string().min(1),
  name: z.string(),
  state: z.enum(['draft', 'published', 'suspended']).nullable(),
  upstreamModels: z.array(z.string().min(1).max(200)).max(8),
  dependencyCount: revision
})
const relationsPageSchema = z.object({
  supplierId: id,
  kind: z.enum(['models', 'dependencies']),
  items: z.array(relationSchema).max(100),
  total: revision,
  page: z.number().int().min(1).max(1_000_000),
  limit: z.union([z.literal(10), z.literal(20), z.literal(50), z.literal(100)]),
  revision,
  dependencyTotal: revision
})
export type SupplierRelationsPage = z.infer<typeof relationsPageSchema>
export type SupplierWrite = {
  name: string
  kind: SupplierRecord['kind']
  protocol: SupplierRecord['protocol']
  baseUrl: string
  dataRegion: string | null
  retainsData: boolean | null
  originVendors: string[]
  models: string[]
  enabled: boolean
  credential: { action: 'keep' | 'clear' } | { action: 'replace'; value: string }
}

function decode<T>(schema: z.ZodType<T>, value: unknown): T {
  const result = schema.safeParse(value)
  if (!result.success) throw new MemberOrgError('invalid')
  return result.data
}

async function request(
  path: string,
  method: 'GET' | 'POST' | 'PUT' | 'DELETE',
  body?: unknown,
  fetcher = globalThis.fetch,
  expected = method === 'POST' ? 201 : method === 'DELETE' ? 204 : 200
) {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), 12_000)
  try {
    return await memberRequest(
      path,
      { method, ...(body === undefined ? {} : { body: JSON.stringify(body) }) },
      async (input, init) => {
        const response = await fetcher(input, { ...init, signal: controller.signal })
        if (response.ok && response.status !== expected) throw new MemberOrgError('invalid')
        return response
      }
    )
  } finally {
    clearTimeout(timeout)
  }
}

const collection = '/api/member/platform/suppliers'
export async function readSupplierRelations(
  supplierId: string,
  query: SupplierRelationsQuery,
  fetcher = globalThis.fetch
): Promise<SupplierRelationsPage> {
  decode(id, supplierId)
  if (
    !['models', 'dependencies'].includes(query.kind) ||
    !Number.isInteger(query.page) ||
    query.page < 1 ||
    query.page > 1_000_000 ||
    ![10, 20, 50, 100].includes(query.limit) ||
    [...query.search].length > 128 ||
    new TextEncoder().encode(query.model).length > 200 ||
    (query.kind === 'models' && query.model !== '')
  )
    throw new MemberOrgError('invalid')
  const params = new URLSearchParams({
    page: String(query.page),
    limit: String(query.limit),
    search: query.search,
    model: query.model
  })
  const page = decode(
    relationsPageSchema,
    await request(
      `${collection}/${encodeURIComponent(supplierId)}/${query.kind}/page?${params}`,
      'GET',
      undefined,
      fetcher
    )
  )
  const effective = Math.min(query.page, Math.max(1, Math.ceil(page.total / query.limit)))
  if (
    page.supplierId !== supplierId ||
    page.kind !== query.kind ||
    page.limit !== query.limit ||
    page.page !== effective ||
    page.items.length !== Math.min(query.limit, Math.max(0, page.total - (effective - 1) * query.limit)) ||
    new Set(page.items.map((item) => item.id)).size !== page.items.length ||
    (query.kind === 'dependencies' && page.total > page.dependencyTotal) ||
    page.items.some((item) =>
      query.kind === 'models'
        ? item.state !== null ||
          item.upstreamModels.length !== 0 ||
          item.dependencyCount > page.dependencyTotal ||
          item.id !== item.name
        : item.state === null ||
          item.upstreamModels.length === 0 ||
          item.dependencyCount !== 0 ||
          new Set(item.upstreamModels).size !== item.upstreamModels.length
    )
  )
    throw new MemberOrgError('invalid')
  return page
}
export async function resolveSupplierCreate(
  intent: string,
  fetcher = globalThis.fetch
): Promise<SupplierCreateResolution> {
  return decode(
    resolutionSchema,
    await request(`${collection}/intents/${decode(id, intent)}/resolve`, 'POST', {}, fetcher, 200)
  )
}
function addressed(value: string) {
  return `${collection}/${decode(id, value)}`
}

export async function readSupplierAccess(fetcher = globalThis.fetch): Promise<SupplierAccess> {
  return decode(accessSchema, await request(`${collection}/access`, 'GET', undefined, fetcher))
}

export async function readSupplier(value: string, fetcher = globalThis.fetch): Promise<SupplierRecord> {
  const record = decode(recordSchema, await request(addressed(value), 'GET', undefined, fetcher))
  if (record.id !== value) throw new MemberOrgError('invalid')
  return record
}

export async function createSupplier(intentId: string, supplier: SupplierWrite, fetcher = globalThis.fetch) {
  return decode(createdSchema, await request(collection, 'POST', { intentId: decode(id, intentId), supplier }, fetcher))
}

export async function updateSupplier(
  value: string,
  reviewed: number,
  supplier: SupplierWrite,
  fetcher = globalThis.fetch
) {
  const record = decode(
    recordSchema,
    await request(addressed(value), 'PUT', { revision: decode(revision, reviewed), supplier }, fetcher)
  )
  if (record.id !== value || record.updatedAt <= reviewed) throw new MemberOrgError('invalid')
  return record
}

export async function deleteSupplier(value: string, reviewed: number, fetcher = globalThis.fetch) {
  const result = await request(addressed(value), 'DELETE', { revision: decode(revision, reviewed) }, fetcher)
  if (result !== null) throw new MemberOrgError('invalid')
}
