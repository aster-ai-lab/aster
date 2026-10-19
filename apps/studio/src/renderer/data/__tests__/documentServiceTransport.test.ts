import { describe, expect, it, vi } from 'vitest'

import { documentServiceFetch } from '../documentServiceTransport'

const runtime = () => ({ apiBaseUrl: 'http://127.0.0.1:9999', token: 'native-test-token' })
const credential = () =>
  new globalThis.Response(JSON.stringify({ origin: 'https://gateway.test', token: 'gateway-test-token' }))

describe('authenticated document service transport', () => {
  it('uses the existing issuer, keeps native credentials off the gateway, and refuses redirects', async () => {
    const calls: { url: string; options?: NonNullable<Parameters<typeof globalThis.fetch>[1]> }[] = []
    const fetchImpl: typeof globalThis.fetch = async (input, options) => {
      calls.push({ url: String(input), options })
      return calls.length === 1 ? credential() : new globalThis.Response('{"text":"recognized"}')
    }
    const transport = documentServiceFetch('http://127.0.0.1:8080', { fetch: fetchImpl, runtime })
    const response = await transport('http://127.0.0.1:8080/v1/ocr', { method: 'POST', body: new Uint8Array([1]) })
    expect(await response.json()).toEqual({ text: 'recognized' })
    expect(calls.map((call) => call.url)).toEqual([
      'http://127.0.0.1:9999/api/os-gateway/credential',
      'https://gateway.test/v1/ocr'
    ])
    expect(new globalThis.Headers(calls[0].options?.headers).get('authorization')).toBe('Bearer native-test-token')
    expect(new globalThis.Headers(calls[1].options?.headers).get('authorization')).toBe('Bearer gateway-test-token')
    expect(calls[1].options?.redirect).toBe('error')
  })

  it('refuses missing/expired native auth, an unrelated origin and unknown service paths', async () => {
    const fetchImpl = vi.fn<typeof globalThis.fetch>(async () => new globalThis.Response(null, { status: 401 }))
    const missing = documentServiceFetch('https://gateway.test', { fetch: fetchImpl, runtime: () => undefined })
    await expect(missing('https://gateway.test/v1/ocr', { method: 'POST' })).rejects.toThrow('sign_in')
    expect(fetchImpl).not.toHaveBeenCalled()
    const transport = documentServiceFetch('https://gateway.test', { fetch: fetchImpl, runtime })
    await expect(transport('https://other.test/v1/ocr', { method: 'POST' })).rejects.toThrow('unavailable')
    await expect(transport('https://gateway.test/v1/chat/completions', { method: 'POST' })).rejects.toThrow(
      'unavailable'
    )
    expect(fetchImpl).not.toHaveBeenCalled()
    await expect(transport('https://gateway.test/v1/ocr', { method: 'POST' })).rejects.toThrow('sign_in')
    expect(fetchImpl).toHaveBeenCalledTimes(1)
  })

  it('fences task cancellation before auth', async () => {
    const controller = new globalThis.AbortController()
    const fetchImpl = vi.fn<typeof globalThis.fetch>(async () => credential())
    const transport = documentServiceFetch('https://gateway.test', { fetch: fetchImpl, runtime })
    controller.abort()
    await expect(
      transport('https://gateway.test/v1/ocr', { method: 'POST', signal: controller.signal })
    ).rejects.toThrow('cancel')
    expect(fetchImpl).not.toHaveBeenCalled()
  })

  it('cancels a pending native credential request without calling the gateway', async () => {
    const controller = new globalThis.AbortController()
    const fetchImpl = vi.fn<typeof globalThis.fetch>(
      async (_input, options) =>
        new Promise<never>((_resolve, reject) => {
          options?.signal?.addEventListener(
            'abort',
            () => reject(new globalThis.DOMException('aborted', 'AbortError')),
            { once: true }
          )
        })
    )
    const transport = documentServiceFetch('https://gateway.test', { fetch: fetchImpl, runtime })
    const pending = transport('https://gateway.test/v1/ocr', { method: 'POST', signal: controller.signal })
    controller.abort()
    await expect(pending).rejects.toThrow('cancel')
    expect(fetchImpl).toHaveBeenCalledTimes(1)
    expect(fetchImpl.mock.calls[0][1]?.signal?.aborted).toBe(true)
  })

  it('cancels streaming consumption after headers without exposing an unbounded or late result', async () => {
    const controller = new globalThis.AbortController()
    let index = 0
    const fetchImpl: typeof globalThis.fetch = async (_input, options) =>
      ++index === 1
        ? credential()
        : new globalThis.Response(
            new globalThis.ReadableStream({
              start(stream) {
                stream.enqueue(new globalThis.TextEncoder().encode('data: first\n\n'))
                options?.signal?.addEventListener(
                  'abort',
                  () => stream.error(new globalThis.DOMException('aborted', 'AbortError')),
                  { once: true }
                )
              }
            })
          )
    const transport = documentServiceFetch('https://gateway.test', { fetch: fetchImpl, runtime })
    const response = await transport('https://gateway.test/v1/audio/transcriptions', {
      method: 'POST',
      signal: controller.signal
    })
    const consuming = response.text()
    controller.abort()
    await expect(consuming).rejects.toThrow('cancel')
  })

  it('preserves streaming transcript chunks and closes response deadlines on consumption', async () => {
    let index = 0
    const fetchImpl: typeof globalThis.fetch = async () => {
      index++
      return index === 1
        ? credential()
        : new globalThis.Response(
            new globalThis.ReadableStream({
              start(controller) {
                controller.enqueue(new globalThis.TextEncoder().encode('data: first\n\n'))
                controller.enqueue(new globalThis.TextEncoder().encode('data: second\n\n'))
                controller.close()
              }
            }),
            { headers: { 'content-type': 'text/event-stream' } }
          )
    }
    const transport = documentServiceFetch('https://gateway.test', { fetch: fetchImpl, runtime })
    const response = await transport('https://gateway.test/v1/audio/transcriptions', { method: 'POST' })
    expect(response.headers.get('content-type')).toBe('text/event-stream')
    expect(await response.text()).toBe('data: first\n\ndata: second\n\n')
  })

  it('cancels an oversized OCR response before presenting it as a valid result', async () => {
    let index = 0
    const cancelled = vi.fn()
    const fetchImpl: typeof globalThis.fetch = async () =>
      ++index === 1
        ? credential()
        : new globalThis.Response(
            new globalThis.ReadableStream({
              start(controller) {
                controller.enqueue(new Uint8Array(8 * 1024 * 1024 + 1))
              },
              cancel: cancelled
            })
          )
    const transport = documentServiceFetch('https://gateway.test', { fetch: fetchImpl, runtime })
    const response = await transport('https://gateway.test/v1/ocr', { method: 'POST' })
    await expect(response.text()).rejects.toThrow('too_large')
    expect(cancelled).toHaveBeenCalledTimes(1)
  })
})
