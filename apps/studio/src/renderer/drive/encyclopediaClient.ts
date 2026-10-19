/**
 * Team encyclopedia HTTP client. The selected center only chooses the path.
 * The server resolves the member and filters the rows.
 */
import { PERSONAL_CENTER, readCenterId } from '@renderer/data/memberOrgs'
import { readStudioRuntime } from '@shared/studioRuntime'

import { DriveApiError } from './driveTransport'

export interface EncyclopediaCitation {
  fileId: string
  passageId: string | null
  generation: number
}

export interface EncyclopediaArticle {
  id: string
  title: string
  body: string
  status: 'draft' | 'published' | 'pending'
  audienceKind: 'org' | 'dept'
  audienceDeptId: string | null
  citations: EncyclopediaCitation[]
  writerMemberId: string | null
  sessionLabel: string | null
  canDetach: boolean
}

interface ApiEnvelope<T> {
  code?: number | string
  message?: string
  data?: T
  success?: boolean
}

function orgId(): string {
  const id = readCenterId()
  return id && id !== PERSONAL_CENTER ? id : ''
}

function base(): string {
  const runtime = (globalThis as Record<string, unknown>).__A3S_STUDIO_RUNTIME__ as { apiBaseUrl?: string } | undefined
  const root = runtime?.apiBaseUrl?.replace(/\/$/, '') ?? ''
  return `${root}/api/drive/${orgId()}`
}

function shellToken(): string {
  try {
    return readStudioRuntime()?.token ?? ''
  } catch {
    return ''
  }
}

export function filesDriveHref(fileId: string): string {
  return `/app/files?driveFile=${encodeURIComponent(fileId)}`
}

/** A team passage cited by an agent turn: `team/<file>/<passage>@<generation>`. */
export interface TeamPassageRef {
  fileId: string
  passageId: string
  generation: number
}

const UUID = '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}'
const TEAM_PASSAGE = new RegExp(`^team/(${UUID})/(${UUID})@(\\d{1,15})$`, 'i')

export function parseTeamPassage(place: string): TeamPassageRef | null {
  const match = TEAM_PASSAGE.exec(place.trim())
  if (!match) return null
  return { fileId: match[1], passageId: match[2], generation: Number(match[3]) }
}

export interface TeamPassageHit {
  passageId: string
  fileId: string | null
  heading: string
  folderName: string
  body: string
  /** Entity names that linked this passage to the question. */
  path: string[]
}

interface TeamPassageRaw {
  id: string
  passage_id?: string
  file_id?: string | null
  heading?: string
  folder_name?: string
  body?: string
  path?: string[]
}

/** Passages this member can see, ranked by the server (`fast` mode). */
export async function searchTeamPassages(query: string, signal?: AbortSignal): Promise<TeamPassageHit[]> {
  const data = await request<{ passages?: TeamPassageRaw[] } | []>('/encyclopedia-passages', {
    method: 'POST',
    body: JSON.stringify({ query, mode: 'fast' }),
    signal
  })
  const rows = Array.isArray(data) ? [] : (data.passages ?? [])
  return rows.map((row) => ({
    passageId: row.passage_id ?? row.id,
    fileId: row.file_id ?? null,
    heading: (row.heading ?? '').replace(/^#+\s*/u, ''),
    folderName: row.folder_name ?? '',
    body: row.body ?? '',
    path: row.path ?? []
  }))
}

/** True when a published article already cites a newer generation of the same file. */
export async function teamPassageIsStale(ref: TeamPassageRef, signal?: AbortSignal): Promise<boolean> {
  const articles = await listEncyclopedia(signal)
  return articles.some((article) =>
    article.citations.some((citation) => citation.fileId === ref.fileId && citation.generation > ref.generation)
  )
}

async function request<T>(path: string, init: RequestInit): Promise<T> {
  if (!orgId()) return [] as T
  // 主窗口与登录页不是同一个源，本地存储里没有成员令牌。
  // 环回车道只带壳层令牌，成员会话由壳层附在上游请求上。
  const shell = shellToken()
  if (!shell) throw new DriveApiError('drive.unauthenticated', 'member session missing', 401)
  const response = await fetch(`${base()}${path}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${shell}`,
      'Content-Type': 'application/json',
      ...(init.headers ?? {})
    }
  })
  const envelope = (await response.json().catch(() => null)) as ApiEnvelope<T> | null
  if (!response.ok || envelope?.success === false) {
    throw new DriveApiError(
      String(envelope?.code ?? response.status),
      envelope?.message ?? response.statusText,
      response.status
    )
  }
  return (envelope?.data ?? (envelope as T)) as T
}

function mapArticle(raw: {
  id: string
  title: string
  body: string
  status: EncyclopediaArticle['status']
  audience_kind: EncyclopediaArticle['audienceKind']
  audience_dept_id: string | null
  citations?: Array<{ file_id: string; passage_id: string | null; generation: number }>
  writer_member_id?: string | null
  session_label?: string | null
  can_detach?: boolean
}): EncyclopediaArticle {
  return {
    id: raw.id,
    title: raw.title,
    body: raw.body,
    status: raw.status,
    audienceKind: raw.audience_kind,
    audienceDeptId: raw.audience_dept_id,
    citations: (raw.citations ?? []).map((citation) => ({
      fileId: citation.file_id,
      passageId: citation.passage_id,
      generation: citation.generation
    })),
    writerMemberId: raw.writer_member_id || null,
    sessionLabel: raw.session_label || null,
    canDetach: raw.can_detach === true
  }
}

export interface HandbookFile {
  id: string
  name: string
  contentType: string
  sizeBytes: number
  status: string
  folder: string
  text: string
}

const HANDBOOK_BASE = 'hub_base_handbook'

function runtimeRoot(): string {
  const runtime = (globalThis as Record<string, unknown>).__A3S_STUDIO_RUNTIME__ as { apiBaseUrl?: string } | undefined
  return runtime?.apiBaseUrl?.replace(/\/$/, '') ?? ''
}

function unwrapRows<T>(payload: unknown): T[] {
  if (Array.isArray(payload)) return payload as T[]
  if (!payload || typeof payload !== 'object') return []
  const envelope = payload as ApiEnvelope<T[]>
  if (envelope.success === false) {
    throw new DriveApiError(String(envelope.code ?? 'error'), envelope.message ?? 'request failed', 400)
  }
  return Array.isArray(envelope.data) ? envelope.data : []
}

/**
 * The handbook is the signed-in member's library.
 * Listing it on the team page does not publish those files as team articles.
 */
export async function listHandbook(centerId: string, signal?: AbortSignal): Promise<HandbookFile[]> {
  const scopes = ['personal']
  if (centerId && centerId !== PERSONAL_CENTER) scopes.push(centerId)
  let failure: unknown = null
  const settled = await Promise.all(
    scopes.map(async (scope) => {
      try {
        return await listHandbookScope(scope, signal)
      } catch (reason) {
        if (signal?.aborted) throw reason
        failure ??= reason
        return null
      }
    })
  )
  if (settled.every((rows) => rows === null)) {
    throw failure instanceof Error ? failure : new DriveApiError('error', 'request failed', 400)
  }
  const seen = new Set<string>()
  const files: HandbookFile[] = []
  for (const rows of settled) {
    for (const file of rows ?? []) {
      if (seen.has(file.id)) continue
      seen.add(file.id)
      files.push(file)
    }
  }
  files.sort((left, right) => left.name.localeCompare(right.name, 'zh'))
  return files
}

async function listHandbookScope(scope: string, signal?: AbortSignal): Promise<HandbookFile[]> {
  const shell = shellToken()
  if (!shell) throw new DriveApiError('drive.unauthenticated', 'member session missing', 401)
  const params = new URLSearchParams({ scope, base_id: HANDBOOK_BASE })
  const response = await fetch(`${runtimeRoot()}/api/hub/files?${params}`, {
    method: 'GET',
    signal,
    headers: { Authorization: `Bearer ${shell}`, Accept: 'application/json' }
  })
  const payload = await response.json().catch(() => null)
  if (!response.ok) {
    const envelope = payload as ApiEnvelope<unknown> | null
    throw new DriveApiError(
      String(envelope?.code ?? response.status),
      envelope?.message ?? response.statusText,
      response.status
    )
  }
  return unwrapRows<{
    id: string
    name: string
    content_type?: string
    size_bytes?: number
    status?: string
    folder?: string
    text?: string
  }>(payload).map((row) => ({
    id: row.id,
    name: row.name,
    contentType: row.content_type ?? '',
    sizeBytes: row.size_bytes ?? 0,
    status: row.status ?? 'stored',
    folder: row.folder ?? '',
    text: row.text ?? ''
  }))
}

export async function readHandbookContent(fileId: string, signal?: AbortSignal): Promise<Uint8Array> {
  const shell = shellToken()
  if (!shell) throw new DriveApiError('drive.unauthenticated', 'member session missing', 401)
  const response = await fetch(`${runtimeRoot()}/api/hub/files/${fileId}/content`, {
    method: 'GET',
    signal,
    headers: { Authorization: `Bearer ${shell}` }
  })
  if (!response.ok) {
    throw new DriveApiError(String(response.status), response.statusText, response.status)
  }
  return new Uint8Array(await response.arrayBuffer())
}

export async function listEncyclopedia(signal?: AbortSignal): Promise<EncyclopediaArticle[]> {
  if (!orgId()) return []
  const rows = await request<Array<Parameters<typeof mapArticle>[0]>>('/encyclopedia', { method: 'GET', signal })
  return rows.map(mapArticle)
}

export async function postEncyclopedia(
  articleId: string,
  action: 'publish' | 'return' | 'retry',
  signal?: AbortSignal
) {
  const row = await request<Parameters<typeof mapArticle>[0]>(`/encyclopedia/${articleId}/${action}`, {
    method: 'POST',
    signal
  })
  return mapArticle(row)
}

export async function detachEncyclopedia(articleId: string, title: string, body: string, signal?: AbortSignal) {
  const row = await request<Parameters<typeof mapArticle>[0]>(`/encyclopedia/${articleId}/detach`, {
    method: 'POST',
    body: JSON.stringify({ title, body }),
    signal
  })
  return mapArticle(row)
}
