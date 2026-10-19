import type { DataRequest, DataResponse } from '@shared/data/api/types'
import type { StudioRuntimeDescriptor } from '@shared/studioRuntime'

export async function postStudioDataRequest(
  runtime: StudioRuntimeDescriptor,
  request: DataRequest,
  timeoutMs = 3000
): Promise<DataResponse> {
  const response = await fetch(`${runtime.apiBaseUrl}/api/data`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${runtime.token}`
    },
    body: JSON.stringify(request),
    signal: AbortSignal.timeout(timeoutMs)
  })
  const payload: unknown = await response.json()
  if (!payload || typeof payload !== 'object' || Array.isArray(payload) || !('status' in payload)) {
    throw new Error('Studio data response is invalid.')
  }
  return payload as DataResponse
}
