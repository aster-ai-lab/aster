/**
 * The personal file manager addressed at an organization drive.
 * Path segments are folder and file names. `a3s-drive://{orgId}` is the root.
 */
import { PERSONAL_CENTER, readCenterId } from '@renderer/data/memberOrgs'
import { DriveApiError, driveApi, type DriveFile, type DriveNode } from '@renderer/drive/driveTransport'
import { readStudioRuntime } from '@shared/studioRuntime'

import type { LocalEntry, LocalEntryListing, WorkspaceSearchHit } from './useLocalDocuments'

export const CLOUD_WORKSPACE_SCHEME = 'a3s-drive://'

export class CloudWorkspaceError extends Error {
  readonly status: number

  constructor(status: number) {
    super(`cloud workspace ${status}`)
    this.name = 'CloudWorkspaceError'
    this.status = status
  }
}

interface CloudRecord {
  id: string
  kind: 'file' | 'directory'
  parentId: string | null
  name: string
  sizeBytes: number
  modifiedMs: number
  agentRetrievals?: number
  parseStatus?: 'parsed' | 'stored' | 'unparsed' | 'parsing'
  compileStatus?: 'waiting' | 'queued' | 'running' | 'done' | 'failed' | 'skipped'
}

const records = new Map<string, CloudRecord>()

export function cloudWorkspaceRoot(orgId: string): string {
  return `${CLOUD_WORKSPACE_SCHEME}${orgId}`
}

export function isCloudWorkspace(root: string): boolean {
  return root.startsWith(CLOUD_WORKSPACE_SCHEME) && !root.slice(CLOUD_WORKSPACE_SCHEME.length).includes('/')
}

export function resetCloudWorkspaceForTests(): void {
  records.clear()
}

function aborted(signal?: AbortSignal): void {
  if (signal?.aborted) throw new DOMException('aborted', 'AbortError')
}

function fail(error: unknown): never {
  if (error instanceof CloudWorkspaceError) throw error
  if (error instanceof DOMException) throw error
  if (error instanceof DriveApiError) throw new CloudWorkspaceError(error.status || 500)
  throw new CloudWorkspaceError(500)
}

function shell(): { base: string; token: string; orgId: string } {
  const runtime = readStudioRuntime()
  const orgId = readCenterId()
  if (!runtime?.token || !runtime.apiBaseUrl || !orgId || orgId === PERSONAL_CENTER) throw new CloudWorkspaceError(503)
  return { base: runtime.apiBaseUrl.replace(/\/$/, ''), token: runtime.token, orgId }
}

function childPath(parent: string, name: string): string {
  return `${parent.replace(/[/\\]+$/u, '')}/${name}`
}

function visibleName(name: string): boolean {
  return (
    Boolean(name) &&
    !name.startsWith('.') &&
    !name.includes('/') &&
    !name.includes('\\') &&
    name.length <= 255 &&
    name.toLowerCase() !== 'wiki'
  )
}

function stamp(value: string | undefined): number {
  const parsed = Date.parse(value ?? '')
  return Number.isFinite(parsed) ? parsed : 0
}

function contentType(name: string): string {
  const extension = name.slice(name.lastIndexOf('.') + 1).toLowerCase()
  if (extension === 'md' || extension === 'markdown') return 'text/markdown'
  if (extension === 'txt') return 'text/plain'
  if (extension === 'csv') return 'text/csv'
  if (extension === 'json') return 'application/json'
  return 'application/octet-stream'
}

function revisionOf(record: CloudRecord): string {
  return `${record.id.replace(/-/g, '')}${record.sizeBytes.toString(16)}${record.modifiedMs.toString(16)}`
    .toLowerCase()
    .replace(/[^a-f0-9]/g, '0')
    .padEnd(64, '0')
    .slice(0, 64)
}

function toEntry(path: string, record: CloudRecord): LocalEntry {
  return {
    name: record.name,
    path,
    kind: record.kind,
    sizeBytes: record.sizeBytes,
    modifiedMs: record.modifiedMs,
    agentRetrievals: record.kind === 'file' ? (record.agentRetrievals ?? 0) : undefined,
    parseStatus: record.kind === 'file' ? record.parseStatus : undefined,
    compileStatus: record.kind === 'file' ? record.compileStatus : undefined,
    driveFileId: record.kind === 'file' ? record.id : undefined
  }
}

function remember(path: string, record: CloudRecord): void {
  records.set(path, record)
}

function dropChildren(directory: string): void {
  const prefix = `${directory}/`
  for (const key of [...records.keys()]) {
    if (!key.startsWith(prefix)) continue
    if (!key.slice(prefix.length).includes('/')) records.delete(key)
  }
}

async function drive<T>(run: () => Promise<T>): Promise<T> {
  try {
    return await run()
  } catch (error) {
    fail(error)
  }
}

async function uploadBytes(
  name: string,
  parentId: string | null,
  bytes: Uint8Array,
  expectedRoot?: string,
  signal?: AbortSignal
): Promise<string> {
  const target = shell()
  if (expectedRoot && cloudWorkspaceRoot(target.orgId) !== expectedRoot) throw new CloudWorkspaceError(409)
  const file = new File([bytes.slice()], name, { type: contentType(name) })
  const form = new FormData()
  form.append('file', file)
  if (parentId) form.append('node_id', parentId)
  let response: Response
  try {
    response = await fetch(`${target.base}/api/drive/${target.orgId}/files`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${target.token}` },
      body: form,
      signal
    })
  } catch (error) {
    fail(error)
  }
  const envelope = (await response.json().catch(() => null)) as {
    success?: boolean
    data?: { id?: string }
    id?: string
  } | null
  if (!response.ok || envelope?.success === false) throw new CloudWorkspaceError(response.status || 500)
  const id = envelope?.data?.id ?? envelope?.id
  if (!id) throw new CloudWorkspaceError(500)
  return id
}

async function replaceBytes(fileId: string, name: string, bytes: Uint8Array, signal?: AbortSignal): Promise<void> {
  const target = shell()
  const file = new File([bytes.slice()], name, { type: contentType(name) })
  const form = new FormData()
  form.append('file', file)
  let response: Response
  try {
    response = await fetch(`${target.base}/api/drive/${target.orgId}/files/${fileId}/content`, {
      method: 'PUT',
      headers: { Authorization: `Bearer ${target.token}` },
      body: form,
      signal
    })
  } catch (error) {
    fail(error)
  }
  if (!response.ok) throw new CloudWorkspaceError(response.status || 500)
  const envelope = (await response.json().catch(() => null)) as { success?: boolean } | null
  if (envelope?.success === false) throw new CloudWorkspaceError(response.status || 500)
}

function editorFileHint(path: string): string | null {
  if (typeof window === 'undefined') return null
  const params = new URLSearchParams(window.location.search)
  if (params.get('file-editor') !== '1' || params.get('path') !== path) return null
  const id = params.get('file') ?? ''
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id) ? id : null
}

async function downloadBytes(
  fileId: string,
  signal?: AbortSignal,
  onProgress?: (ratio: number) => void
): Promise<Uint8Array> {
  const target = shell()
  let response: Response
  try {
    response = await fetch(`${target.base}/api/drive/${target.orgId}/files/${fileId}/content`, {
      cache: 'no-store',
      headers: { Authorization: `Bearer ${target.token}` },
      signal
    })
  } catch (error) {
    if (signal?.aborted) throw new DOMException('aborted', 'AbortError')
    fail(error)
  }
  if (!response.ok) throw new CloudWorkspaceError(response.status || 500)
  const reader = response.body?.getReader()
  if (!reader) {
    const bytes = new Uint8Array(await response.arrayBuffer())
    onProgress?.(1)
    return bytes
  }
  const totalHeader = Number(response.headers.get('content-length'))
  const total = Number.isFinite(totalHeader) && totalHeader > 0 ? totalHeader : 0
  const chunks: Uint8Array[] = []
  let received = 0
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    if (!value) continue
    chunks.push(value)
    received += value.byteLength
    if (total > 0) onProgress?.(Math.min(received / total, 0.99))
  }
  const bytes = new Uint8Array(received)
  let offset = 0
  for (const chunk of chunks) {
    bytes.set(chunk, offset)
    offset += chunk.byteLength
  }
  onProgress?.(1)
  return bytes
}

async function listDirectory(
  root: string,
  directory: string,
  parentId: string | null,
  signal?: AbortSignal
): Promise<LocalEntryListing> {
  aborted(signal)
  if (cloudWorkspaceRoot(shell().orgId) !== root) throw new CloudWorkspaceError(409)
  const [nodes, files] = await Promise.all([
    drive(() => driveApi.listNodes(parentId ?? undefined)),
    drive(() => driveApi.listFiles(parentId ?? undefined))
  ])
  aborted(signal)
  dropChildren(directory)
  const entries: LocalEntry[] = []
  const used = new Set<string>()
  const take = (name: string, record: CloudRecord) => {
    if (!visibleName(name) || used.has(name)) return
    used.add(name)
    const path = childPath(directory, name)
    remember(path, record)
    entries.push(toEntry(path, record))
  }
  for (const node of nodes) take(node.name, directoryRecord(node))
  for (const file of files) take(file.name, fileRecord(file))
  return { entries, capped: files.length >= 500 }
}

function directoryRecord(node: DriveNode): CloudRecord {
  return {
    id: node.id,
    kind: 'directory',
    parentId: node.parentId,
    name: node.name,
    sizeBytes: 0,
    modifiedMs: stamp(node.createdAt)
  }
}

function fileRecord(file: DriveFile): CloudRecord {
  return {
    id: file.id,
    kind: 'file',
    parentId: file.nodeId,
    name: file.name,
    sizeBytes: file.sizeBytes,
    modifiedMs: stamp(file.updatedAt || file.createdAt),
    agentRetrievals: file.agentRetrievals,
    parseStatus: file.parseStatus,
    compileStatus: file.compileStatus
  }
}

async function locate(root: string, path: string, signal?: AbortSignal): Promise<CloudRecord | null> {
  if (!path || path === root) return null
  const prefix = `${root}/`
  if (!path.startsWith(prefix)) throw new CloudWorkspaceError(400)
  const cached = records.get(path)
  if (cached) return cached
  const segments = path.slice(prefix.length).split('/').filter(Boolean)
  let current = root
  let parentId: string | null = null
  for (const segment of segments) {
    await listDirectory(root, current, parentId, signal)
    const next = records.get(childPath(current, segment))
    if (!next) throw new CloudWorkspaceError(404)
    current = childPath(current, segment)
    parentId = next.id
    if (next.kind === 'file' && current !== path) throw new CloudWorkspaceError(404)
  }
  return records.get(path) ?? null
}

async function folderId(root: string, directory: string, signal?: AbortSignal): Promise<string | null> {
  if (!directory || directory === root) return null
  const folder = await locate(root, directory, signal)
  if (!folder || folder.kind !== 'directory') throw new CloudWorkspaceError(404)
  return folder.id
}

export async function listCloudEntries(root: string, directory = '', signal?: AbortSignal): Promise<LocalEntryListing> {
  const folder = !directory || directory === root ? root : directory
  const parentId = await folderId(root, folder, signal)
  return listDirectory(root, folder, parentId, signal)
}

export async function createCloudEntry(
  root: string,
  parent: string,
  name: string,
  kind: 'file' | 'folder'
): Promise<LocalEntry> {
  if (!visibleName(name.trim())) throw new CloudWorkspaceError(400)
  const directory = parent || root
  const listing = await listCloudEntries(root, directory)
  if (listing.entries.some((entry) => entry.name === name.trim())) throw new CloudWorkspaceError(409)
  const parentId = await folderId(root, directory)
  const trimmed = name.trim()
  if (kind === 'folder') {
    const id = await drive(() => driveApi.createFolder(trimmed, parentId ?? undefined))
    const path = childPath(directory, trimmed)
    const record: CloudRecord = { id, kind: 'directory', parentId, name: trimmed, sizeBytes: 0, modifiedMs: Date.now() }
    remember(path, record)
    return toEntry(path, record)
  }
  await uploadBytes(trimmed, parentId, new Uint8Array())
  const refreshed = await listCloudEntries(root, directory)
  const created = refreshed.entries.find((entry) => entry.kind === 'file' && entry.name === trimmed)
  if (!created) throw new CloudWorkspaceError(500)
  return created
}

export async function renameCloudEntry(root: string, path: string, name: string): Promise<LocalEntry> {
  const trimmed = name.trim()
  if (!visibleName(trimmed)) throw new CloudWorkspaceError(400)
  const current = await locate(root, path)
  if (!current) throw new CloudWorkspaceError(404)
  const parent = path.slice(0, path.lastIndexOf('/'))
  const siblings = await listCloudEntries(root, parent)
  if (siblings.entries.some((entry) => entry.name === trimmed && entry.path !== path))
    throw new CloudWorkspaceError(409)
  if (current.kind === 'directory') await drive(() => driveApi.renameNode(current.id, trimmed))
  else await drive(() => driveApi.renameFile(current.id, trimmed))
  records.delete(path)
  const next = childPath(parent, trimmed)
  const record = { ...current, name: trimmed }
  remember(next, record)
  return toEntry(next, record)
}

export async function moveCloudEntry(root: string, path: string, destination: string): Promise<LocalEntry> {
  const current = await locate(root, path)
  if (!current) throw new CloudWorkspaceError(404)
  const directory = !destination || destination === root ? root : destination
  const listing = await listCloudEntries(root, directory)
  if (listing.entries.some((entry) => entry.name === current.name)) throw new CloudWorkspaceError(409)
  const parentId = await folderId(root, directory)
  if (current.kind === 'directory') await drive(() => driveApi.moveNode(current.id, parentId))
  else await drive(() => driveApi.moveFile(current.id, parentId))
  records.delete(path)
  const next = childPath(directory, current.name)
  const record = { ...current, parentId }
  remember(next, record)
  return toEntry(next, record)
}

export async function removeCloudEntry(root: string, path: string): Promise<void> {
  const current = await locate(root, path)
  if (!current) throw new CloudWorkspaceError(404)
  if (current.kind === 'directory') await drive(() => driveApi.deleteNode(current.id))
  else await drive(() => driveApi.deleteFile(current.id))
  const prefix = `${path}/`
  for (const key of [...records.keys()]) {
    if (key === path || key.startsWith(prefix)) records.delete(key)
  }
}

export async function readCloudDocumentBytes(
  root: string,
  path: string,
  onProgress?: (ratio: number) => void,
  maxBytes?: number,
  onSnapshot?: (revision: string) => void,
  signal?: AbortSignal
): Promise<Uint8Array> {
  const hinted = editorFileHint(path)
  if (hinted) {
    const bytes = await downloadBytes(hinted, signal, onProgress)
    if (maxBytes && bytes.byteLength > maxBytes) await tooLarge(bytes.byteLength)
    const cached = records.get(path)
    if (cached?.kind === 'file') remember(path, { ...cached, sizeBytes: bytes.byteLength })
    onSnapshot?.(
      revisionOf({
        id: hinted,
        kind: 'file',
        parentId: cached?.parentId ?? null,
        name: cached?.name ?? path.split('/').pop() ?? 'file',
        sizeBytes: bytes.byteLength,
        modifiedMs: cached?.modifiedMs ?? Date.now()
      })
    )
    return bytes
  }
  const current = await locate(root, path, signal)
  if (!current || current.kind !== 'file') throw new CloudWorkspaceError(404)
  if (maxBytes && current.sizeBytes > maxBytes) await tooLarge(current.sizeBytes)
  const bytes = await downloadBytes(current.id, signal, onProgress)
  if (maxBytes && bytes.byteLength > maxBytes) await tooLarge(bytes.byteLength)
  const observed = { ...current, sizeBytes: bytes.byteLength }
  remember(path, observed)
  onSnapshot?.(revisionOf(observed))
  onProgress?.(1)
  return bytes
}

export async function writeCloudDocumentBytes(
  root: string,
  path: string,
  bytes: Uint8Array,
  expectedRevision?: string
): Promise<string | void> {
  const current = await locate(root, path)
  if (!current || current.kind !== 'file') throw new CloudWorkspaceError(404)
  if (expectedRevision && expectedRevision !== revisionOf(current)) throw new CloudWorkspaceError(409)
  await replaceBytes(current.id, current.name, bytes)
  const observed = { ...current, sizeBytes: bytes.byteLength, modifiedMs: Date.now() }
  remember(path, observed)
  if (expectedRevision === undefined) return undefined
  return revisionOf(observed)
}

async function droppedFileBytes(file: Blob): Promise<Uint8Array> {
  if (typeof file.arrayBuffer === 'function') return new Uint8Array(await file.arrayBuffer())
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer))
    reader.onerror = () => reject(reader.error ?? new Error('read failed'))
    reader.readAsArrayBuffer(file)
  })
}

export async function importCloudFile(
  root: string,
  directory: string,
  file: File,
  signal?: AbortSignal,
  replace = true
): Promise<void> {
  if (!visibleName(file.name)) throw new CloudWorkspaceError(400)
  const parent = directory || root
  const parentId = await folderId(root, parent, signal)
  const bytes = await droppedFileBytes(file)
  aborted(signal)
  const listing = await listCloudEntries(root, parent, signal)
  const existing = listing.entries.find((entry) => entry.name === file.name)
  if (existing) {
    if (!replace || existing.kind !== 'file') throw new CloudWorkspaceError(409)
    const current = records.get(existing.path)
    if (!current || current.kind !== 'file') throw new CloudWorkspaceError(404)
    await replaceBytes(current.id, file.name, bytes, signal)
    remember(existing.path, { ...current, sizeBytes: bytes.byteLength, modifiedMs: Date.now() })
    return
  }
  await uploadBytes(file.name, parentId, bytes, root, signal)
  await listCloudEntries(root, parent, signal)
}

export async function searchCloudEntries(
  root: string,
  query: string,
  signal?: AbortSignal
): Promise<WorkspaceSearchHit[]> {
  const needle = query.trim().toLowerCase()
  if (!needle) return []
  const hits: WorkspaceSearchHit[] = []
  const frontier = [root]
  let visited = 0
  while (frontier.length > 0 && hits.length < 40 && visited < 60) {
    aborted(signal)
    const directory = frontier.shift()
    if (!directory) break
    visited += 1
    const listing = await listCloudEntries(root, directory, signal)
    for (const entry of listing.entries) {
      if (entry.kind === 'directory') frontier.push(entry.path)
      if (!entry.name.toLowerCase().includes(needle)) continue
      const relativePath = entry.path.startsWith(`${root}/`) ? entry.path.slice(root.length + 1) : entry.name
      hits.push({ path: entry.path, relativePath, isDirectory: entry.kind === 'directory' })
      if (hits.length >= 40) break
    }
  }
  return hits
}

export function subscribeCloudDocumentChanges(signal: AbortSignal): Promise<void> {
  if (signal.aborted) return Promise.reject(new DOMException('aborted', 'AbortError'))
  return new Promise((_, reject) => {
    const stop = () => reject(new DOMException('aborted', 'AbortError'))
    signal.addEventListener('abort', stop, { once: true })
  })
}

export async function openCloudWorkspaceFile(root: string, path: string): Promise<void> {
  const bytes = await readCloudDocumentBytes(root, path)
  const name = path.split('/').pop() || 'download'
  const url = URL.createObjectURL(new Blob([bytes.slice()]))
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = name
  anchor.click()
  URL.revokeObjectURL(url)
}

async function tooLarge(bytes: number): Promise<never> {
  const module = await import('./useLocalDocuments')
  throw new module.FileTooLargeError(bytes)
}
