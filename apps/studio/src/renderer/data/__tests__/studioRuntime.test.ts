import { afterEach, describe, expect, it, vi } from 'vitest'

import { parseStudioRuntime, readStudioHomePath, readStudioRuntime } from '@shared/studioRuntime'

import { postStudioDataRequest } from '../studioTransport'

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('studio runtime', () => {
  it('reads a frozen loopback descriptor and ignores an absent one', () => {
    const host = {} as typeof globalThis
    expect(readStudioRuntime(host)).toBeUndefined()
    Object.defineProperty(host, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) }
    })
    expect(readStudioRuntime(host)).toEqual({ apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) })
    expect(parseStudioRuntime({ apiBaseUrl: 'http://example.com', token: 'cd'.repeat(32) })).toBeUndefined()
    expect(readStudioHomePath(host)).toBe('')
    Object.defineProperty(host, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32), homePath: '/Users/roylin' }
    })
    expect(readStudioHomePath(host)).toBe('/Users/roylin')
    expect(readStudioRuntime(host)).toEqual({ apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) })
  })

  it('posts the data request with the bearer token', async () => {
    const fetchMock = viFetch()
    const response = await postStudioDataRequest(
      { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) },
      { id: 'req-1', method: 'GET', path: '/topics' }
    )
    expect(response).toMatchObject({ id: 'req-1', status: 200 })
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:9/api/data',
      expect.objectContaining({
        method: 'POST',
        headers: expect.objectContaining({ authorization: `Bearer ${'cd'.repeat(32)}` })
      })
    )
  })
})

function viFetch() {
  const fetchMock = vi.fn(async () => ({
    json: async () => ({ id: 'req-1', status: 200, data: [] })
  }))
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

