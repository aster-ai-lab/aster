import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const assistantCatalogMocks = vi.hoisted(() => ({
  language: 'en-US',
  read: vi.fn(),
  resourcesPath: '/resources'
}))

vi.mock('@data/hooks/useCache', () => ({
  useCache: () => [assistantCatalogMocks.resourcesPath]
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: { language: assistantCatalogMocks.language },
    t: (key: string) => key
  })
}))

import {
  ASSISTANT_CATALOG_MY_TAB,
  buildAssistantCatalogTabs,
  catalogAgentEntityId,
  filterAssistantCatalogPresets,
  getAssistantPresetCatalogKey,
  isCatalogAgentEntityId,
  loadPublishedAssistantPresets,
  toCreateAssistantDtoFromCatalogPreset,
  unpublishedCatalogAgents,
  useAssistantCatalogPresets,
  usePublishedCatalogAgents
} from '../useAssistantCatalogPresets'

describe('assistant catalog presets', () => {
  it('refreshes composer agent availability after install and uninstall without a reload', async () => {
    vi.stubGlobal('__A3S_STUDIO_RUNTIME__', { apiBaseUrl: 'http://127.0.0.1:4010', token: 'ef'.repeat(32) })
    let installed = false
    vi.stubGlobal(
      'fetch',
      vi.fn(
        async (input: RequestInfo | URL) =>
          new Response(
            JSON.stringify({
              code: 0,
              data: {
                items: String(input).includes('/api/capabilities?')
                  ? [
                      {
                        slug: 'a3s-review',
                        kind: 'plugin',
                        name: '合同审查',
                        installed,
                        surfaces: ['skill', 'agent'],
                        agentRuntime: {
                          runtime: 'edge',
                          harness: 'builtin',
                          resources: { cpuMillis: 500, memoryBytes: 268435456 }
                        },
                        install: {
                          type: 'plugin',
                          packageType: 'aix',
                          packageId: 'use/a3s/review',
                          digest: `sha256:${'a'.repeat(64)}`,
                          size: 1024
                        }
                      }
                    ]
                  : []
              }
            }),
            { headers: { 'content-type': 'application/json' } }
          )
      )
    )
    const { result, unmount } = renderHook(() => useAssistantCatalogPresets())
    try {
      await waitFor(() =>
        expect(result.current.presets.find((row) => row.packageSlug === 'a3s-review')?.installed).toBe(false)
      )
      installed = true
      act(() => window.dispatchEvent(new Event('a3s:plugin-install')))
      await waitFor(() =>
        expect(result.current.presets.find((row) => row.packageSlug === 'a3s-review')?.installed).toBe(true)
      )
      installed = false
      act(() => window.dispatchEvent(new Event('a3s:plugin-install')))
      await waitFor(() =>
        expect(result.current.presets.find((row) => row.packageSlug === 'a3s-review')?.installed).toBe(false)
      )
    } finally {
      unmount()
      vi.unstubAllGlobals()
    }
  })
  beforeEach(() => {
    assistantCatalogMocks.language = 'en-US'
    assistantCatalogMocks.read.mockReset()
    assistantCatalogMocks.read.mockResolvedValue('[]')
    delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
    Object.assign(window, {
      api: {
        ...window.api,
        fs: {
          ...window.api?.fs,
          read: assistantCatalogMocks.read
        }
      }
    })
  })

  it('loads every local assistant catalog preset for picker reuse', async () => {
    assistantCatalogMocks.read.mockResolvedValue(
      JSON.stringify([
        { id: 'preset-product', name: 'Product Manager', group: ['Career'] },
        { id: 'invalid-missing-name' },
        { id: 'preset-business', name: 'Business Helper', group: ['Business'] }
      ])
    )

    const { result } = renderHook(() => useAssistantCatalogPresets())

    await waitFor(() => expect(result.current.presets).toHaveLength(2))
    expect(assistantCatalogMocks.read).toHaveBeenCalledWith('/resources/data/agents-en.json', 'utf-8')
    expect(result.current.presets.map((preset) => preset.id)).toEqual(['preset-product', 'preset-business'])
    expect(result.current.isLoading).toBe(false)
  })

  it('puts an enabled Console agent ahead of a bundled preset with the same name', async () => {
    const token = 'ab'.repeat(32)
    Object.assign(globalThis, {
      __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:4010', token }
    })
    const fetchMock = vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            code: 0,
            data: {
              items: [
                {
                  slug: 'writer',
                  name: 'Product Manager',
                  description: '来自目录',
                  prompt: '用目录提示',
                  emoji: '文',
                  category: 'office',
                  featured: true
                },
                {
                  slug: 'acp-cloud',
                  name: 'ACP 云端智能体',
                  description: 'ACP protocolVersion 1',
                  prompt: '记住上一轮',
                  emoji: '云',
                  category: 'code',
                  featured: true
                }
              ]
            }
          }),
          { status: 200, headers: { 'content-type': 'application/json' } }
        )
    )
    vi.stubGlobal('fetch', fetchMock)
    assistantCatalogMocks.read.mockResolvedValue(
      JSON.stringify([{ id: 'preset-product', name: 'Product Manager', prompt: '旧提示', group: ['Career'] }])
    )

    const { result } = renderHook(() => useAssistantCatalogPresets())
    await waitFor(() => expect(result.current.presets.some((preset) => preset.id === 'writer')).toBe(true))
    expect(result.current.presets).toEqual([
      expect.objectContaining({
        id: 'writer',
        name: 'Product Manager',
        prompt: '用目录提示',
        group: ['Featured', 'Office']
      }),
      expect.objectContaining({
        id: 'acp-cloud',
        name: 'ACP 云端智能体',
        prompt: '记住上一轮',
        group: ['Featured', 'Programming']
      })
    ])
    expect(
      toCreateAssistantDtoFromCatalogPreset(result.current.presets.find((preset) => preset.id === 'acp-cloud')!)
    ).toMatchObject({
      name: 'ACP 云端智能体',
      prompt: '记住上一轮',
      emoji: '云'
    })
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:4010/api/catalog-agents?pageSize=100',
      expect.objectContaining({ method: 'GET' })
    )
    delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
    vi.unstubAllGlobals()
    Object.assign(window, {
      api: {
        fs: { read: assistantCatalogMocks.read }
      }
    })
  })

  it('reads the published catalog as assistant presets', async () => {
    const rows = await loadPublishedAssistantPresets('zh-CN')
    expect(rows).toEqual([])
  })

  it('lists an agent package but leaves a package with a view to the plugin catalog', async () => {
    Object.assign(globalThis, {
      __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:4010', token: 'ef'.repeat(32) }
    })
    const aix = (slug: string) => ({
      type: 'plugin',
      packageType: 'aix',
      packageId: `use/a3s/${slug}`,
      digest: `sha256:${'a'.repeat(64)}`,
      size: 1024
    })
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      if (String(input).includes('/review/icon')) {
        return new Response('<svg/>', { headers: { 'content-type': 'image/svg+xml' } })
      }
      const items = String(input).includes('/api/capabilities?')
        ? [
            {
              slug: 'review',
              kind: 'plugin',
              name: '法务专家',
              surfaces: ['agent', 'skill'],
              install: aix('review'),
              iconUrl: '/v1/api/org/capabilities/review/icon?v=1'
            },
            {
              slug: 'cobot-arm',
              kind: 'plugin',
              name: '七轴轻量化协作机器人',
              surfaces: ['agent', 'skill', 'ui'],
              install: aix('cobot-arm')
            }
          ]
        : []
      return new Response(JSON.stringify({ code: 0, data: { items } }), {
        status: 200,
        headers: { 'content-type': 'application/json' }
      })
    })
    vi.stubGlobal('fetch', fetchMock)

    const rows = await loadPublishedAssistantPresets('zh-CN')

    expect(rows.map((preset) => preset.id)).toEqual(['review'])
    // The icon path needs the member token; the card gets a self-contained image instead.
    expect(rows[0]?.avatar).toMatch(/^data:image\/svg\+xml;base64,/)
    expect(rows[0]?.emoji).toBe(rows[0]?.avatar)
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:4010/api/capabilities/review/icon?v=1',
      expect.objectContaining({ headers: { authorization: `Bearer ${'ef'.repeat(32)}` } })
    )
    delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
    vi.unstubAllGlobals()
  })

  it('does not load assistant presets while disabled', () => {
    const { result } = renderHook(() => useAssistantCatalogPresets({ enabled: false }))

    expect(assistantCatalogMocks.read).not.toHaveBeenCalled()
    expect(result.current).toEqual({ isLoading: false, presets: [] })
  })

  it('keeps my assistants first and orders known system groups before custom groups', () => {
    const tabs = buildAssistantCatalogTabs(
      [
        { id: 'preset-business', name: 'Business helper', group: ['Business'] },
        { id: 'preset-career', name: 'Career helper', group: ['Career'] },
        { id: 'preset-custom', name: 'Custom helper', group: ['Custom'] }
      ],
      2,
      'Mine'
    )

    expect(tabs.map((tab) => tab.id)).toEqual([ASSISTANT_CATALOG_MY_TAB, 'Career', 'Business', 'Custom'])
    expect(tabs.map((tab) => tab.count)).toEqual([2, 1, 1, 1])
  })

  it('filters only the active system group by name or description', () => {
    const presets = filterAssistantCatalogPresets(
      [
        { id: 'preset-product', name: 'Product Manager', description: 'Plan requirements', group: ['Career'] },
        { id: 'preset-marketing', name: 'Marketing Planner', description: 'Campaign ideas', group: ['Business'] },
        { id: 'preset-research', name: 'Research Writer', description: 'Long-form product analysis', group: ['Career'] }
      ],
      'Career',
      'product'
    )

    expect(presets.map((preset) => preset.name)).toEqual(['Product Manager', 'Research Writer'])
    expect(filterAssistantCatalogPresets(presets, ASSISTANT_CATALOG_MY_TAB, 'product')).toEqual([])
  })

  it('builds a DataApi create payload from a catalog preset without legacy fields', () => {
    const dto = toCreateAssistantDtoFromCatalogPreset({
      id: '550e8400-e29b-41d4-a716-446655440000',
      name: ' Product Manager ',
      prompt: ' You are a product manager. ',
      description: ' Plan requirements ',
      emoji: ' PM ',
      group: ['Career'],
      defaultModel: {
        id: 'gpt-4o',
        provider: 'openai',
        name: 'GPT-4o',
        group: 'OpenAI'
      }
    })

    expect(dto).toEqual({
      name: 'Product Manager',
      prompt: 'You are a product manager.',
      description: 'Plan requirements',
      emoji: 'PM',
      modelId: 'openai::gpt-4o'
    })
  })

  it('keeps an OS catalog agent that is not already a local agent', () => {
    const published = [
      { id: 'acp-cloud', name: 'ACP 云端智能体', prompt: '记住 sessionId', group: ['Programming'] },
      { id: 'writer', name: 'Writer', prompt: '', group: ['Office'] }
    ]
    expect(unpublishedCatalogAgents(new Set(['Writer']), published).map((preset) => preset.id)).toEqual(['acp-cloud'])
    expect(catalogAgentEntityId('acp-cloud')).toBe('catalog:acp-cloud')
    expect(isCatalogAgentEntityId('catalog:acp-cloud')).toBe(true)
    expect(isCatalogAgentEntityId('local-agent')).toBe(false)
  })

  it('loads the work sidebar from the OS catalog-agents member API', async () => {
    const token = 'cd'.repeat(32)
    Object.assign(globalThis, {
      __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:4010', token }
    })
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      expect(String(input)).toBe('http://127.0.0.1:4010/api/catalog-agents?pageSize=100')
      return new Response(
        JSON.stringify({
          code: 0,
          data: {
            items: [
              {
                slug: 'acp-cloud',
                name: 'ACP 云端智能体',
                description: 'ACP',
                prompt: '记住同一个 sessionId',
                emoji: '☁',
                category: 'code',
                featured: true,
                enabled: true
              }
            ]
          }
        }),
        { status: 200, headers: { 'content-type': 'application/json' } }
      )
    })
    vi.stubGlobal('fetch', fetchMock)

    const { result } = renderHook(() => usePublishedCatalogAgents())

    await waitFor(() => expect(result.current.agents).toHaveLength(1))
    expect(result.current.agents[0]?.id).toBe('acp-cloud')
    expect(result.current.agents[0]?.name).toBe('ACP 云端智能体')
    expect(result.current.error).toBeNull()
    expect(fetchMock).toHaveBeenCalled()
    vi.unstubAllGlobals()
  })

  it('uses the preset id as the stable catalog key', () => {
    expect(getAssistantPresetCatalogKey({ id: '550e8400-e29b-41d4-a716-446655440000' })).toBe(
      '550e8400-e29b-41d4-a716-446655440000'
    )
  })
})
