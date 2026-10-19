import { IM_ATTACHMENT_MAX_BYTES } from '@renderer/im/imApi'
import { ImActionError } from '@renderer/im/imStore'
import { readStudioRuntime } from '@shared/studioRuntime'

/** One file the ToBot turn prompt can open by absolute path. */
export type TobotFilePart = {
  type: 'file'
  url: string
  filename: string
  mediaType: string
  sizeBytes: number
}

const previews = new Map<string, string>()

export function rememberPreview(path: string, url: string) {
  if (path && url) previews.set(path, url)
}

export function previewFor(path: string): string | undefined {
  return previews.get(path)
}

/** `file://` plus the absolute path, with the same byte encoding the agent plane decodes. */
export function fileUrl(path: string): string {
  const bytes = new TextEncoder().encode(path)
  let url = 'file://'
  for (const byte of bytes) {
    const plain =
      (byte >= 0x41 && byte <= 0x5a) ||
      (byte >= 0x61 && byte <= 0x7a) ||
      (byte >= 0x30 && byte <= 0x39) ||
      byte === 0x2f ||
      byte === 0x2d ||
      byte === 0x5f ||
      byte === 0x2e ||
      byte === 0x7e
    url += plain ? String.fromCharCode(byte) : `%${byte.toString(16).toUpperCase().padStart(2, '0')}`
  }
  return url
}

export function pathFromFileUrl(url: string): string {
  if (!url.startsWith('file://')) return ''
  try {
    return decodeURIComponent(url.slice('file://'.length))
  } catch {
    return ''
  }
}

function shell(): { base: string; token: string } {
  let runtime: { apiBaseUrl: string; token: string } | undefined
  try {
    runtime = readStudioRuntime()
  } catch {
    runtime = undefined
  }
  if (!runtime) throw new ImActionError('request', 'runtime')
  return { base: runtime.apiBaseUrl.replace(/\/$/, ''), token: runtime.token }
}

function parentPath(path: string): string {
  const slash = path.lastIndexOf('/')
  if (slash <= 0) throw new ImActionError('request', 'path')
  return path.slice(0, slash)
}

/** Writes the bytes where the running agent can read them, and returns the turn part. */
export async function uploadTobotFile(file: File): Promise<TobotFilePart> {
  if (file.size <= 0) throw new ImActionError('empty')
  if (file.size > IM_ATTACHMENT_MAX_BYTES) throw new ImActionError('too_large')
  const { base, token } = shell()
  const name = file.name.trim() || 'file'
  let response: Response
  try {
    response = await fetch(`${base}/api/composer-files/${encodeURIComponent(name)}`, {
      method: 'POST',
      headers: {
        authorization: `Bearer ${token}`,
        'content-type': 'application/octet-stream'
      },
      body: file
    })
  } catch (error) {
    throw new ImActionError('request', error instanceof Error ? error.message : 'upload')
  }
  if (response.status === 413) throw new ImActionError('too_large')
  if (!response.ok) throw new ImActionError('request', `upload ${response.status}`)
  const body = (await response.json().catch(() => null)) as { path?: unknown; name?: unknown } | null
  const path = body && typeof body.path === 'string' ? body.path : ''
  if (!path.startsWith('/') || path.split('/').includes('..')) throw new ImActionError('request', 'upload')
  const filename = body && typeof body.name === 'string' && body.name ? body.name : name
  return {
    type: 'file',
    url: fileUrl(path),
    filename,
    mediaType: file.type || 'application/octet-stream',
    sizeBytes: file.size
  }
}

export async function readAgentFile(path: string): Promise<Blob> {
  const { base, token } = shell()
  const root = parentPath(path)
  const response = await fetch(
    `${base}/api/workspace-file/raw?root=${encodeURIComponent(root)}&path=${encodeURIComponent(path)}`,
    { headers: { authorization: `Bearer ${token}` } }
  )
  if (!response.ok) throw new ImActionError('request', `read ${response.status}`)
  return response.blob()
}

export async function openAgentFile(path: string): Promise<void> {
  const { base, token } = shell()
  const response = await fetch(`${base}/api/workspace-file/open`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${token}`,
      'content-type': 'application/json'
    },
    body: JSON.stringify({ root: parentPath(path), path })
  })
  if (!response.ok) throw new ImActionError('request', `open ${response.status}`)
}
