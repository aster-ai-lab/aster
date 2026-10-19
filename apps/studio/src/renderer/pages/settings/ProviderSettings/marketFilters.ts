import type { TenantMarketModel } from '@renderer/data/tenantModelMarket'

export type MarketSort = 'name-asc' | 'name-desc'
export type MarketView = 'cards' | 'list'

export type MarketQuery = {
  query: string
  vendor: string
  category: string
  tag: string
  sort: MarketSort
}

export const EMPTY_MARKET_QUERY: MarketQuery = {
  query: '',
  vendor: '',
  category: '',
  tag: '',
  sort: 'name-asc'
}

export function uniqueValues(models: readonly TenantMarketModel[], read: (model: TenantMarketModel) => string): string[] {
  const values: string[] = []
  for (const model of models) {
    const value = read(model).trim()
    if (value && !values.includes(value)) values.push(value)
  }
  return values
}

export function marketVendors(models: readonly TenantMarketModel[]): string[] {
  return uniqueValues(models, (model) => model.vendor)
}

export function marketCategories(models: readonly TenantMarketModel[]): string[] {
  return uniqueValues(models, (model) => model.category)
}

export function marketTags(models: readonly TenantMarketModel[]): string[] {
  const tags: string[] = []
  for (const model of models) {
    for (const tag of model.tags) {
      if (!tags.includes(tag)) tags.push(tag)
    }
  }
  return tags
}

export function filterServiceModels(models: readonly TenantMarketModel[], query: MarketQuery): TenantMarketModel[] {
  const needle = query.query.trim().toLowerCase()
  const matched = models.filter((model) => {
    if (query.vendor && model.vendor !== query.vendor) return false
    if (query.category && model.category !== query.category) return false
    if (query.tag && !model.tags.includes(query.tag)) return false
    if (!needle) return true
    return [model.name, model.id, model.summary, model.vendor, model.endpoint, model.category, ...model.tags].some(
      (field) => field.toLowerCase().includes(needle)
    )
  })
  return [...matched].sort((left, right) => {
    const order = left.name.localeCompare(right.name, 'zh')
    return query.sort === 'name-desc' ? -order : order
  })
}

export function marketFiltersActive(query: MarketQuery): boolean {
  return query.vendor.length > 0 || query.category.length > 0 || query.tag.length > 0
}
