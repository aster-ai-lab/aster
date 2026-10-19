import { afterEach, describe, expect, it, vi } from 'vitest'

import type { Model } from '@shared/data/types/model'
import type { ApiKeyEntry } from '@shared/data/types/provider'
import type { Provider } from '@shared/data/types/provider'

import { answerStudioProviderRequest, resetStudioProviderPlaneForTests } from '../studioProviderPlane'

const token = 'a'.repeat(64)
const runtime = { apiBaseUrl: 'http://127.0.0.1:9', token }
const session = 'a3s_ms_member.session.token'

function request(
  method: 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE',
  path: string,
  extra: Record<string, unknown> = {}
) {
  return { id: 'req-1', method, path, ...extra }
}

function jsonResponse(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}

describe('studioProviderPlane', () => {
  afterEach(() => {
    resetStudioProviderPlaneForTests()
    Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
    delete document.documentElement.dataset.a3sModelService
    vi.unstubAllGlobals()
  })

  it('leaves provider routes to the Electron bridge when no studio runtime is present', async () => {
    await expect(answerStudioProviderRequest(request('GET', '/providers'))).resolves.toBeNull()
  })

  it('lists only the signed-in gateway and keeps the member session out of the provider store', async () => {
    document.documentElement.dataset.a3sModelService = 'ToGateway'
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const calls: string[] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init?: RequestInit) => {
        calls.push(`${init?.method ?? 'GET'} ${url}`)
        if (String(url).endsWith('/api/os-gateway/credential')) {
          return jsonResponse({ origin: 'http://127.0.0.1:8080', token: session })
        }
        if (String(url).endsWith('/v1/api/org/models')) {
          return jsonResponse({
            success: true,
            data: {
              items: [
                {
                  id: 'deepseek-v4-flash',
                  name: 'DeepSeek V4 Flash',
                  brand: 'DeepSeek',
                  catalogKey: 'deepseek/deepseek-chat',
                  capabilities: ['tool_call'],
                  protocols: ['openai', 'anthropic'],
                  available: true
                }
              ],
              autoModel: 'deepseek-v4-flash'
            }
          })
        }
        return jsonResponse({}, 404)
      })
    )

    const listed = await answerStudioProviderRequest(request('GET', '/providers'))
    const providers = listed?.data as Provider[]
    expect(providers).toHaveLength(1)
    expect(providers[0]).toMatchObject({
      id: 'a3s',
      name: 'ToGateway',
      isEnabled: true,
      defaultChatEndpoint: 'openai-chat-completions'
    })
    expect(providers[0]?.endpointConfigs?.['openai-chat-completions']?.baseUrl).toBe('http://127.0.0.1:8080/v1')
    expect(providers[0]?.endpointConfigs?.['anthropic-messages']).toMatchObject({
      adapterFamily: 'anthropic',
      baseUrl: 'http://127.0.0.1:8080/v1'
    })
    expect(providers[0]?.endpointConfigs?.['google-generate-content']?.baseUrl).toBe('http://127.0.0.1:8080/v1beta')
    expect(JSON.stringify(providers[0])).not.toContain(session)
    expect(JSON.stringify(providers[0])).not.toContain('open.a3s.net')

    const keys = await answerStudioProviderRequest(request('GET', '/providers/a3s/api-keys'))
    expect(keys?.data).toEqual({ keys: [{ id: 'a3s:session', key: session, isEnabled: true }] })

    const rejected = await answerStudioProviderRequest(
      request('PUT', '/providers/a3s/api-keys', { body: { keys: [{ id: 'user', key: 'sk-user', isEnabled: true }] } })
    )
    expect(rejected?.status).toBe(400)
    const still = await answerStudioProviderRequest(request('GET', '/providers/a3s/api-keys'))
    expect((still?.data as { keys: ApiKeyEntry[] }).keys[0]?.key).toBe(session)

    const created = await answerStudioProviderRequest(
      request('POST', '/providers', { body: { providerId: 'openai', name: 'OpenAI' } })
    )
    expect(created?.status).toBe(400)

    const models = await answerStudioProviderRequest(request('GET', '/models', { params: { providerId: 'a3s' } }))
    expect(models?.data).toEqual([
      expect.objectContaining({
        id: 'a3s::deepseek-v4-flash',
        providerId: 'a3s',
        apiModelId: 'deepseek-v4-flash',
        name: 'DeepSeek V4 Flash',
        group: 'DeepSeek',
        isEnabled: true,
        endpointTypes: expect.arrayContaining(['openai-chat-completions', 'anthropic-messages'])
      })
    ])
    expect(calls.some((call) => call.endsWith('/v1/api/org/models'))).toBe(true)
    expect(calls.some((call) => call.includes('/api/org-models'))).toBe(false)
    expect(calls.some((call) => call.includes('/api/providers'))).toBe(false)
    expect(calls.some((call) => call.includes('tenant-model-market'))).toBe(false)
    expect(calls.some((call) => call.includes('/api/model-service/models'))).toBe(false)

    const auto = await answerStudioProviderRequest(request('GET', '/models/a3s::auto'))
    expect(auto?.data).toMatchObject({
      id: 'a3s::auto',
      providerId: 'a3s',
      apiModelId: 'auto',
      name: 'auto',
      isEnabled: true
    })
  })

  it('does not enable a second provider from a saved host or key', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => jsonResponse({ origin: 'http://127.0.0.1:8080', token: session }))
    )
    const patched = await answerStudioProviderRequest(
      request('PATCH', '/providers/openai', { body: { isEnabled: true, apiKey: 'sk-test-value' } })
    )
    expect(patched?.status).toBe(400)
    const openai = await answerStudioProviderRequest(request('GET', '/models', { params: { providerId: 'openai' } }))
    expect(openai?.status).toBe(400)
    expect((openai?.data as Model[] | undefined) ?? []).toEqual([])
  })
})
