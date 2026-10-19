/**
 * 组织云盘 HTTP 客户端（apps/api docs/DRIVE.md §4；经 Tauri 壳 /api/im 同款车道模式）。
 * 字节上传走 multipart 直连；列表/树/管理走环回车道。
 */
import { PERSONAL_CENTER, readCenterId } from '@renderer/data/memberOrgs'
import { readStudioRuntime } from '@shared/studioRuntime'

export interface DriveNode {
  id: string
  parentId: string | null
  deptId: string | null
  name: string
  createdAt: string
  excluded: boolean
  canExclude: boolean
}

export interface FileEncyclopediaSource {
  fileId: string
  fileName: string
  heading: string
  excerpt: string
}

export interface FileEncyclopedia {
  fileId: string
  fileName: string
  article: {
    id: string
    title: string
    body: string
    status: string
    sources: FileEncyclopediaSource[]
  } | null
}

interface FileEncyclopediaRaw {
  file_id: string
  file_name: string
  article: {
    id: string
    title: string
    body: string
    status: string
    sources?: Array<{
      file_id: string
      file_name: string
      heading: string
      excerpt: string
    }>
  } | null
}

export interface PipelineParseTask {
  fileId: string
  fileName: string
  status: 'stored' | 'parsing' | 'unparsed'
}

export interface PipelineCompileTask {
  id: string
  fileId: string | null
  fileName: string
  status: 'queued' | 'running' | 'done' | 'failed'
}

export interface PipelineQueue {
  parse: PipelineParseTask[]
  compile: PipelineCompileTask[]
}

interface PipelineRaw {
  parse?: Array<{ file_id?: string; file_name?: string; status?: string }>
  compile?: Array<{ id?: string; file_id?: string | null; file_name?: string; status: string }>
}

export interface DriveFile {
  id: string
  name: string
  contentType: string
  sizeBytes: number
  nodeId: string | null
  deptId: string | null
  createdAt: string
  updatedAt?: string
  agentRetrievals: number
  /** `parsed` when extraction produced text. `unparsed` when it refused the file. */
  parseStatus: 'parsed' | 'stored' | 'unparsed' | 'parsing'
  /** Latest encyclopedia index job for this file. */
  compileStatus: 'waiting' | 'queued' | 'running' | 'done' | 'failed' | 'skipped'
}

export class DriveApiError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly status: number
  ) {
    super(message)
    this.name = 'DriveApiError'
  }
}

interface ApiEnvelope<T> {
  code?: number | string
  message?: string
  data?: T
  success?: boolean
}

function token(): string {
  try {
    return localStorage.getItem('a3s_org_member_token') ?? ''
  } catch {
    return ''
  }
}

function orgId(): string {
  const id = readCenterId()
  return id && id !== PERSONAL_CENTER ? id : ''
}

let cachedByteOrigin: string | null = null

export function resetDriveByteBaseForTests(): void {
  cachedByteOrigin = null
}

/** 字节面（上传/下载）直连基址：网关 origin 经壳层 bootstrap 解析，失败回退车道。 */
export async function driveByteBase(): Promise<string> {
  if (cachedByteOrigin) return cachedByteOrigin
  try {
    const runtime = (globalThis as Record<string, unknown>).__A3S_STUDIO_RUNTIME__ as
      | { apiBaseUrl?: string }
      | undefined
    const loopback = runtime?.apiBaseUrl?.replace(/\/$/, '') ?? ''
    const response = await fetch(`${loopback}/api/bootstrap`)
    const envelope = await response.json()
    const origin = (envelope?.data ?? envelope)?.orgOrigin
    if (typeof origin === 'string' && origin.startsWith('http')) {
      cachedByteOrigin = `${origin.replace(/\/$/, '')}/v1/api/org/drive/${orgId()}`
      return cachedByteOrigin
    }
  } catch {
    // bootstrap 不可用 → 回退车道。
  }
  cachedByteOrigin = driveBase()
  return cachedByteOrigin
}

export function driveBase(): string {
  const runtime = (globalThis as Record<string, unknown>).__A3S_STUDIO_RUNTIME__ as { apiBaseUrl?: string } | undefined
  const base = runtime?.apiBaseUrl?.replace(/\/$/, '') ?? ''
  return `${base}/api/drive/${orgId()}`
}

function shellToken(): string {
  try {
    return readStudioRuntime()?.token ?? ''
  } catch {
    return ''
  }
}

async function unwrap<T>(path: string, init: RequestInit): Promise<T> {
  // 环回车道只认壳层令牌。成员会话在壳层，由壳层附在上游请求上。
  const shell = shellToken()
  if (!shell) throw new DriveApiError('drive.unauthenticated', 'member session missing', 401)
  let response: Response
  try {
    response = await fetch(`${driveBase()}${path}`, {
      ...init,
      cache: 'no-store',
      headers: {
        Authorization: `Bearer ${shell}`,
        'Content-Type': 'application/json',
        ...init.headers
      }
    })
  } catch (error) {
    throw new DriveApiError('drive.network', String(error), 0)
  }
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

// 蛇形 → 驼峰
function mapNode(raw: {
  id: string
  parent_id: string | null
  dept_id: string | null
  name: string
  created_at: string
  excluded?: boolean
  can_exclude?: boolean
}): DriveNode {
  return {
    id: raw.id,
    parentId: raw.parent_id,
    deptId: raw.dept_id,
    name: raw.name,
    createdAt: raw.created_at,
    excluded: raw.excluded === true,
    canExclude: raw.can_exclude === true
  }
}

function mapFile(raw: {
  id: string
  name: string
  content_type: string
  size_bytes: number
  node_id: string | null
  dept_id: string | null
  created_at: string
  updated_at?: string
  agent_retrievals?: number
  parse_status?: string
  compile_status?: string
}): DriveFile {
  const compile = raw.compile_status
  const compileStatus =
    compile === 'waiting' ||
    compile === 'queued' ||
    compile === 'running' ||
    compile === 'done' ||
    compile === 'failed' ||
    compile === 'skipped'
      ? compile
      : 'skipped'
  return {
    id: raw.id,
    name: raw.name,
    contentType: raw.content_type,
    sizeBytes: raw.size_bytes,
    nodeId: raw.node_id,
    deptId: raw.dept_id,
    createdAt: raw.created_at,
    updatedAt: raw.updated_at,
    agentRetrievals: Number(raw.agent_retrievals) || 0,
    parseStatus:
      raw.parse_status === 'parsed' || raw.parse_status === 'unparsed' || raw.parse_status === 'parsing'
        ? raw.parse_status
        : 'stored',
    compileStatus
  }
}

export const driveApi = {
  async listNodes(parentId?: string): Promise<DriveNode[]> {
    const query = parentId ? `?parent_id=${encodeURIComponent(parentId)}` : ''
    const rows = await unwrap<Array<Parameters<typeof mapNode>[0]>>(`/nodes${query}`, { method: 'GET' })
    return rows.map(mapNode)
  },

  async createFolder(name: string, parentId?: string, deptId?: string): Promise<string> {
    const result = await unwrap<{ id: string }>('/nodes', {
      method: 'POST',
      body: JSON.stringify({ name, parent_id: parentId ?? null, dept_id: deptId ?? null })
    })
    return result.id
  },

  async renameNode(nodeId: string, name: string): Promise<void> {
    await unwrap(`/nodes/${nodeId}/name`, { method: 'PUT', body: JSON.stringify({ name }) })
  },

  async moveNode(nodeId: string, parentId: string | null): Promise<void> {
    await unwrap(`/nodes/${nodeId}/parent`, {
      method: 'PUT',
      body: JSON.stringify({ parent_id: parentId })
    })
  },

  async deleteNode(nodeId: string): Promise<void> {
    await unwrap(`/nodes/${nodeId}`, { method: 'DELETE' })
  },

  async renameFile(fileId: string, name: string): Promise<void> {
    await unwrap(`/files/${fileId}/name`, { method: 'PUT', body: JSON.stringify({ name }) })
  },

  async moveFile(fileId: string, parentId: string | null): Promise<void> {
    await unwrap(`/files/${fileId}/parent`, {
      method: 'PUT',
      body: JSON.stringify({ parent_id: parentId })
    })
  },

  async deleteFile(fileId: string): Promise<void> {
    await unwrap(`/files/${fileId}`, { method: 'DELETE' })
  },

  async setFolderEncyclopedia(nodeId: string, excluded: boolean): Promise<void> {
    await unwrap(`/nodes/${nodeId}/encyclopedia`, {
      method: 'PUT',
      body: JSON.stringify({ excluded })
    })
  },

  async reparse(fileId: string): Promise<void> {
    await unwrap(`/files/${encodeURIComponent(fileId)}/parse`, { method: 'POST', body: '{}' })
  },

  async pipeline(): Promise<PipelineQueue> {
    const raw = await unwrap<PipelineRaw>('/pipeline', { method: 'GET' })
    const compileStatus = (value: string): PipelineCompileTask['status'] | null =>
      value === 'queued' || value === 'running' || value === 'done' || value === 'failed' ? value : null
    return {
      parse: (raw.parse ?? []).flatMap((row) =>
        row.file_id && row.file_name
          ? [
              {
                fileId: row.file_id,
                fileName: row.file_name,
                status: row.status === 'parsing' || row.status === 'unparsed' ? row.status : 'stored'
              }
            ]
          : []
      ),
      compile: (raw.compile ?? []).flatMap((row) => {
        const status = compileStatus(row.status)
        if (!row.id || !status) return []
        return [{ id: row.id, fileId: row.file_id ?? null, fileName: row.file_name || '', status }]
      })
    }
  },

  async searchEncyclopedia(
    query: string
  ): Promise<{
    phase: 'not-ready' | 'published'
    hits: Array<{ id: string; heading: string; folder: string; body: string }>
  }> {
    const rows = await unwrap<{
      phase?: string
      passages?: Array<{ id?: string; heading?: string; folder_name?: string; body?: string }>
    }>('/encyclopedia-passages', { method: 'POST', body: JSON.stringify({ query }) })
    const hits = (rows.passages ?? []).flatMap((row) => {
      if (!row.body?.trim()) return []
      return [{ id: row.id || row.body, heading: row.heading ?? '', folder: row.folder_name ?? '', body: row.body }]
    })
    return { phase: rows.phase === 'not-ready' ? 'not-ready' : 'published', hits }
  },

  async listEncyclopedia(): Promise<Array<{ id: string; title: string; body: string; status: string }>> {
    const rows = await unwrap<Array<{ id?: string; title?: string; body?: string; status?: string }>>('/encyclopedia', {
      method: 'GET'
    })
    return rows.flatMap((row) => {
      if (!row.id || !row.title || typeof row.body !== 'string') return []
      return [{ id: row.id, title: row.title, body: row.body, status: row.status ?? '' }]
    })
  },

  async fileEncyclopedia(fileId: string): Promise<FileEncyclopedia> {
    const raw = await unwrap<FileEncyclopediaRaw>(`/files/${encodeURIComponent(fileId)}/encyclopedia`, {
      method: 'GET'
    })
    return {
      fileId: raw.file_id,
      fileName: raw.file_name,
      article: raw.article
        ? {
            id: raw.article.id,
            title: raw.article.title,
            body: raw.article.body,
            status: raw.article.status,
            sources: (raw.article.sources ?? []).map((source) => ({
              fileId: source.file_id,
              fileName: source.file_name,
              heading: source.heading,
              excerpt: source.excerpt
            }))
          }
        : null
    }
  },

  async listFiles(nodeId?: string): Promise<DriveFile[]> {
    const query = nodeId ? `?node_id=${encodeURIComponent(nodeId)}` : ''
    const rows = await unwrap<Array<Parameters<typeof mapFile>[0]>>(`/files${query}`, { method: 'GET' })
    return rows.map(mapFile)
  },

  /** 上传：multipart 直连网关（字节不经环回车道——同 IM SSE 的直连原则）。 */
  async upload(file: File, nodeId?: string, deptId?: string, sessionLabel?: string): Promise<void> {
    const form = new FormData()
    form.append('file', file)
    if (nodeId) form.append('node_id', nodeId)
    if (deptId) form.append('dept_id', deptId)
    if (sessionLabel && sessionLabel.length <= 64) form.append('session_label', sessionLabel)
    const response = await fetch(`${await driveByteBase()}/files`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${token()}` },
      body: form
    }).catch(() => null)
    if (!response || !response.ok) {
      throw new DriveApiError('drive.upload_failed', 'upload failed', response?.status ?? 0)
    }
  },

  /** 下载 URL（带 token 的直连不可行——EventSource 同款限制；走 fetch blob 由调用端保存）。 */
  async downloadBlob(fileId: string): Promise<Blob> {
    const response = await fetch(`${await driveByteBase()}/files/${fileId}/content`, {
      headers: { Authorization: `Bearer ${token()}` }
    }).catch(() => null)
    if (!response || !response.ok) {
      throw new DriveApiError('drive.download_failed', 'download failed', response?.status ?? 0)
    }
    return response.blob()
  }
}
