import { readStudioRuntime } from '@shared/studioRuntime'
import type { StudioRuntimeDescriptor } from '@shared/studioRuntime'

type FetchOptions = NonNullable<Parameters<typeof globalThis.fetch>[1]>
const ENDPOINT_LIMITS: Readonly<Record<string, number>> = {
  '/v1/ocr': 8 * 1024 * 1024,
  '/v1/audio/transcriptions': 8 * 1024 * 1024,
  '/v1/images/edits': 50 * 1024 * 1024
}

const KEY = 'workbench.feature.pdf.tools.workspace.service.'

/** Project only known contextual failures into presentation; never display upstream error text. */
export function documentServiceFailureKey(error: unknown): string | undefined {
  const message = error instanceof Error ? error.message : ''
  return [KEY + 'sign_in', KEY + 'unavailable', KEY + 'too_large', 'workbench.feature.pdf.tools.job.cancel'].includes(
    message
  )
    ? message
    : undefined
}

/** Delegate to the existing native credential issuer; never retain credentials in task data. */
export function documentServiceFetch(
  origin: string,
  dependencies: {
    fetch?: typeof globalThis.fetch
    runtime?: () => StudioRuntimeDescriptor | undefined
    timeoutMs?: number
  } = {}
): typeof globalThis.fetch {
  const fetchImpl = dependencies.fetch ?? globalThis.fetch
  const configured = new globalThis.URL(origin)
  if (!['http:', 'https:'].includes(configured.protocol) || configured.username || configured.password) {
    throw new Error(KEY + 'unavailable')
  }
  return async (input, options) => {
    const raw = typeof input === 'string' ? input : input instanceof globalThis.URL ? input.href : input.url
    const requested = new globalThis.URL(raw)
    const limit = ENDPOINT_LIMITS[requested.pathname]
    if (
      requested.origin !== configured.origin ||
      !limit ||
      requested.search ||
      requested.hash ||
      options?.method !== 'POST'
    ) {
      throw new Error(KEY + 'unavailable')
    }
    const runtime = (dependencies.runtime ?? readStudioRuntime)()
    if (!runtime) throw new Error(KEY + 'sign_in')
    if (options.signal?.aborted) throw new Error('workbench.feature.pdf.tools.job.cancel')
    const scope = requestScope(options, dependencies.timeoutMs ?? 120_000)
    try {
      const response = await fetchImpl(`${runtime.apiBaseUrl}/api/os-gateway/credential`, {
        headers: { authorization: `Bearer ${runtime.token}` },
        signal: scope.signal,
        redirect: 'error'
      })
      if (!response.ok)
        throw new Error(KEY + (response.status === 401 || response.status === 403 ? 'sign_in' : 'unavailable'))
      const text = await boundedResponse(response, 64 * 1024, () => undefined).text()
      const credential: unknown = JSON.parse(text)
      if (!credential || typeof credential !== 'object' || Array.isArray(credential))
        throw new Error(KEY + 'unavailable')
      const body = credential as Record<string, unknown>
      if (
        typeof body.origin !== 'string' ||
        typeof body.token !== 'string' ||
        !body.token.trim() ||
        body.token.length > 16_384 ||
        /[\r\n]/.test(body.token)
      )
        throw new Error(KEY + 'unavailable')
      const target = new globalThis.URL(body.origin)
      if (
        !['http:', 'https:'].includes(target.protocol) ||
        target.username ||
        target.password ||
        target.pathname !== '/' ||
        target.search ||
        target.hash
      )
        throw new Error(KEY + 'unavailable')
      const headers = new globalThis.Headers(options.headers)
      headers.set('authorization', `Bearer ${body.token}`)
      if (scope.signal.aborted)
        throw new Error(options.signal?.aborted ? 'workbench.feature.pdf.tools.job.cancel' : KEY + 'unavailable')
      const result = await fetchImpl(`${target.origin}${requested.pathname}`, {
        ...options,
        headers,
        signal: scope.signal,
        redirect: 'error'
      })
      if (!result.ok) {
        await result.body?.cancel()
        throw new Error(KEY + (result.status === 401 || result.status === 403 ? 'sign_in' : 'unavailable'))
      }
      return boundedResponse(result, limit, scope.close, options.signal)
    } catch (error) {
      scope.close()
      if (options.signal?.aborted) throw new Error('workbench.feature.pdf.tools.job.cancel')
      if (error instanceof Error && error.message.startsWith(KEY)) throw error
      throw new Error(KEY + 'unavailable')
    }
  }
}

function requestScope(options: FetchOptions, timeoutMs: number) {
  const controller = new globalThis.AbortController()
  const onAbort = () => controller.abort()
  options.signal?.addEventListener('abort', onAbort, { once: true })
  const timer = globalThis.setTimeout(onAbort, timeoutMs)
  return {
    signal: controller.signal,
    close: () => {
      globalThis.clearTimeout(timer)
      options.signal?.removeEventListener('abort', onAbort)
    }
  }
}

/** Preserve streaming progress while bounding every response and cancelling excess bytes. */
function boundedResponse(
  response: InstanceType<typeof globalThis.Response>,
  limit: number,
  close: () => void,
  signal?: FetchOptions['signal']
) {
  if (!response.body) {
    close()
    return response
  }
  const reader = response.body.getReader()
  let size = 0
  const stream = new globalThis.ReadableStream<Uint8Array>({
    async pull(controller) {
      try {
        const part = await reader.read()
        if (part.done) {
          controller.close()
          close()
          return
        }
        size += part.value.byteLength
        if (size > limit) {
          await reader.cancel()
          throw new Error(KEY + 'too_large')
        }
        controller.enqueue(part.value)
      } catch (error) {
        controller.error(
          new Error(
            signal?.aborted
              ? 'workbench.feature.pdf.tools.job.cancel'
              : (documentServiceFailureKey(error) ?? KEY + 'unavailable')
          )
        )
        close()
      }
    },
    async cancel(reason) {
      try {
        await reader.cancel(reason)
      } finally {
        close()
      }
    }
  })
  return new globalThis.Response(stream, {
    status: response.status,
    statusText: response.statusText,
    headers: response.headers
  })
}
