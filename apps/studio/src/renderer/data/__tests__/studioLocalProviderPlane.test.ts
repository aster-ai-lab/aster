import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { DataRequest } from '@shared/data/api/types'

import { answerStudioProviderRequest, resetStudioProviderPlaneForTests } from '../studioProviderPlane'

const runtime = { apiBaseUrl: 'http://127.0.0.1:9', token: 'a'.repeat(64) }
const local = { id: 'openai', name: 'OpenAI', baseUrl: 'http://127.0.0.1:8000/v1', enabled: true, hasApiKey: true }
const response = (body: unknown) => new Response(JSON.stringify(body), { status: 200 })
const request = (method: DataRequest['method'], path: string, extra: Partial<DataRequest> = {}): DataRequest => ({ id: 'local', method, path, ...extra })

describe('aster local provider build boundary', () => {
  beforeEach(() => {
    vi.stubEnv('PUBLIC_STUDIO_TENANT', 'aster')
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
  })
  afterEach(() => {
    resetStudioProviderPlaneForTests()
    Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
    vi.unstubAllEnvs()
    vi.unstubAllGlobals()
  })

  it('lists local providers without requesting an organization credential or exposing its raw store', async () => {
    const fetch = vi.fn(async (url: string) => {
      expect(url).toBe(`${runtime.apiBaseUrl}/api/providers`)
      return response({ providers: [local, { ...local, id: 'a3s', name: 'raw managed store' }] })
    })
    vi.stubGlobal('fetch', fetch)
    const result = await answerStudioProviderRequest(request('GET', '/providers'))
    expect(result?.status).toBe(200)
    expect(result?.data).toEqual([expect.objectContaining({ id: 'openai', name: 'OpenAI', isEnabled: true })])
    expect(fetch).toHaveBeenCalledTimes(1)
  })

  it('persists a custom provider through the native local DTO, then renames and deletes it', async () => {
    const saved: unknown[] = []
    vi.stubGlobal('fetch', vi.fn(async (url: string, init?: RequestInit) => {
      expect(url).toBe(`${runtime.apiBaseUrl}/api/providers/custom-fixture`)
      if (init?.method === 'POST') saved.push(JSON.parse(String(init.body)))
      return response({ ...local, id: 'custom-fixture', name: 'Private endpoint' })
    }))
    const created = await answerStudioProviderRequest(request('POST', '/providers', { body: {
      providerId: 'custom-fixture', name: 'Private endpoint',
      endpointConfigs: { 'openai-chat-completions': { adapterFamily: 'openai-compatible', baseUrl: local.baseUrl } },
      apiKeys: [{ id: 'fixture', key: 'fixture-key', isEnabled: true }]
    } }))
    expect(created?.status).toBe(200)
    await answerStudioProviderRequest(request('PATCH', '/providers/custom-fixture', { body: { name: 'Renamed' } }))
    await answerStudioProviderRequest(request('DELETE', '/providers/custom-fixture'))
    expect(saved).toEqual([
      { enabled: true, name: 'Private endpoint', baseUrl: local.baseUrl, apiKey: 'fixture-key' },
      { name: 'Renamed' }, { deleted: true }
    ])
  })

  it('keeps the ToYou build on managed admission even if runtime branding says aster', async () => {
    vi.stubEnv('PUBLIC_STUDIO_TENANT', 'toyou')
    document.documentElement.dataset.tenant = 'aster'
    const fetch = vi.fn()
    vi.stubGlobal('fetch', fetch)
    const rejected = await answerStudioProviderRequest(request('POST', '/providers', { body: { providerId: 'openai' } }))
    expect(rejected?.status).toBe(400)
    expect(fetch).not.toHaveBeenCalled()
    delete document.documentElement.dataset.tenant
  })
})
