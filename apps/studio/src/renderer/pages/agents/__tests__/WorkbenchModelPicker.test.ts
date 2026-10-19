import { describe, expect, it, vi } from 'vitest'

import type { ModelEntry } from '@renderer/data/modelProviders'
import { toOrgModel, type OrgModel } from '@renderer/data/orgModels'

import {
  buildPickerGroups,
  catalogOptionSelectable,
  catalogPickerGroups,
  compactTokenCount,
  filterOrgEntries,
  formatPriceMultiplier,
  formatPublishedRate,
  formatUnitPrice,
  composerReasoningEffort,
  reasoningEffortColor,
  reasoningEffortLadder,
  reasoningEffortScale,
  visibleReasoningEffort
} from '../WorkbenchModelPicker'

vi.mock('@renderer/pages/settings/ProviderSettings/components/ProviderAvatar', () => ({ ProviderAvatar: () => null }))

function entry(providerId: string, id: string, providerName = providerId): ModelEntry {
  return {
    key: `${providerId}/${id}`,
    providerId,
    providerName,
    id,
    name: id,
    reasoning: false,
    toolCall: false,
    attachment: false,
    contextWindow: null,
    outputLimit: null,
    inputModalities: ['text'],
    outputModalities: ['text'],
    inputCost: null,
    outputCost: null
  }
}

describe('buildPickerGroups', () => {
  it('puts configured providers first and marks the rest as needing setup', () => {
    const items = [
      entry('anthropic', 'claude'),
      entry('zhipuai', 'glm-4.6', 'Zhipu AI'),
      entry('zhipuai', 'glm-4.5', 'Zhipu AI')
    ]
    const groups = buildPickerGroups(
      items,
      new Set(['anthropic', 'zhipu']),
      new Set(['zhipu']),
      new Set(items.map((item) => item.key))
    )

    expect(groups.map((group) => group.key)).toEqual(['zhipuai', 'anthropic'])
    expect(groups[0].options.map((option) => option.id)).toEqual(['glm-4.5', 'glm-4.6'])
    expect(groups[0].options[0]).toMatchObject({ configured: true, studioProviderId: 'zhipu' })
    expect(groups[1].options[0]).toMatchObject({ configured: false, studioProviderId: 'anthropic' })
  })

  it('ranks official vendors ahead of resellers', () => {
    const groups = buildPickerGroups(
      [entry('302ai', 'gpt-5', '302.AI'), entry('openai', 'gpt-5', 'OpenAI'), entry('deepseek', 'chat', 'DeepSeek')],
      new Set(),
      new Set(),
      new Set()
    )

    expect(groups.map((group) => group.key)).toEqual(['deepseek', 'openai', '302ai'])
  })

  it('only lets members pick what the organization serves', () => {
    const groups = buildPickerGroups(
      [entry('deepseek', 'chat'), entry('deepseek', 'reasoner')],
      new Set(['deepseek']),
      new Set(['deepseek']),
      new Set(['deepseek/chat'])
    )

    expect(groups[0].configured).toBe(true)
    expect(groups[0].options.map((option) => [option.id, option.configured])).toEqual([
      ['chat', true],
      ['reasoner', false]
    ])
  })

  it('serves organization models from the signed-in gateway when the catalog marks them available', () => {
    const alias = entry('deepseek', 'deepseek-v4-flash', 'DeepSeek')
    alias.key = 'org:deepseek-v4-flash'
    alias.logoId = 'deepseek'
    const groups = buildPickerGroups([alias], new Set(), new Set(), new Set(['org:deepseek-v4-flash']))
    expect(groups[0].options[0]).toMatchObject({
      configured: true,
      studioProviderId: 'a3s',
      id: 'deepseek-v4-flash',
      logoId: 'deepseek'
    })
  })
})

function catalogFixture(raw: Record<string, unknown>): OrgModel {
  const model = toOrgModel({ capabilities: [], available: true, ...raw })
  if (!model) throw new Error('catalog fixture needs an id')
  return model
}

describe('catalogPickerGroups', () => {
  const catalog = [
    catalogFixture({
      id: 'deepseek-v4-flash',
      name: 'Boyue DeepSeek V4 Flash',
      brand: 'DeepSeek'
    }),
    catalogFixture({
      id: 'bailian/deepseek-v4.1-flash',
      name: 'Boyue DeepSeek V4.1 Flash',
      brand: 'DeepSeek'
    }),
    catalogFixture({
      id: 'offline-model',
      name: 'Offline',
      brand: 'Other',
      available: false
    })
  ]

  it('uses the member catalog as the only row source and search narrows those aliases', () => {
    const listed = catalogPickerGroups(catalog, '').flatMap((group) => group.options.map((option) => option.id))
    expect([...listed].sort()).toEqual(['bailian/deepseek-v4.1-flash', 'deepseek-v4-flash', 'offline-model'])
    expect(listed).not.toContain('gpt-4o')

    expect(catalogPickerGroups(catalog, 'v4.1').flatMap((group) => group.options.map((option) => option.id))).toEqual([
      'bailian/deepseek-v4.1-flash'
    ])
    expect(catalogPickerGroups(catalog, 'not-in-catalog').flatMap((group) => group.options)).toEqual([])
  })

  it('keeps an unavailable catalog row visible and not selectable', () => {
    const options = catalogPickerGroups(catalog, '').flatMap((group) => group.options)
    const offline = options.find((option) => option.id === 'offline-model')
    expect(offline?.studioProviderId).toBe('a3s')
    expect(catalogOptionSelectable(offline!)).toBe(false)
    expect(
      options
        .filter(catalogOptionSelectable)
        .map((option) => option.id)
        .sort()
    ).toEqual(['bailian/deepseek-v4.1-flash', 'deepseek-v4-flash'])
  })
})

describe('filterOrgEntries', () => {
  it('matches name, upstream id, and provider case-insensitively', () => {
    const entries = [entry('deepseek', 'deepseek-chat', 'DeepSeek'), entry('moonshotai', 'kimi-k2', 'Moonshot')]
    expect(filterOrgEntries(entries, '  KIMI ').map((item) => item.id)).toEqual(['kimi-k2'])
    expect(filterOrgEntries(entries, 'deepseek').map((item) => item.id)).toEqual(['deepseek-chat'])
    expect(filterOrgEntries(entries, '')).toHaveLength(2)
  })
})

describe('compactTokenCount', () => {
  it('abbreviates context sizes', () => {
    expect(compactTokenCount(131072)).toBe('131.1K')
    expect(compactTokenCount(1_000_000)).toBe('1M')
    expect(compactTokenCount(64000)).toBe('64K')
    expect(compactTokenCount(null)).toBeNull()
  })
})

describe('formatUnitPrice', () => {
  it('trims trailing zeros on a per-million price', () => {
    expect(formatUnitPrice(0.44)).toBe('0.44')
    expect(formatUnitPrice(1.2)).toBe('1.2')
    expect(formatUnitPrice(0.3)).toBe('0.3')
  })
})

describe('formatPriceMultiplier', () => {
  it('trims a billing coefficient', () => {
    expect(formatPriceMultiplier(1)).toBe('1')
    expect(formatPriceMultiplier(1.5)).toBe('1.5')
  })
})

describe('formatPublishedRate', () => {
  const t = (key: string, values: { value: string }) => `${key}:${values.value}`

  it('includes cache prices only when they are published', () => {
    expect(formatPublishedRate(t, { input: 0.3, output: 1.2 })).toBe(
      'workbench.model_picker.price_input:0.3 · workbench.model_picker.price_output:1.2'
    )
    expect(formatPublishedRate(t, { input: 0.3, output: 1.2, cacheRead: 0.03, cacheWrite: 0.375 })).toBe(
      'workbench.model_picker.price_input:0.3 · workbench.model_picker.price_cache_read:0.03 · workbench.model_picker.price_cache_write:0.375 · workbench.model_picker.price_output:1.2'
    )
  })
})

describe('reasoningEffortLadder', () => {
  it('uses the DeepSeek V4 effort vocabulary, including a namespaced alias', () => {
    expect(reasoningEffortLadder('deepseek-v4-flash')).toEqual(['none', 'low', 'high', 'max'])
    expect(reasoningEffortLadder('bailian/deepseek-v4.1-flash')).toEqual(['none', 'low', 'high', 'max'])
  })

  it('stays empty when the id declares no intensity ladder', () => {
    expect(reasoningEffortLadder('text-embedding-3-small')).toEqual([])
  })

  it('uses a published list in canonical order and keeps an empty list empty', () => {
    expect(reasoningEffortLadder('deepseek-v4-flash', ['max', 'high', 'high', 'turbo'])).toEqual(['high', 'max'])
    expect(reasoningEffortLadder('deepseek-v4-flash', [])).toEqual([])
    expect(reasoningEffortLadder('text-embedding-3-small', ['low'])).toEqual(['low'])
    expect(reasoningEffortLadder('deepseek-v4-flash', null)).toEqual(['none', 'low', 'high', 'max'])
  })
})

describe('visibleReasoningEffort', () => {
  it('shows the chosen effort for a reasoning model and hides it otherwise', () => {
    expect(visibleReasoningEffort('deepseek-v4-flash', 'high')).toBe('high')
    expect(visibleReasoningEffort('deepseek-v4-flash', 'medium')).toBe('default')
    expect(visibleReasoningEffort('text-embedding-3-small', 'high')).toBeNull()
    expect(visibleReasoningEffort('deepseek-v4-flash', 'low', [])).toBeNull()
    expect(visibleReasoningEffort('text-embedding-3-small', 'high', ['high'])).toBe('high')
  })
})

describe('reasoningEffortScale', () => {
  it('places 默认 between 关闭 and the rising intensities', () => {
    expect(reasoningEffortScale(['none', 'low', 'high', 'max'])).toEqual(['none', 'default', 'low', 'high', 'max'])
    expect(reasoningEffortScale(['high'])).toEqual(['default', 'high'])
    expect(reasoningEffortScale([])).toEqual([])
  })
})

describe('composerReasoningEffort', () => {
  it('keeps only intensities above 默认 beside the model name', () => {
    expect(composerReasoningEffort('deepseek-v4-flash', 'default')).toBeNull()
    expect(composerReasoningEffort('deepseek-v4-flash', 'none')).toBeNull()
    expect(composerReasoningEffort('deepseek-v4-flash', 'low')).toBe('low')
    expect(composerReasoningEffort('deepseek-v4-flash', 'high')).toBe('high')
    expect(composerReasoningEffort('deepseek-v4-flash', 'max')).toBe('max')
    expect(composerReasoningEffort('text-embedding-3-small', 'high')).toBeNull()
    expect(composerReasoningEffort('text-embedding-3-small', 'high', ['high'])).toBe('high')
    expect(composerReasoningEffort('deepseek-v4-flash', 'low', ['high', 'max'])).toBeNull()
  })
})

describe('reasoningEffortColor', () => {
  it('uses a different text color for each rising intensity', () => {
    const colors = ['low', 'high', 'max'].map((effort) => reasoningEffortColor(effort as 'low'))
    expect(new Set(colors).size).toBe(colors.length)
    expect(reasoningEffortColor('default')).not.toBe(reasoningEffortColor('high'))
  })
})
