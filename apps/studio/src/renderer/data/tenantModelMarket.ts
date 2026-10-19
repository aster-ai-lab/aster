import { studioModelServiceName } from '@renderer/tenantBrand'

/** One model the launch tenant publishes. The desktop does not add or edit these. */
export type TenantMarketModel = {
  id: string
  name: string
  summary: string
  category: string
  vendor: string
  endpoint: string
  tags: string[]
}

const MARKET_PATH = '/tenant-model-market.json'

let cached: Promise<TenantMarketModel[]> | null = null

export function resetTenantModelMarketCache(): void {
  cached = null
}

/** Models from the tenant catalog. Empty when this launch has no model service. */
export function loadTenantModelMarket(): Promise<TenantMarketModel[]> {
  if (!studioModelServiceName()) return Promise.resolve([])
  cached ??= fetchTenantModelMarket()
  return cached
}

async function fetchTenantModelMarket(): Promise<TenantMarketModel[]> {
  try {
    const response = await fetch(MARKET_PATH)
    if (!response.ok) return []
    return parseTenantModelMarket(await response.json())
  } catch {
    return []
  }
}

export function parseTenantModelMarket(value: unknown): TenantMarketModel[] {
  if (!value || typeof value !== 'object') return []
  const models = (value as { models?: unknown }).models
  if (!Array.isArray(models)) return []
  const parsed: TenantMarketModel[] = []
  for (const item of models) {
    const model = parseMarketModel(item)
    if (model) parsed.push(model)
  }
  return parsed
}

function parseMarketModel(value: unknown): TenantMarketModel | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  const id = text(row.id)
  const name = text(row.name)
  const category = text(row.category)
  if (!id || !name || !category) return null
  const tags = Array.isArray(row.tags) ? row.tags.map(text).filter((tag): tag is string => tag !== null) : []
  return {
    id,
    name,
    summary: text(row.summary) ?? '',
    category,
    vendor: text(row.vendor) ?? '',
    endpoint: text(row.endpoint) ?? '',
    tags
  }
}

function text(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const trimmed = value.trim()
  return trimmed.length > 0 ? trimmed : null
}
