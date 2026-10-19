import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { clearConnectorConfig, loadConnectorConfig, readConnectorConfig, saveConnectorConfig } from '../connectorConfig'

const shellToken = 'ab'.repeat(32)

function jsonResponse(payload: unknown, status = 200): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' }
  })
}

describe('connectorConfig', () => {
  let fetchMock: ReturnType<typeof vi.fn>

  beforeEach(() => {
    localStorage.clear()
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:4000', token: shellToken }
    })
    fetchMock = vi.fn(async () => jsonResponse({ ok: true }))
    vi.stubGlobal('fetch', fetchMock)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  })

  it('saves through the shell route and keeps a form-recovery copy', async () => {
    await saveConnectorConfig('a3s-maas', {
      'X-Maas-Username': 'demo',
      'X-Maas-Password': 'p',
      Blank: ''
    })

    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:4000/api/capabilities/config/a3s-maas',
      expect.objectContaining({
        method: 'PUT',
        headers: expect.objectContaining({ authorization: `Bearer ${shellToken}` })
      })
    )
    // Blank values drop; the remembered copy refills the form after a reopen.
    expect(readConnectorConfig('a3s-maas')).toEqual({
      'X-Maas-Username': 'demo',
      'X-Maas-Password': 'p'
    })
  })

  it('throws when the shell rejects the save, still keeping the form copy', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ error: 'unauthorized' }, 401))

    await expect(saveConnectorConfig('a3s-maas', { 'X-Maas-Password': 'p' })).rejects.toThrow('HTTP 401')
    expect(readConnectorConfig('a3s-maas')).toEqual({ 'X-Maas-Password': 'p' })
  })

  it('clears best-effort: an unreachable shell never blocks uninstall', async () => {
    fetchMock.mockRejectedValue(new TypeError('fetch failed'))

    await expect(clearConnectorConfig('a3s-maas')).resolves.toBeUndefined()
    expect(readConnectorConfig('a3s-maas')).toEqual({})
  })

  it('prefers the shell copy when loading and pushes a local-only copy up', async () => {
    localStorage.setItem('a3s.connector.config.v1', JSON.stringify({ slug: { K: 'local' } }))
    fetchMock.mockImplementation((input: RequestInfo | URL) =>
      Promise.resolve(
        String(input).endsWith('/api/capabilities/config')
          ? jsonResponse({ slug: { K: 'remote' } })
          : jsonResponse({ ok: true })
      )
    )

    await expect(loadConnectorConfig('slug')).resolves.toEqual({ K: 'remote' })
    expect(readConnectorConfig('slug')).toEqual({ K: 'remote' })

    // No shell copy: the local copy is returned and pushed up to the device.
    fetchMock.mockImplementation(async () => jsonResponse({}))
    await expect(loadConnectorConfig('slug')).resolves.toEqual({ K: 'remote' })
  })
})
