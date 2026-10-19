/**
 * Frozen descriptor the Tauri shell injects into the renderer.
 * The loopback file may also carry `rendererUrl` for the shell; the page only
 * reads the API base and token.
 */
export interface StudioRuntimeDescriptor {
  apiBaseUrl: string
  token: string
}

export interface StudioShellLaunch extends StudioRuntimeDescriptor {
  rendererUrl: string
}

const LOOPBACK_ORIGIN = /^http:\/\/127\.0\.0\.1:\d{1,5}$/
const LOOPBACK_PAGE = /^http:\/\/127\.0\.0\.1:\d{1,5}\/windows\/main\/index\.html$/
const TOKEN = /^[a-f0-9]{64}$/

export function parseStudioRuntime(value: unknown): StudioRuntimeDescriptor | undefined {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return undefined
  const record = value as Record<string, unknown>
  if (typeof record.apiBaseUrl !== 'string' || !LOOPBACK_ORIGIN.test(record.apiBaseUrl)) return undefined
  if (typeof record.token !== 'string' || !TOKEN.test(record.token)) return undefined
  return { apiBaseUrl: record.apiBaseUrl, token: record.token }
}

export function parseStudioShellLaunch(value: unknown): StudioShellLaunch | undefined {
  const runtime = parseStudioRuntime(value)
  if (!runtime || !value || typeof value !== 'object' || Array.isArray(value)) return undefined
  const rendererUrl = (value as Record<string, unknown>).rendererUrl
  if (typeof rendererUrl !== 'string' || !LOOPBACK_PAGE.test(rendererUrl)) return undefined
  return { ...runtime, rendererUrl }
}

/** Home directory from the shell boot JSON. Empty outside the Tauri shell. */
export function readStudioHomePath(host: typeof globalThis = globalThis): string {
  const value = (host as typeof host & { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  if (!value || typeof value !== 'object' || Array.isArray(value)) return ''
  const homePath = (value as Record<string, unknown>).homePath
  return typeof homePath === 'string' ? homePath : ''
}

/** Absent descriptor means the Electron preload bridge. A present but invalid one fails closed. */
export function readStudioRuntime(host: typeof globalThis = globalThis): StudioRuntimeDescriptor | undefined {
  const value = (host as typeof host & { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  if (value === undefined) return undefined
  const runtime = parseStudioRuntime(value)
  if (!runtime) {
    throw new Error('The studio runtime descriptor is invalid.')
  }
  return runtime
}
