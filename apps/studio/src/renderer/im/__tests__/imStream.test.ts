import { describe, expect, it, vi } from 'vitest'

import type { ImApi } from '../imApi'
import { backoffDelay, keepStreamOpen, SseParser } from '../imStream'

describe('SseParser', () => {
  it('splits frames across chunk boundaries and skips comments', () => {
    const parser = new SseParser()
    expect(parser.push(': connected\nretry: 5000\n\nid: 7\nevent: im.facts.message.created\nda')).toEqual([])
    expect(parser.push('ta: {"conversation_id":"c"}\n\n')).toEqual([
      { id: '7', event: 'im.facts.message.created', data: '{"conversation_id":"c"}' }
    ])
  })

  it('joins multi-line data and accepts CRLF', () => {
    const parser = new SseParser()
    expect(parser.push('data: a\r\ndata: b\r\n\r\n')).toEqual([{ id: undefined, event: undefined, data: 'a\nb' }])
  })
})

describe('keepStreamOpen', () => {
  it('backs off from 1s to a 30s cap', () => {
    expect([1, 2, 3, 6, 10].map(backoffDelay)).toEqual([1000, 2000, 4000, 30000, 30000])
  })

  it('feeds frames, then reconnects after the stream ends', async () => {
    const encoder = new TextEncoder()
    const stream = vi.fn(
      async () =>
        new Response(
          new ReadableStream({
            start(controller) {
              controller.enqueue(encoder.encode('event: im.facts.cursor.changed\ndata: {"conversation_id":"c"}\n\n'))
              controller.close()
            }
          })
        )
    )
    const timers = { setTimeout: vi.fn(() => 1), clearTimeout: vi.fn() }
    const onOpen = vi.fn()
    const onFrame = vi.fn()
    const onClose = vi.fn()
    const stop = keepStreamOpen({ stream } as unknown as ImApi, 'studio-1', { onOpen, onFrame, onClose }, timers)
    await vi.waitFor(() => expect(onClose).toHaveBeenCalled())
    expect(stream).toHaveBeenCalledWith('studio-1', expect.any(AbortSignal))
    expect(onOpen).toHaveBeenCalledTimes(1)
    expect(onFrame).toHaveBeenCalledWith({ id: undefined, event: 'im.facts.cursor.changed', data: '{"conversation_id":"c"}' })
    expect(timers.setTimeout).toHaveBeenCalledWith(expect.any(Function), 1000)
    stop()
    expect(timers.clearTimeout).toHaveBeenCalled()
  })
})
