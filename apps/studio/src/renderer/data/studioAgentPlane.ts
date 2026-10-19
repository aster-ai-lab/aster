import { cacheService } from '@renderer/data/CacheService'
import type { DataApiDataChangeEffect, DataRequest, DataResponse } from '@shared/data/api/types'
import { readStudioRuntime, type StudioRuntimeDescriptor } from '@shared/studioRuntime'
import { agentFormSubmissionSchema, agentFormReceiptSchema, type AgentFormSubmission } from '@shared/types/agentForm'
import {
  agentSessionArchiveIdentitySchema,
  agentSessionArchiveRequestSchema,
  agentSessionArchiveReceiptSchema,
  agentSessionArchiveLookupSchema
} from '@shared/types/agentSessionArchive'
import { studioExecutionObservationSchema, studioExecutionSnapshotSchema } from '@shared/types/studioExecution'
import {
  workbenchProofSchema,
  workbenchProofActionSchema,
  type WorkbenchProofAction
} from '@shared/types/workbenchProof'

/**
 * Cherry's agents, sessions, and turns live behind Electron's main process. In
 * the Tauri shell they are served by the Rust agent plane (a3s-boot, under
 * `/api/agents*`, `/api/agent-sessions*`), which runs a3s-code over ACP and
 * fans stream events out over SSE. This module routes the DataApi paths and
 * the `ai.agent.*` / `ai.stream.*` IPC there. Active only when the Tauri
 * runtime descriptor is present.
 */

type Listener = (payload: unknown) => void
type DataChangeListener = (effects: DataApiDataChangeEffect[]) => void

const TOPIC_PREFIX = 'agent-session:'

class PlaneHttpError extends Error {
  constructor(
    readonly status: number,
    message: string
  ) {
    super(message)
  }
}

let planeUrl: Promise<string> | null = null
let ipcInstalled = false
let events: globalThis.EventSource | null = null
let eventsConnected = false
let executionReady = false
const listeners = new Map<string, Set<Listener>>()
const dataChangeListeners = new Set<DataChangeListener>()

export async function loadWorkbenchProofs(sessionId: string) {
  const value = await planeJson('GET', `/api/agent-sessions/${encodeURIComponent(sessionId)}/proofs`)
  if (!Array.isArray(value)) throw new Error('Invalid workbench evidence')
  const proofs = value.map((row) => workbenchProofSchema.parse(row))
  if (proofs.some((proof) => proof.sessionId !== sessionId)) throw new Error('Invalid workbench evidence scope')
  return proofs
}

export async function actOnWorkbenchProof(sessionId: string, messageId: string, action: WorkbenchProofAction) {
  const body = workbenchProofActionSchema.parse(action)
  const proof = workbenchProofSchema.parse(
    await planeJson(
      'POST',
      `/api/agent-sessions/${encodeURIComponent(sessionId)}/messages/${encodeURIComponent(messageId)}/proof`,
      body
    )
  )
  if (proof.sessionId !== sessionId || proof.messageId !== messageId) throw new Error('Invalid review response scope')
  sessionChanged(sessionId)
  return proof
}

export async function loadWorkbenchArtifact(sessionId: string, artifactId: string, signal: AbortSignal) {
  const value = await planeJson(
    'GET',
    `/api/agent-sessions/${encodeURIComponent(sessionId)}/proof-artifacts/${encodeURIComponent(artifactId)}`,
    undefined,
    signal
  )
  if (!value || typeof value !== 'object') throw new Error('Invalid artifact snapshot')
  const row = value as Record<string, unknown>
  if (
    row.id !== artifactId ||
    typeof row.name !== 'string' ||
    typeof row.mediaType !== 'string' ||
    typeof row.contentDigest !== 'string' ||
    typeof row.data !== 'string' ||
    row.data.length > 30 * 1024 * 1024
  )
    throw new Error('Invalid artifact snapshot')
  return {
    id: artifactId,
    name: row.name,
    mediaType: row.mediaType,
    contentDigest: row.contentDigest,
    data: row.data,
    text: typeof row.text === 'string' ? row.text : undefined
  }
}

export async function publishMailToBotPolicy(policy: {
  allow: boolean
  list: boolean
  read: boolean
  folders: readonly string[]
}): Promise<void> {
  await planeJson('POST', '/mcp/member-mail/policy', policy)
}

export async function publishWorkToBotPolicy(policy: {
  allow: boolean
  list: boolean
  read: boolean
  kinds: readonly string[]
}): Promise<void> {
  await planeJson('POST', '/mcp/member-work/policy', policy)
}

/** Last compact deck text from the open 演示文稿 view. */
export async function publishPresentonDeck(text: string): Promise<void> {
  await planeJson('POST', '/mcp/presenton/snapshot', { text })
}

/** Submit one pending IntelligentUI form; false when the plane rejects it. */
export async function submitAgentForm(
  submission: AgentFormSubmission,
  signal: globalThis.AbortSignal
): Promise<boolean> {
  const parsed = agentFormSubmissionSchema.safeParse(submission)
  if (!parsed.success || signal.aborted) return false
  const result = agentFormReceiptSchema.safeParse(
    await planeJson('POST', `/api/agent-forms/${parsed.data.binding.requestId}`, parsed.data, signal)
  )
  return !signal.aborted && result.success && result.data.requestId === parsed.data.binding.requestId && result.data.ok
}

export function resetStudioAgentPlaneForTests() {
  planeUrl = null
  ipcInstalled = false
  events?.close()
  events = null
  eventsConnected = false
  invalidateExecutions()
  listeners.clear()
  dataChangeListeners.clear()
}

function runtimeOrNull(): StudioRuntimeDescriptor | null {
  try {
    return readStudioRuntime() ?? null
  } catch {
    return null
  }
}

function resolvePlane(runtime: StudioRuntimeDescriptor): Promise<string> {
  if (!planeUrl) {
    planeUrl = globalThis
      .fetch(`${runtime.apiBaseUrl}/api/agent-plane`, {
        headers: { authorization: `Bearer ${runtime.token}` }
      })
      .then(async (response) => {
        const body = (await response.json().catch(() => null)) as { baseUrl?: unknown } | null
        if (!response.ok || typeof body?.baseUrl !== 'string') {
          throw new PlaneHttpError(503, 'agent runtime is unavailable')
        }
        return body.baseUrl
      })
      .catch((error: unknown) => {
        planeUrl = null
        throw error
      })
  }
  return planeUrl
}

async function planeJson(
  method: string,
  path: string,
  body?: unknown,
  signal?: globalThis.AbortSignal
): Promise<unknown> {
  const runtime = runtimeOrNull()
  if (!runtime) throw new PlaneHttpError(503, 'agent runtime is unavailable')
  const base = await resolvePlane(runtime)
  signal?.throwIfAborted()
  const response = await globalThis.fetch(`${base}${path}`, {
    method,
    signal,
    cache: 'no-store',
    headers: {
      authorization: `Bearer ${runtime.token}`,
      'cache-control': 'no-store',
      ...(body !== undefined ? { 'content-type': 'application/json' } : {})
    },
    body: body !== undefined ? JSON.stringify(body) : undefined
  })
  const text = await response.text()
  const parsed = text ? (JSON.parse(text) as unknown) : null
  if (!response.ok) {
    const message =
      parsed && typeof parsed === 'object' && typeof (parsed as { message?: unknown }).message === 'string'
        ? (parsed as { message: string }).message
        : `agent plane ${response.status}`
    throw new PlaneHttpError(response.status, message)
  }
  return parsed
}

function agentPath(path: string): boolean {
  return (
    path === '/agents' ||
    /^\/agents\/[^/]+$/.test(path) ||
    /^\/agents\/[^/]+\/tasks(\/|$)/.test(path) ||
    path === '/agent-tasks' ||
    path.startsWith('/agent-tasks/') ||
    path === '/agent-sessions' ||
    path.startsWith('/agent-sessions/') ||
    path === '/pins' ||
    /^\/pins\/[^/]+$/.test(path)
  )
}

function queryString(params: unknown): string {
  if (!params || typeof params !== 'object') return ''
  const search = new globalThis.URLSearchParams()
  for (const [key, value] of Object.entries(params as Record<string, unknown>)) {
    if (value === undefined || value === null || value === '') continue
    search.set(key, Array.isArray(value) ? value.join(',') : String(value))
  }
  const text = search.toString()
  return text ? `?${text}` : ''
}

/** The plane stores sessions with their resolved workspace; user workspaces live in the bridge. */
async function resolveWorkspace(source: unknown): Promise<unknown> {
  if (!source || typeof source !== 'object') return source
  const record = source as { type?: unknown; workspaceId?: unknown; place?: unknown; path?: unknown }
  if (record.type !== 'user' || typeof record.workspaceId !== 'string') {
    const inbox: Record<string, unknown> = { type: record.type }
    if (record.place === 'inbox') {
      inbox.place = 'inbox'
      if (typeof record.path === 'string' && record.path.trim()) inbox.path = record.path
    }
    return inbox
  }
  const response = await globalThis.api?.dataApi?.request({
    id: `studio-agent-workspace-${record.workspaceId}`,
    method: 'GET',
    path: `/agent-workspaces/${encodeURIComponent(record.workspaceId)}`
  } as DataRequest)
  if (!response || response.error || !response.data) {
    throw new PlaneHttpError(404, 'workspace not found')
  }
  const resolved: Record<string, unknown> = { type: 'user', workspace: response.data }
  if (record.place === 'project') resolved.place = 'project'
  return resolved
}

async function withWorkspace(body: unknown): Promise<Record<string, unknown>> {
  const record = body && typeof body === 'object' ? { ...(body as Record<string, unknown>) } : {}
  if ('workspace' in record) record.workspace = await resolveWorkspace(record.workspace)
  return record
}

async function dispatch(request: DataRequest): Promise<unknown> {
  const method = request.method
  const path = request.path
  if (path === '/agent-sessions' && method === 'POST') {
    return planeJson('POST', '/api/agent-sessions', await withWorkspace(request.body))
  }
  if (/^\/agent-sessions\/[^/]+\/workspace$/.test(path) && method === 'PUT') {
    return planeJson('PUT', `/api${path}`, await resolveWorkspace(request.body))
  }
  if (method === 'GET') return planeJson('GET', `/api${path}${queryString(request.params)}`)
  if (method === 'DELETE' && (/^\/pins\/[^/]+$/.test(path) || /^\/agent-sessions\/[^/]+\/messages\/[^/]+$/.test(path)))
    return planeJson('DELETE', `/api${path}`)
  if (method === 'PATCH' || method === 'POST') return planeJson(method, `/api${path}`, request.body ?? {})
  throw new PlaneHttpError(405, `${method} ${path} is not supported`)
}

function errorCode(status: number): string {
  if (status === 400) return 'VALIDATION_ERROR'
  if (status === 404) return 'NOT_FOUND'
  if (status === 405) return 'METHOD_NOT_ALLOWED'
  if (status === 409) return 'CONFLICT'
  if (status === 503) return 'SERVICE_UNAVAILABLE'
  return 'INTERNAL_SERVER_ERROR'
}

export async function answerStudioAgentRequest(request: DataRequest): Promise<DataResponse | null> {
  if (!runtimeOrNull() || !agentPath(String(request?.path || ''))) return null
  installStudioAgentIpc()
  try {
    return { id: request.id, status: 200, data: await dispatch(request), metadata: { timestamp: Date.now() } }
  } catch (error) {
    const status = error instanceof PlaneHttpError ? error.status : 500
    const message = error instanceof Error ? error.message : 'agent request failed'
    return {
      id: request.id,
      status,
      error: { code: errorCode(status), message, status },
      metadata: { timestamp: Date.now() }
    } as DataResponse
  }
}

function sessionOf(input: unknown): string | null {
  const topicId = input && typeof input === 'object' ? (input as { topicId?: unknown }).topicId : undefined
  if (typeof topicId !== 'string' || !topicId.startsWith(TOPIC_PREFIX)) return null
  return encodeURIComponent(topicId.slice(TOPIC_PREFIX.length))
}

async function ipcResult(pending: Promise<unknown>) {
  try {
    return { ok: true, data: await pending }
  } catch (error) {
    return {
      ok: false,
      error: { code: 'INTERNAL', message: error instanceof Error ? error.message : 'agent request failed' }
    }
  }
}

function field(input: unknown, key: string): unknown {
  return input && typeof input === 'object' ? (input as Record<string, unknown>)[key] : undefined
}

function forgetSessions(path: string, input: unknown): Promise<unknown> | null {
  const sessionIds = field(input, 'sessionIds')
  if (!Array.isArray(sessionIds)) return null
  return ipcResult(
    planeJson('POST', path, { sessionIds }).then((data) => {
      const deleted = data && typeof data === 'object' ? (data as { deletedIds?: unknown }).deletedIds : undefined
      if (Array.isArray(deleted)) {
        for (const id of deleted) {
          if (typeof id === 'string') sessionChanged(id)
        }
      }
      return data
    })
  )
}

function routeIpc(route: string, input: unknown): Promise<unknown> | null {
  if (route === 'ai.agent.session.archive') {
    return ipcResult(
      Promise.resolve().then(async () => {
        const request = agentSessionArchiveRequestSchema.parse(input)
        const receipt = agentSessionArchiveReceiptSchema.parse(
          await planeJson('POST', '/api/agent-sessions/archive', request)
        )
        const ids = new Set(request.sessionIds)
        if (
          receipt.operationId !== request.operationId ||
          receipt.epoch !== request.epoch ||
          receipt.results.length !== ids.size ||
          receipt.results.some((item) => !ids.has(item.sessionId))
        ) {
          throw new Error('Archive receipt does not match the submitted selection')
        }
        sessionsChanged(request.sessionIds)
        return receipt
      })
    )
  }
  if (route === 'ai.agent.session.archive_outcome') {
    return ipcResult(
      Promise.resolve().then(async () => {
        const request = agentSessionArchiveIdentitySchema.parse(input)
        const lookup = agentSessionArchiveLookupSchema.parse(
          await planeJson(
            'GET',
            `/api/agent-sessions/archive-operations/${encodeURIComponent(request.operationId)}?epoch=${encodeURIComponent(request.epoch)}`
          )
        )
        if (lookup.receipt && lookup.receipt.operationId !== request.operationId)
          throw new Error('Archive receipt belongs to another operation')
        return lookup
      })
    )
  }
  if (route === 'ai.agent.create') return ipcResult(planeJson('POST', '/api/agents', input ?? {}))
  const taskRoute = taskIpc(route, input)
  if (taskRoute) return taskRoute
  if (route === 'ai.agent.session.reuse_or_create') {
    return ipcResult(
      withWorkspace(input).then((body) => planeJson('POST', '/api/agent-sessions/reuse-or-create', body))
    )
  }
  if (route === 'ai.agent.session.fork') {
    const sourceSessionId = field(input, 'sourceSessionId')
    const messageId = field(input, 'messageId')
    if (typeof sourceSessionId !== 'string' || typeof messageId !== 'string') return null
    return ipcResult(
      planeJson('POST', `/api/agent-sessions/${encodeURIComponent(sourceSessionId)}/fork`, { messageId }).then(
        (data) => {
          const sessionId = data && typeof data === 'object' ? (data as { sessionId?: unknown }).sessionId : undefined
          if (typeof sessionId === 'string') sessionChanged(sessionId)
          return data
        }
      )
    )
  }
  if (route === 'ai.agent.session.delete') {
    return forgetSessions(
      field(input, 'permanent') === true ? '/api/agent-sessions/delete' : '/api/agent-sessions/archive',
      input
    )
  }
  if (route === 'ai.agent.session.delete_permanently') return forgetSessions('/api/agent-sessions/delete', input)
  if (route === 'ai.agent.session.restore') {
    const id = field(input, 'sessionId')
    if (typeof id !== 'string' || !id.trim()) return null
    return ipcResult(
      planeJson('POST', `/api/agent-sessions/${encodeURIComponent(id)}/restore`, {}).then((data) => {
        sessionChanged(id)
        return data
      })
    )
  }
  if (
    route === 'ai.agent.session.prewarm' ||
    route === 'ai.agent.session.close_warm' ||
    route === 'ai.agent.session.refresh_context_usage' ||
    route === 'ai.agent.session.set_pending_input_count'
  ) {
    return Promise.resolve({ ok: true, data: undefined })
  }
  if (route === 'ai.tool.respond_approval') {
    const approvalId = field(input, 'approvalId')
    if (typeof approvalId !== 'string') return null
    return ipcResult(
      planeJson('POST', `/api/agent-approvals/${encodeURIComponent(approvalId)}`, {
        approved: field(input, 'approved') === true
      })
    )
  }
  if (!route.startsWith('ai.stream.')) return null
  const session = sessionOf(input)
  if (!session) return null
  ensureEvents()
  if (route === 'ai.stream.open') {
    return ipcResult(planeJson('POST', `/api/agent-sessions/${session}/turns`, input))
  }
  if (route === 'ai.stream.attach') return ipcResult(planeJson('GET', `/api/agent-sessions/${session}/turn`))
  if (route === 'ai.stream.abort') {
    return ipcResult(planeJson('POST', `/api/agent-sessions/${session}/turn/abort`).then(() => undefined))
  }
  if (route === 'ai.stream.detach') return Promise.resolve({ ok: true, data: undefined })
  return null
}

function emit(channel: string, payload: unknown) {
  for (const listener of listeners.get(channel) ?? []) {
    try {
      listener(payload)
    } catch {
      // One subscriber failing must not starve the rest.
    }
  }
}

function taskIdOf(input: unknown): string | null {
  const taskId = field(input, 'taskId')
  return typeof taskId === 'string' && taskId ? taskId : null
}

async function attachWorkspace(record: Record<string, unknown>): Promise<Record<string, unknown>> {
  const workspace = record.workspace
  if (!workspace || typeof workspace !== 'object' || (workspace as { type?: unknown }).type !== 'user') {
    return record
  }
  const workspaceId = (workspace as { workspaceId?: unknown }).workspaceId
  if (typeof workspaceId !== 'string' || !workspaceId) return record
  const response = await globalThis.api?.dataApi?.request({
    id: `studio-agent-workspace-${workspaceId}`,
    method: 'GET',
    path: `/agent-workspaces/${encodeURIComponent(workspaceId)}`
  } as DataRequest)
  const data = response && !response.error ? (response.data as { path?: unknown; name?: unknown }) : undefined
  if (!data || typeof data.path !== 'string' || !data.path) return record
  return {
    ...record,
    workspacePath: data.path,
    workspaceName: typeof data.name === 'string' ? data.name : workspaceId
  }
}

async function taskBody(input: unknown): Promise<Record<string, unknown>> {
  const record = input && typeof input === 'object' ? { ...(input as Record<string, unknown>) } : {}
  if (record.patch && typeof record.patch === 'object') {
    record.patch = await attachWorkspace({ ...(record.patch as Record<string, unknown>) })
    return record
  }
  return attachWorkspace(record)
}

function taskIpc(route: string, input: unknown): Promise<unknown> | null {
  const taskId = taskIdOf(input)
  if (route === 'ai.agent.task.create') {
    return ipcResult(taskBody(input).then((body) => planeJson('POST', '/api/agent-tasks', body)))
  }
  if (!taskId) return null
  const id = encodeURIComponent(taskId)
  if (route === 'ai.agent.task.update') {
    return ipcResult(taskBody(input).then((body) => planeJson('PATCH', `/api/agent-tasks/${id}`, body)))
  }
  if (route === 'ai.agent.task.pause') {
    return ipcResult(planeJson('POST', `/api/agent-tasks/${id}/pause`, input ?? {}))
  }
  if (route === 'ai.agent.task.resume') {
    return ipcResult(planeJson('POST', `/api/agent-tasks/${id}/resume`, input ?? {}))
  }
  if (route === 'ai.agent.task.delete') {
    const agentId = field(input, 'agentId')
    const query = typeof agentId === 'string' ? `?agentId=${encodeURIComponent(agentId)}` : ''
    return ipcResult(planeJson('DELETE', `/api/agent-tasks/${id}${query}`))
  }
  if (route === 'ai.agent.task.run') {
    return ipcResult(planeJson('POST', `/api/agent-tasks/${id}/run`, input ?? {}).then(() => undefined))
  }
  return null
}

function taskChanged(taskId: string) {
  const effects = [
    { endpoint: '/agent-tasks', kind: 'membership' },
    { endpoint: '/agent-tasks/:taskId', entityIds: [taskId] },
    { endpoint: '/agents/:agentId/tasks', kind: 'membership' }
  ] as unknown as DataApiDataChangeEffect[]
  for (const listener of dataChangeListeners) listener(effects)
}

function sessionChanged(sessionId: string) {
  sessionsChanged([sessionId])
}

function sessionsChanged(sessionIds: readonly string[]) {
  const route = sessionIds.length === 1 ? { routeParams: { sessionId: sessionIds[0] } } : {}
  const effects = [
    { endpoint: '/agent-sessions', kind: 'membership' },
    { endpoint: '/agent-sessions/latest' },
    { endpoint: '/agent-sessions/:sessionId', ...route, entityIds: [...sessionIds] },
    { endpoint: '/agent-sessions/:sessionId/messages', kind: 'membership', ...route }
  ] as unknown as DataApiDataChangeEffect[]
  for (const listener of dataChangeListeners) listener(effects)
}

function invalidateExecutions() {
  executionReady = false
  cacheService.set('studio.execution.observations', null)
}

function handleEvent(raw: string) {
  if (!eventsConnected) return
  let event: { channel?: unknown; payload?: unknown }
  try {
    event = JSON.parse(raw) as { channel?: unknown; payload?: unknown }
  } catch {
    return
  }
  if (!event || typeof event !== 'object') return
  if (event.channel === 'execution.invalidated') {
    invalidateExecutions()
    return
  }
  if (event.channel === 'execution.snapshot') {
    invalidateExecutions()
    const parsed = studioExecutionSnapshotSchema.safeParse(event.payload)
    if (!parsed.success || new Set(parsed.data.topics.map((topic) => topic.topicId)).size !== parsed.data.topics.length)
      return
    cacheService.set(
      'studio.execution.observations',
      Object.fromEntries(parsed.data.topics.map((topic) => [topic.topicId, topic.execution]))
    )
    for (const row of parsed.data.liveStatuses ?? []) {
      const status = { ...row.status }
      delete status.nativeExecution
      cacheService.setShared(`topic.stream.statuses.${row.topicId}` as never, status as never)
    }
    executionReady = true
    return
  }
  const payload = event.payload as Record<string, unknown> | undefined
  if (event.channel === 'topic.status') {
    const topicId = payload?.topicId
    if (typeof topicId === 'string') {
      const rawStatus = payload?.status
      if (rawStatus && typeof rawStatus === 'object' && !Array.isArray(rawStatus)) {
        const { nativeExecution, ...status } = rawStatus as Record<string, unknown>
        if (executionReady) {
          const observations = { ...cacheService.get('studio.execution.observations') }
          const parsed = studioExecutionObservationSchema.safeParse(nativeExecution)
          if (parsed.success && /^agent-session:[^\p{Cc}]+$/u.test(topicId) && topicId.length <= 256)
            observations[topicId] = parsed.data
          else delete observations[topicId]
          cacheService.set('studio.execution.observations', observations)
        }
        cacheService.setShared(`topic.stream.statuses.${topicId}` as never, status as never)
      } else {
        if (executionReady) {
          const observations = { ...cacheService.get('studio.execution.observations') }
          delete observations[topicId]
          cacheService.set('studio.execution.observations', observations)
        }
        cacheService.setShared(`topic.stream.statuses.${topicId}` as never, null as never)
      }
    }
    return
  }
  if (event.channel === 'data.changed') {
    if (typeof payload?.sessionId === 'string') sessionChanged(payload.sessionId)
    if (
      Array.isArray(payload?.sessionIds) &&
      payload.sessionIds.length > 0 &&
      payload.sessionIds.length <= 200 &&
      payload.sessionIds.every((id) => typeof id === 'string' && id.length > 0)
    )
      sessionsChanged(payload.sessionIds)
    if (typeof payload?.taskId === 'string') taskChanged(payload.taskId)
    return
  }
  if (typeof event.channel === 'string') emit(event.channel, payload)
}

function ensureEvents() {
  if (events || typeof globalThis.EventSource !== 'function') return
  const runtime = runtimeOrNull()
  if (!runtime) return
  void resolvePlane(runtime)
    .then((base) => {
      if (events) return
      events = new globalThis.EventSource(`${base}/api/agent-events?token=${encodeURIComponent(runtime.token)}`)
      events.onopen = () => {
        eventsConnected = true
        invalidateExecutions()
      }
      events.onerror = () => {
        eventsConnected = false
        invalidateExecutions()
      }
      events.onmessage = (message) => handleEvent(String(message.data))
    })
    .catch(() => {
      // Turn requests surface the unavailable runtime themselves.
    })
}

export function installStudioAgentIpc() {
  if (ipcInstalled || !runtimeOrNull()) return
  const bridge = globalThis.api?.ipcApi
  if (!bridge?.request) return
  ipcInstalled = true
  const request = bridge.request.bind(bridge)
  const on = bridge.on?.bind(bridge)
  bridge.request = (route: string, input?: unknown) => routeIpc(route, input) ?? request(route, input)
  bridge.on = (channel: string, callback: Listener) => {
    if (!channel.startsWith('ai.stream.')) return on ? on(channel, callback) : () => {}
    ensureEvents()
    let set = listeners.get(channel)
    if (!set) {
      set = new Set()
      listeners.set(channel, set)
    }
    set.add(callback)
    return () => {
      set.delete(callback)
    }
  }
  const dataApi = globalThis.api?.dataApi
  if (dataApi) {
    const onDataChanged = dataApi.onDataChanged?.bind(dataApi)
    dataApi.onDataChanged = (callback: DataChangeListener) => {
      dataChangeListeners.add(callback)
      const detach = onDataChanged?.(callback)
      return () => {
        dataChangeListeners.delete(callback)
        detach?.()
      }
    }
  }
}
