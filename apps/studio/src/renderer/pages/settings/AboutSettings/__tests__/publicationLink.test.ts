import { afterEach, describe, expect, it, vi } from 'vitest'

import { publishedNoticeUrl } from '../publicationLink'

const runtime = { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) }

afterEach(() => {
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

describe('publishedNoticeUrl', () => {
  it('returns an absolute http(s) address from the shell envelope', async () => {
    const fetchImpl = vi.fn(async () =>
      response({ code: 0, data: { url: 'https://docs.example/v1/api/legal/privacy' } })
    )
    await expect(publishedNoticeUrl('privacy', { fetchImpl, runtime })).resolves.toBe(
      'https://docs.example/v1/api/legal/privacy'
    )
    expect(fetchImpl).toHaveBeenCalledWith('http://127.0.0.1:9/api/publications/privacy', {
      headers: { authorization: `Bearer ${runtime.token}` }
    })
  })

  it('accepts a bare document and rejects a relative or script address', async () => {
    await expect(
      publishedNoticeUrl('terms', {
        fetchImpl: async () => response({ url: 'http://127.0.0.1:3000/v1/api/legal/terms' }),
        runtime
      })
    ).resolves.toBe('http://127.0.0.1:3000/v1/api/legal/terms')
    await expect(
      publishedNoticeUrl('terms', {
        fetchImpl: async () => response({ data: { url: '/v1/api/legal/terms' } }),
        runtime
      })
    ).resolves.toBeNull()
    await expect(
      publishedNoticeUrl('support', {
        fetchImpl: async () => response({ data: { url: 'javascript:alert(1)' } }),
        runtime
      })
    ).resolves.toBeNull()
  })

  it('stays closed when the shell, the network, or the document is missing', async () => {
    await expect(
      publishedNoticeUrl('copyright', { runtime: null, fetchImpl: async () => response({}) })
    ).resolves.toBeNull()
    await expect(
      publishedNoticeUrl('copyright', {
        runtime,
        fetchImpl: async () => ({ ok: false, json: async () => ({}) }) as Response
      })
    ).resolves.toBeNull()
    await expect(
      publishedNoticeUrl('copyright', {
        runtime,
        fetchImpl: async () => {
          throw new Error('offline')
        }
      })
    ).resolves.toBeNull()
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://example.com', token: 'nope' }
    })
    await expect(publishedNoticeUrl('terms')).resolves.toBeNull()
  })
})

function response(body: unknown): Response {
  return { ok: true, json: async () => body } as Response
}
