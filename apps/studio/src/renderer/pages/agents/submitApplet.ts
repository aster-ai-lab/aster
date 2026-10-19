import { readAppletRuntime } from '@shared/appletRuntime'

export async function submitApplet(
  appId: string,
  input: {
    version: string
    digest: string
    idempotencyKey: string
    values: Record<string, string>
  },
  fetchImpl: typeof fetch = globalThis.fetch
): Promise<boolean> {
  const runtime = readAppletRuntime()
  if (!runtime || runtime.appId !== appId) return false
  try {
    const response = await fetchImpl(`${runtime.apiBaseUrl}/api/applet-runtime/${appId}/submissions`, {
      method: 'POST',
      cache: 'no-store',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
        authorization: `Bearer ${runtime.appletTicket}`
      },
      body: JSON.stringify({
        version: input.version,
        digest: input.digest,
        idempotencyKey: input.idempotencyKey,
        values: input.values
      })
    })
    return response.ok
  } catch {
    return false
  }
}
