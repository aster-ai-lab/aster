import { describe, expect, it } from 'vitest'

import type { TenantMarketModel } from '@renderer/data/tenantModelMarket'

import { filterServiceModels, marketFiltersActive, marketTags, marketVendors } from '../marketFilters'

const models: TenantMarketModel[] = [
  {
    id: 'deepseek-v4-flash',
    name: 'DeepSeek V4 Flash',
    summary: 'OpenAI 兼容对话。',
    category: '对话',
    vendor: 'DeepSeek',
    endpoint: 'OpenAI',
    tags: ['工具调用']
  },
  {
    id: 'deepseek-v4.1-flash',
    name: 'DeepSeek V4.1 Flash',
    summary: 'OpenAI 兼容对话。',
    category: '对话',
    vendor: 'DeepSeek',
    endpoint: 'OpenAI',
    tags: ['工具调用']
  }
]

const base = { query: '', vendor: '', category: '', tag: '', sort: 'name-asc' as const }

describe('model service market', () => {
  it('filters by vendor, tag, and search, then sorts by name', () => {
    expect(marketVendors(models)).toEqual(['DeepSeek'])
    expect(marketTags(models)).toEqual(['工具调用'])
    expect(filterServiceModels(models, { ...base, query: 'v4.1' }).map((model) => model.id)).toEqual([
      'deepseek-v4.1-flash'
    ])
    expect(filterServiceModels(models, { ...base, tag: '工具调用', sort: 'name-desc' }).map((model) => model.name)).toEqual([
      'DeepSeek V4.1 Flash',
      'DeepSeek V4 Flash'
    ])
    expect(filterServiceModels(models, { ...base, vendor: 'Other' })).toEqual([])
    expect(marketFiltersActive({ ...base, category: '对话' })).toBe(true)
    expect(marketFiltersActive(base)).toBe(false)
  })
})
