import { afterEach, expect, it, vi } from 'vitest'

import { invokeAixResource } from '../aixResourceTransport'

afterEach(() => vi.unstubAllGlobals())

it('persists draft bytes through one fixed native route and validates its receipt', async () => {
  vi.stubGlobal('__A3S_STUDIO_RUNTIME__', { apiBaseUrl: 'http://127.0.0.1:8123', token: 'b'.repeat(64) })
  const content = '{"scene":"scene ACL","saveIntent":"original operation"}'
  const operationId = 'e61c4641-4d3a-4e83-9cb2-e0df731d1210'
  const fetch = vi
    .fn<typeof globalThis.fetch>()
    .mockResolvedValue(new Response(JSON.stringify({ revision: 3, content })))
  vi.stubGlobal('fetch', fetch)
  const receipt = await invokeAixResource(
    'a'.repeat(64),
    {
      operation: 'save-draft',
      key: 'session',
      expectedRevision: 2,
      operationId,
      content
    },
    new AbortController().signal
  )
  expect(receipt).toEqual({ revision: 3, content })
  const [url, input] = fetch.mock.calls[0]
  expect(url).toBe(`http://127.0.0.1:8123/api/aix/views/sessions/${'a'.repeat(64)}/resources/drafts/session`)
  expect(input?.method).toBe('PUT')
  expect(JSON.parse(input?.body as string)).toEqual({ expectedRevision: 2, operationId, content })
})

it('keeps uncertain draft writes distinct from conflicts and corrupt reads', async () => {
  vi.stubGlobal('__A3S_STUDIO_RUNTIME__', { apiBaseUrl: 'http://127.0.0.1:8123', token: 'b'.repeat(64) })
  const fetch = vi.fn<typeof globalThis.fetch>()
  vi.stubGlobal('fetch', fetch)
  const input = {
    operation: 'save-draft' as const,
    key: 'session',
    expectedRevision: 0,
    operationId: 'e61c4641-4d3a-4e83-9cb2-e0df731d1210',
    content: 'safe input'
  }
  const signal = new AbortController().signal
  fetch.mockResolvedValueOnce(new Response('', { status: 409 }))
  await expect(invokeAixResource('a'.repeat(64), input, signal)).rejects.toMatchObject({ reason: 'conflict' })
  fetch.mockRejectedValueOnce(new TypeError('disconnected'))
  await expect(invokeAixResource('a'.repeat(64), input, signal)).rejects.toMatchObject({ reason: 'outcome_unknown' })
  fetch.mockResolvedValueOnce(new Response(JSON.stringify({ revision: -1, content: 'bad read' })))
  await expect(
    invokeAixResource('a'.repeat(64), { operation: 'read-draft', key: 'session' }, signal)
  ).rejects.toMatchObject({ reason: 'unavailable' })
})
