import { useCallback, useEffect, useState } from 'react'

import {
  formatForName,
  iconForName,
  kindForName,
  type LibraryDocument
} from '@renderer/pages/documents/documentLibrary'
import { compileWorkspaceOkf } from '@renderer/pages/knowledge/compileLocalOkf'
import { knowledgeCorpusAllowed } from '@renderer/pages/settings/GeneralSettings'
import { readStudioHomePath } from '@shared/studioRuntime'
import { readStudioRuntime } from '@shared/studioRuntime'

import {
  CloudWorkspaceError,
  createCloudEntry,
  importCloudFile,
  isCloudWorkspace,
  listCloudEntries,
  moveCloudEntry,
  openCloudWorkspaceFile,
  readCloudDocumentBytes,
  removeCloudEntry,
  renameCloudEntry,
  searchCloudEntries,
  subscribeCloudDocumentChanges,
  writeCloudDocumentBytes
} from './cloudWorkspace'

export interface LocalDiskFile {
  name: string
  path: string
  sizeBytes: number
  modifiedMs: number
  folder: string
}

export function localDocumentFromFile(file: LocalDiskFile): LibraryDocument {
  return {
    id: file.path,
    name: file.name,
    owner: '',
    place: 'local',
    folder: file.folder,
    kind: kindForName(file.name),
    icon: iconForName(file.name),
    format: formatForName(file.name),
    source: 'computer_backup',
    device: 'computer',
    activity: 'upload',
    locationPath: file.path,
    viewedAt: file.modifiedMs || Date.now(),
    updatedAt: file.modifiedMs || undefined,
    sizeBytes: file.sizeBytes,
    favorite: false,
    trashed: false
  }
}

function shellRequest(): { base: string; token: string } | null {
  try {
    const runtime = readStudioRuntime()
    if (!runtime) return null
    return { base: runtime.apiBaseUrl.replace(/\/$/, ''), token: runtime.token }
  } catch {
    return null
  }
}

function folderName(value: unknown): string | null {
  if (!value || typeof value !== 'object') return null
  const name = (value as { name?: unknown }).name
  if (typeof name !== 'string' || !name || name.startsWith('.') || name.includes('/') || name.includes('\\'))
    return null
  return name
}

function fileRow(value: unknown): LocalDiskFile | null {
  if (!value || typeof value !== 'object') return null
  const row = value as { name?: unknown; path?: unknown; sizeBytes?: unknown; modifiedMs?: unknown; folder?: unknown }
  if (typeof row.name !== 'string' || typeof row.path !== 'string') return null
  const folder = typeof row.folder === 'string' ? row.folder : ''
  if (folder.includes('/') || folder.includes('\\') || folder.startsWith('.')) return null
  return {
    name: row.name,
    path: row.path,
    sizeBytes: Number(row.sizeBytes) || 0,
    modifiedMs: Number(row.modifiedMs) || 0,
    folder
  }
}

async function readWorkspace(root: string): Promise<{ folders: string[]; documents: LibraryDocument[] }> {
  const shell = shellRequest()
  if (!shell || !root) return { folders: [], documents: [] }
  const response = await fetch(`${shell.base}/api/local-documents?root=${encodeURIComponent(root)}`, {
    headers: { Authorization: `Bearer ${shell.token}` }
  })
  if (!response.ok) return { folders: [], documents: [] }
  const body = (await response.json()) as { folders?: unknown; files?: unknown }
  const folders = Array.isArray(body.folders) ? body.folders.flatMap((item) => folderName(item) ?? []) : []
  const documents = Array.isArray(body.files)
    ? body.files.flatMap((item) => {
        const file = fileRow(item)
        return file ? [localDocumentFromFile(file)] : []
      })
    : []
  return { folders, documents }
}

async function postPath(action: 'trash' | 'open', root: string, path: string): Promise<void> {
  const shell = shellRequest()
  if (!shell || !root) throw new Error('local documents unavailable')
  const response = await fetch(`${shell.base}/api/local-documents/${action}`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${shell.token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ root, path })
  })
  if (!response.ok) throw new Error(`${action} failed`)
}

export async function trashLocalDocument(root: string, path: string): Promise<void> {
  await postPath('trash', root, path)
}

export async function openLocalDocument(root: string, path: string): Promise<void> {
  await postPath('open', root, path)
}

function workspaceFileUrl(root: string, path: string, snapshot = false): { url: string; token: string } {
  const shell = shellRequest()
  if (!shell || !root || !path) throw new Error('local documents unavailable')
  const url = `${shell.base}/api/workspace-file/${snapshot ? 'snapshot' : 'raw'}?root=${encodeURIComponent(root)}&path=${encodeURIComponent(path)}`
  return { url, token: shell.token }
}

export class FileTooLargeError extends Error {
  readonly bytes: number

  constructor(bytes: number) {
    super('file too large')
    this.name = 'FileTooLargeError'
    this.bytes = bytes
  }
}

export async function readLocalDocumentBytes(
  root: string,
  path: string,
  onProgress?: (ratio: number) => void,
  maxBytes?: number,
  onSnapshot?: (revision: string) => void,
  signal?: AbortSignal
): Promise<Uint8Array> {
  if (isCloudWorkspace(root)) return readCloudDocumentBytes(root, path, onProgress, maxBytes, onSnapshot, signal)
  const target = workspaceFileUrl(root, path, Boolean(onSnapshot))
  const response = await fetch(target.url, {
    headers: { Authorization: `Bearer ${target.token}` },
    ...(signal ? { signal } : {})
  })
  if (!response.ok) throw new Error('read failed')
  let reported = -1
  const report = (ratio: number) => {
    if (!onProgress) return
    const step = ratio >= 1 ? 1 : Math.floor(ratio * 20) / 20
    if (step === reported) return
    reported = step
    onProgress(step)
  }
  const totalHeader = Number(response.headers?.get('content-length'))
  const total = Number.isFinite(totalHeader) && totalHeader > 0 ? totalHeader : 0
  if (maxBytes && total > maxBytes) {
    await response.body?.cancel()
    throw new FileTooLargeError(total)
  }
  const revision = response.headers?.get('etag')?.match(/^"([a-f0-9]{64})"$/u)?.[1]
  if (onSnapshot && !revision) {
    await response.body?.cancel()
    throw new Error('document snapshot unavailable')
  }
  const finish = (bytes: Uint8Array) => {
    if (revision) onSnapshot?.(revision)
    report(1)
    return bytes
  }
  const stream = response.body
  if (!stream || typeof stream.getReader !== 'function' || total === 0) {
    const bytes = new Uint8Array(await response.arrayBuffer())
    if (maxBytes && bytes.byteLength > maxBytes) throw new FileTooLargeError(bytes.byteLength)
    return finish(bytes)
  }
  const reader = stream.getReader()
  const chunks: Uint8Array[] = []
  let received = 0
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    if (!value) continue
    chunks.push(value)
    received += value.byteLength
    if (maxBytes && received > maxBytes) {
      await reader.cancel()
      throw new FileTooLargeError(received)
    }
    report(Math.min(1, received / total))
  }
  const bytes = new Uint8Array(received)
  let offset = 0
  for (const chunk of chunks) {
    bytes.set(chunk, offset)
    offset += chunk.byteLength
  }
  return finish(bytes)
}

async function fileBytes(file: Blob): Promise<Uint8Array> {
  if (typeof file.arrayBuffer === 'function') return new Uint8Array(await file.arrayBuffer())
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer))
    reader.onerror = () => reject(reader.error ?? new Error('read failed'))
    reader.readAsArrayBuffer(file)
  })
}

export async function importLocalFile(
  root: string,
  directory: string,
  file: File,
  signal?: AbortSignal,
  replace = true
): Promise<void> {
  if (isCloudWorkspace(root)) return importCloudFile(root, directory, file, signal, replace)
  const shell = shellRequest()
  if (!shell || !root || !directory) throw new Error('local documents unavailable')
  const params = new URLSearchParams({ root, directory, name: file.name })
  if (!replace) {
    const listing = await listLocalEntries(root, directory, signal)
    if (listing.entries.some((entry) => entry.name === file.name)) throw new LocalEntryError(409)
    params.set('replace', 'false')
  }
  const bytes = await fileBytes(file)
  const response = await fetch(`${shell.base}/api/workspace-file/import?${params}`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${shell.token}`, 'Content-Type': 'application/octet-stream' },
    body: bytes.slice(),
    signal
  })
  if (!response.ok) throw new LocalEntryError(response.status)
}

const MAX_DROPPED_FILES = 40

export interface DroppedFile {
  file: File
  directories: string[]
}

function acceptableFileName(name: string): boolean {
  return (
    Boolean(name) &&
    name.length <= 255 &&
    !name.startsWith('.') &&
    !name.includes('/') &&
    !name.includes('\\') &&
    name.toLowerCase() !== 'wiki'
  )
}

/** Files from an OS drop, including files nested in a dropped folder. */
export async function filesFromDrop(transfer: DataTransfer): Promise<DroppedFile[]> {
  const items = Array.from(transfer.items ?? [])
  const entries = items.flatMap((item) => {
    if (item.kind !== 'file') return []
    const entry = (item as DataTransferItem & { webkitGetAsEntry?: () => FileSystemEntry | null }).webkitGetAsEntry?.()
    return entry ? [entry] : []
  })
  if (entries.length === 0) {
    return Array.from(transfer.files ?? [])
      .filter((file) => acceptableFileName(file.name))
      .slice(0, MAX_DROPPED_FILES)
      .map((file) => ({ file, directories: [] }))
  }
  const found: DroppedFile[] = []
  const readBatch = (reader: FileSystemDirectoryReader) =>
    new Promise<FileSystemEntry[]>((resolve, reject) => {
      const all: FileSystemEntry[] = []
      const read = () => {
        reader.readEntries((batch) => {
          if (batch.length === 0) resolve(all)
          else {
            all.push(...batch)
            read()
          }
        }, reject)
      }
      read()
    })
  const walk = async (entry: FileSystemEntry, directories: string[]) => {
    if (found.length >= MAX_DROPPED_FILES || entry.name.startsWith('.')) return
    if (entry.isFile) {
      const file = await new Promise<File>((resolve, reject) => {
        ;(entry as FileSystemFileEntry).file(resolve, reject)
      })
      if (acceptableFileName(file.name)) found.push({ file, directories })
      return
    }
    if (!entry.isDirectory) return
    const children = await readBatch((entry as FileSystemDirectoryEntry).createReader())
    for (const child of children) await walk(child, [...directories, entry.name])
  }
  for (const entry of entries) await walk(entry, [])
  return found
}

export async function importDroppedFiles(root: string, directory: string, transfer: DataTransfer): Promise<number> {
  const dropped = await filesFromDrop(transfer)
  let count = 0
  for (const item of dropped) {
    let parent = directory
    for (const segment of item.directories) {
      if (!acceptableFileName(segment)) throw new LocalEntryError(400)
      try {
        parent = (await createLocalEntry(root, parent, segment, 'folder')).path
      } catch (error) {
        if (localEntryStatus(error) !== 409) throw error
        parent = `${parent.replace(/[/\\]+$/u, '')}/${segment}`
      }
    }
    await importLocalFile(root, parent, item.file)
    count += 1
  }
  return count
}

export function writeLocalDocumentBytes(root: string, path: string, bytes: Uint8Array): Promise<void>
export function writeLocalDocumentBytes(
  root: string,
  path: string,
  bytes: Uint8Array,
  expectedRevision: string
): Promise<string>
export async function writeLocalDocumentBytes(
  root: string,
  path: string,
  bytes: Uint8Array,
  expectedRevision?: string
): Promise<string | void> {
  if (isCloudWorkspace(root)) return writeCloudDocumentBytes(root, path, bytes, expectedRevision)
  const target = workspaceFileUrl(root, path, expectedRevision !== undefined)
  const response = await fetch(target.url, {
    method: 'PUT',
    headers: {
      Authorization: `Bearer ${target.token}`,
      'Content-Type': 'application/octet-stream',
      ...(expectedRevision === undefined ? {} : { 'If-Match': `"${expectedRevision}"` })
    },
    body: bytes.slice()
  })
  if (!response.ok) throw new Error('write failed')
  const insideWiki = path.split('/').includes('wiki')
  if (!insideWiki && knowledgeCorpusAllowed(root, readStudioHomePath())) {
    void compileWorkspaceOkf(root).catch(() => undefined)
  }
  if (expectedRevision === undefined) return undefined
  const revision = response.headers?.get('etag')?.match(/^"([a-f0-9]{64})"$/u)?.[1]
  if (!revision) throw new Error('document snapshot unavailable')
  return revision
}

export interface LocalEntry {
  name: string
  path: string
  kind: 'file' | 'directory'
  sizeBytes: number
  modifiedMs: number
  /** Times an agent retrieved this file. Absent on folders and search hits. */
  agentRetrievals?: number
  /** Present on team drive files after the server reports parse and compile progress. */
  parseStatus?: 'parsed' | 'stored' | 'unparsed' | 'parsing'
  compileStatus?: 'waiting' | 'queued' | 'running' | 'done' | 'failed' | 'skipped'
  /** Organization drive file id. Present only for team cloud files. */
  driveFileId?: string
}

export interface LocalEntryListing {
  entries: LocalEntry[]
  capped: boolean
}

export interface WorkspaceSearchHit {
  path: string
  relativePath: string
  isDirectory: boolean
}

class LocalEntryError extends Error {
  status: number

  constructor(status: number) {
    super(`local documents ${status}`)
    this.status = status
  }
}

function insideWorkspace(root: string, path: string): boolean {
  const base = root.replace(/[/\\]+$/u, '')
  return path === base || path.startsWith(`${base}/`)
}

function entryRow(root: string, value: unknown): LocalEntry | null {
  if (!value || typeof value !== 'object') return null
  const row = value as {
    name?: unknown
    path?: unknown
    kind?: unknown
    sizeBytes?: unknown
    modifiedMs?: unknown
    agentRetrievals?: unknown
  }
  if (typeof row.name !== 'string' || typeof row.path !== 'string') return null
  if (row.name.startsWith('.') || row.name.includes('/') || row.name.includes('\\')) return null
  if (row.kind !== 'file' && row.kind !== 'directory') return null
  if (!insideWorkspace(root, row.path)) return null
  return {
    name: row.name,
    path: row.path,
    kind: row.kind,
    sizeBytes: Number(row.sizeBytes) || 0,
    modifiedMs: Number(row.modifiedMs) || 0,
    agentRetrievals: row.kind === 'file' ? Number(row.agentRetrievals) || 0 : undefined
  }
}

async function postJson(path: string, body: unknown): Promise<unknown> {
  const shell = shellRequest()
  if (!shell) throw new LocalEntryError(503)
  const response = await fetch(`${shell.base}${path}`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${shell.token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(body)
  })
  if (!response.ok) throw new LocalEntryError(response.status)
  return response.json()
}

export function localEntryStatus(error: unknown): number {
  if (error instanceof LocalEntryError) return error.status
  if (error instanceof CloudWorkspaceError) return error.status
  return 0
}

export interface LocalDocumentChange {
  revision: number
  paths: string[]
  fullScan: boolean
}

function changePaths(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value.flatMap((item) => {
    if (typeof item !== 'string' || item.includes('\\') || item.includes('\0') || item.startsWith('/')) return []
    if (item === '') return ['']
    const parts = item.split('/')
    if (parts.some((part) => !part || part === '.' || part === '..' || part.startsWith('.'))) return []
    return [item]
  })
}

const MAXIMUM_SSE_FRAME_BYTES = 256 * 1024

function splitSseFrame(buffer: string): { frame: string; rest: string } | null {
  const lf = buffer.indexOf('\n\n')
  const crlf = buffer.indexOf('\r\n\r\n')
  if (lf < 0 && crlf < 0) return null
  if (crlf >= 0 && (lf < 0 || crlf < lf)) return { frame: buffer.slice(0, crlf), rest: buffer.slice(crlf + 4) }
  return { frame: buffer.slice(0, lf), rest: buffer.slice(lf + 2) }
}

function parseChangeFrame(frame: string): LocalDocumentChange | null {
  const dataLines: string[] = []
  for (const line of frame.split(/\r?\n/)) {
    if (!line || line.startsWith(':') || line.startsWith('event:') || line.startsWith('id:')) continue
    if (line.startsWith('data:')) dataLines.push(line.slice(5).replace(/^ /u, ''))
  }
  if (dataLines.length === 0) return null
  let body: { revision?: unknown; paths?: unknown; fullScan?: unknown }
  try {
    body = JSON.parse(dataLines.join('\n')) as { revision?: unknown; paths?: unknown; fullScan?: unknown }
  } catch {
    throw new LocalEntryError(500)
  }
  if (typeof body.revision !== 'number' || !Number.isSafeInteger(body.revision) || body.revision < 0) {
    throw new LocalEntryError(500)
  }
  return { revision: body.revision, paths: changePaths(body.paths), fullScan: body.fullScan === true }
}

/**
 * Read one directory-invalidation stream. An empty path is the workspace root.
 * The caller reconnects with the last revision when the stream ends.
 * onReady runs after stream admission, before the first change frame is read.
 */
export async function subscribeLocalDocumentChanges(
  root: string,
  after: number,
  signal: AbortSignal,
  onChange: (change: LocalDocumentChange) => void,
  onReady?: () => void
): Promise<void> {
  if (isCloudWorkspace(root)) return subscribeCloudDocumentChanges(signal)
  const shell = shellRequest()
  if (!shell || !root) throw new LocalEntryError(503)
  const params = `root=${encodeURIComponent(root)}&after=${encodeURIComponent(String(after))}`
  const response = await fetch(`${shell.base}/api/local-documents/changes?${params}`, {
    cache: 'no-store',
    headers: { Accept: 'text/event-stream', Authorization: `Bearer ${shell.token}` },
    signal
  })
  if (!response.ok) throw new LocalEntryError(response.status)
  const contentType = response.headers?.get('content-type') ?? ''
  if (!contentType.includes('text/event-stream') || !response.body) throw new LocalEntryError(500)
  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  try {
    if (!signal.aborted) onReady?.()
    while (!signal.aborted) {
      const { done, value } = await reader.read()
      if (done) return
      buffer += decoder.decode(value, { stream: true })
      if (buffer.length > MAXIMUM_SSE_FRAME_BYTES) throw new LocalEntryError(500)
      for (;;) {
        const split = splitSseFrame(buffer)
        if (!split) break
        buffer = split.rest
        const change = parseChangeFrame(split.frame)
        if (change) onChange(change)
      }
    }
  } finally {
    try {
      await reader.cancel()
    } catch {
      // The stream is already closed.
    }
  }
}

export async function listLocalEntries(root: string, directory = '', signal?: AbortSignal): Promise<LocalEntryListing> {
  if (isCloudWorkspace(root)) return listCloudEntries(root, directory, signal)
  const shell = shellRequest()
  if (!shell || !root) return { entries: [], capped: false }
  const params = `root=${encodeURIComponent(root)}${directory ? `&path=${encodeURIComponent(directory)}` : ''}`
  const response = await fetch(`${shell.base}/api/local-documents/entries?${params}`, {
    cache: 'no-store',
    headers: { Authorization: `Bearer ${shell.token}` },
    signal
  })
  if (!response.ok) throw new LocalEntryError(response.status)
  const body = (await response.json()) as { entries?: unknown; capped?: unknown }
  const entries = Array.isArray(body.entries) ? body.entries.flatMap((item) => entryRow(root, item) ?? []) : []
  return { entries, capped: body.capped === true }
}

export async function createLocalEntry(
  root: string,
  parent: string,
  name: string,
  kind: 'file' | 'folder'
): Promise<LocalEntry> {
  if (isCloudWorkspace(root)) return createCloudEntry(root, parent, name, kind)
  const created = entryRow(root, await postJson('/api/local-documents/entries', { root, parent, name, kind }))
  if (!created) throw new LocalEntryError(500)
  return created
}

export async function renameLocalEntry(root: string, path: string, name: string): Promise<LocalEntry> {
  if (isCloudWorkspace(root)) return renameCloudEntry(root, path, name)
  const renamed = entryRow(root, await postJson('/api/local-documents/rename', { root, path, name }))
  if (!renamed) throw new LocalEntryError(500)
  return renamed
}

export async function moveLocalEntry(root: string, path: string, destination: string): Promise<LocalEntry> {
  if (isCloudWorkspace(root)) return moveCloudEntry(root, path, destination)
  const moved = entryRow(root, await postJson('/api/local-documents/move', { root, path, destination }))
  if (!moved) throw new LocalEntryError(500)
  return moved
}

export async function removeLocalEntry(root: string, path: string): Promise<void> {
  if (isCloudWorkspace(root)) return removeCloudEntry(root, path)
  await postJson('/api/local-documents/remove', { root, path })
}

export async function openWorkspaceFile(root: string, path: string): Promise<void> {
  if (isCloudWorkspace(root)) return openCloudWorkspaceFile(root, path)
  await postJson('/api/workspace-file/open', { root, path })
}

export async function searchWorkspaceEntries(
  root: string,
  query: string,
  signal?: AbortSignal
): Promise<WorkspaceSearchHit[]> {
  if (isCloudWorkspace(root)) return searchCloudEntries(root, query, signal)
  const shell = shellRequest()
  if (!shell || !root) return []
  const params = `root=${encodeURIComponent(root)}&q=${encodeURIComponent(query)}&limit=40`
  const response = await fetch(`${shell.base}/api/workspace-search?${params}`, {
    headers: { Authorization: `Bearer ${shell.token}` },
    signal
  })
  if (!response.ok) throw new LocalEntryError(response.status)
  const body = (await response.json()) as { entries?: unknown }
  if (!Array.isArray(body.entries)) return []
  return body.entries.flatMap((item) => {
    if (!item || typeof item !== 'object') return []
    const row = item as { path?: unknown; relativePath?: unknown; isDirectory?: unknown }
    if (typeof row.path !== 'string' || typeof row.relativePath !== 'string') return []
    if (!insideWorkspace(root, row.path)) return []
    const name = row.relativePath.split('/').pop() ?? ''
    if (!name || name.startsWith('.')) return []
    return [{ path: row.path, relativePath: row.relativePath, isDirectory: row.isDirectory === true }]
  })
}

export function useLocalDocuments(root: string) {
  const [documents, setDocuments] = useState<LibraryDocument[]>([])
  const [folders, setFolders] = useState<string[]>([])
  const [phase, setPhase] = useState<'loading' | 'ready' | 'error'>('loading')

  const reload = useCallback(async () => {
    if (!root) {
      setDocuments((current) => (current.length === 0 ? current : []))
      setFolders((current) => (current.length === 0 ? current : []))
      setPhase('ready')
      return
    }
    try {
      const listing = await readWorkspace(root)
      setFolders(listing.folders)
      setDocuments(listing.documents)
      setPhase('ready')
    } catch {
      setFolders([])
      setDocuments([])
      setPhase('error')
    }
  }, [root])

  useEffect(() => {
    void reload()
  }, [reload])

  return { documents, folders, phase, reload }
}
