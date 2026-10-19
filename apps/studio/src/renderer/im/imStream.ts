/**
 * Server-sent events over a fetch body. EventSource cannot send the loopback bearer,
 * so the stream is read and split into frames here.
 */
import type { ImApi } from './imApi'

export type SseFrame = { id?: string; event?: string; data: string }

/** Incremental `text/event-stream` parser; feed decoded chunks, collect complete frames. */
export class SseParser {
  private buffer = ''

  push(chunk: string): SseFrame[] {
    this.buffer += chunk.replace(/\r\n?/g, '\n')
    const frames: SseFrame[] = []
    let boundary = this.buffer.indexOf('\n\n')
    while (boundary >= 0) {
      const block = this.buffer.slice(0, boundary)
      this.buffer = this.buffer.slice(boundary + 2)
      const frame = parseBlock(block)
      if (frame) frames.push(frame)
      boundary = this.buffer.indexOf('\n\n')
    }
    return frames
  }
}

function parseBlock(block: string): SseFrame | null {
  const data: string[] = []
  let id: string | undefined
  let event: string | undefined
  for (const line of block.split('\n')) {
    if (!line || line.startsWith(':')) continue
    const colon = line.indexOf(':')
    const field = colon < 0 ? line : line.slice(0, colon)
    const value = colon < 0 ? '' : line.slice(colon + 1).replace(/^ /, '')
    if (field === 'data') data.push(value)
    else if (field === 'event') event = value
    else if (field === 'id') id = value
  }
  if (data.length === 0) return null
  return { id, event, data: data.join('\n') }
}

/** Reconnect delay: 1s, doubling, capped at 30s. */
export function backoffDelay(attempt: number): number {
  return Math.min(1000 * 2 ** Math.max(0, attempt - 1), 30_000)
}

export type StreamHandlers = {
  onOpen: () => void
  onFrame: (frame: SseFrame) => void
  onClose: () => void
}

export type StreamTimers = {
  setTimeout: (callback: () => void, ms: number) => unknown
  clearTimeout: (handle: unknown) => void
}

const DEFAULT_TIMERS: StreamTimers = {
  setTimeout: (callback, ms) => globalThis.setTimeout(callback, ms),
  clearTimeout: (handle) => globalThis.clearTimeout(handle as ReturnType<typeof setTimeout>)
}

/** Keeps one stream open, reconnecting with backoff. Returns a stop function. */
export function keepStreamOpen(api: ImApi, deviceId: string, handlers: StreamHandlers, timers = DEFAULT_TIMERS): () => void {
  let stopped = false
  let attempt = 0
  let controller: AbortController | null = null
  let timer: unknown = null

  const schedule = () => {
    if (stopped) return
    attempt += 1
    timer = timers.setTimeout(() => void connect(), backoffDelay(attempt))
  }

  const connect = async () => {
    if (stopped) return
    controller = new AbortController()
    let opened = false
    try {
      const response = await api.stream(deviceId, controller.signal)
      const reader = response.body!.getReader()
      const decoder = new TextDecoder()
      const parser = new SseParser()
      opened = true
      attempt = 0
      handlers.onOpen()
      for (;;) {
        const { done, value } = await reader.read()
        if (done || stopped) break
        for (const frame of parser.push(decoder.decode(value, { stream: true }))) handlers.onFrame(frame)
      }
    } catch {
      // Network drop, abort or rejected ticket: fall through to reconnect.
    }
    if (opened) handlers.onClose()
    schedule()
  }

  void connect()
  return () => {
    stopped = true
    controller?.abort()
    if (timer !== null) timers.clearTimeout(timer)
  }
}
