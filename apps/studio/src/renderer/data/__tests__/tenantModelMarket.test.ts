import { describe, expect, it } from 'vitest'

import { parseTenantModelMarket } from '../tenantModelMarket'
import { filterMarketModels, marketCategories } from '../../pages/settings/ProviderSettings/ModelList/TenantModelMarket'

const catalog = {
  models: [
    {
      id: 'deepseek-v4-flash',
      name: 'DeepSeek V4 Flash',
      summary: 'OpenAI 兼容对话。',
      category: '对话',
      tags: ['工具调用']
    },
    {
      id: 'deepseek-v4.1-flash',
      name: 'DeepSeek V4.1 Flash',
      summary: 'OpenAI 兼容对话。',
      category: '对话',
      tags: ['工具调用']
    },
    { id: ' ', name: 'blank', category: '对话' },
    { id: 'kept', name: 'Kept', category: '向量' }
  ]
}

describe('tenant model market', () => {
  it('keeps complete tenant rows and drops blank ids', () => {
    const models = parseTenantModelMarket(catalog)
    expect(models.map((model) => model.id)).toEqual(['deepseek-v4-flash', 'deepseek-v4.1-flash', 'kept'])
    expect(models[0]).toMatchObject({ name: 'DeepSeek V4 Flash', category: '对话', tags: ['工具调用'] })
    expect(marketCategories(models)).toEqual(['对话', '向量'])
  })

  it('filters by category and search text', () => {
    const models = parseTenantModelMarket(catalog)
    expect(filterMarketModels(models, 'v4.1', '__all__').map((model) => model.id)).toEqual(['deepseek-v4.1-flash'])
    expect(filterMarketModels(models, '', '向量').map((model) => model.id)).toEqual(['kept'])
    expect(parseTenantModelMarket(null)).toEqual([])
  })
})
