import { afterEach, describe, expect, it, vi } from 'vitest'

import { fetchOrgModels, orgModelAsCatalogEntry, orgModelCapabilities, toOrgModel } from '../orgModels'

describe('toOrgModel', () => {
  it('reads the member catalog item and drops malformed fields', () => {
    expect(
      toOrgModel({
        id: 'deepseek-v3',
        name: 'DeepSeek V3',
        brand: 'DeepSeek',
        catalogKey: 'deepseek/deepseek-chat',
        capabilities: ['tool_call', 7, 'vision'],
        contextWindow: 131072,
        outputLimit: -1,
        inputPrice: 0,
        outputPrice: 'x',
        protocols: ['openai', 'anthropic', 1],
        source: 'relay',
        available: true
      })
    ).toEqual({
      id: 'deepseek-v3',
      name: 'DeepSeek V3',
      brand: 'DeepSeek',
      description: undefined,
      catalogKey: 'deepseek/deepseek-chat',
      protocols: ['openai', 'anthropic'],
      capabilities: ['tool_call', 'vision'],
      contextWindow: 131072,
      outputLimit: null,
      inputPrice: 0,
      cacheReadPrice: null,
      cacheWritePrice: null,
      outputPrice: null,
      priceMultiplier: null,
      reasoningEfforts: null,
      source: 'relay',
      available: true
    })
    expect(toOrgModel({ name: 'no id' })).toBeNull()
  })
})

describe('orgModelAsCatalogEntry', () => {
  const base = toOrgModel({ id: 'qwen-max', name: 'Qwen Max', brand: 'Qwen', capabilities: ['reasoning'] })!

  it('keeps the offering alias and uses the catalog key only as the vendor icon', () => {
    const entry = orgModelAsCatalogEntry({
      ...base,
      catalogKey: 'alibaba-cn/qwen-max-latest',
      protocols: ['anthropic', 'openai']
    })
    expect(entry).toMatchObject({
      key: 'org:qwen-max',
      providerId: 'qwen',
      providerName: 'Qwen',
      logoId: 'alibaba-cn',
      id: 'qwen-max',
      reasoning: true,
      toolCall: false
    })
    expect(entry.endpointTypes?.[0]).toBe('anthropic-messages')
    expect(entry.endpointTypes).toContain('openai-chat-completions')
  })

  it('groups by brand and uses the org id when no catalog key is present', () => {
    expect(orgModelAsCatalogEntry(base)).toMatchObject({
      providerId: 'qwen',
      logoId: 'qwen',
      id: 'qwen-max'
    })
    expect(orgModelAsCatalogEntry(base).priceMultiplier).toBeUndefined()
    expect(orgModelAsCatalogEntry({ ...base, priceMultiplier: 1.5 }).priceMultiplier).toBe(1.5)
    expect(orgModelAsCatalogEntry(base).reasoningEfforts).toBeUndefined()
    expect(orgModelAsCatalogEntry({ ...base, reasoningEfforts: [] }).reasoningEfforts).toEqual([])
    expect(orgModelAsCatalogEntry({ ...base, reasoningEfforts: ['max', 'high'] }).reasoningEfforts).toEqual([
      'max',
      'high'
    ])
  })

  it('keeps a catalog row usable when capabilities are missing', () => {
    const bare = { ...base, capabilities: undefined as unknown as string[] }
    expect(orgModelCapabilities(bare)).toEqual([])
    expect(orgModelAsCatalogEntry(bare)).toMatchObject({
      reasoning: false,
      toolCall: false,
      inputModalities: ['text'],
      outputModalities: ['text']
    })
  })
})

describe('fetchOrgModels', () => {
  const token = 'a'.repeat(64)

  afterEach(() => {
    vi.unstubAllGlobals()
    delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  })

  function installRuntime() {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      value: { apiBaseUrl: 'http://127.0.0.1:3000', token },
      configurable: true
    })
  }

  it('reads the member catalog from the envelope and drops rows that cannot be used', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({
          success: true,
          data: {
            items: [
              { id: 'deepseek-v4-flash', name: 'DeepSeek V4 Flash', brand: 'DeepSeek', available: true },
              { name: 'missing id', available: true },
              { id: 'bare', available: true }
            ],
            defaultModel: 'deepseek-v4-flash'
          }
        })
      }))
    )

    const catalog = await fetchOrgModels()
    expect(catalog.defaultModel).toBe('deepseek-v4-flash')
    expect(catalog.items.map((item) => item.id)).toEqual(['deepseek-v4-flash', 'bare'])
    expect(catalog.items[1]?.capabilities).toEqual([])
    expect(orgModelAsCatalogEntry(catalog.items[1]!)).toMatchObject({ name: 'bare', reasoning: false })
  })

  it('accepts a catalog that is not wrapped in the success envelope', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        status: 200,
        json: async () => ({
          items: [{ id: 'qwen-max', name: 'Qwen Max', capabilities: ['reasoning'], available: true }]
        })
      }))
    )

    const catalog = await fetchOrgModels()
    expect(catalog.items.map((item) => item.name)).toEqual(['Qwen Max'])
    expect(orgModelCapabilities(catalog.items[0]!)).toContain('reasoning')
  })
})
