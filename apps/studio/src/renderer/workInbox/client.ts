import { MemberOrgError, memberRequest, readCenterId } from '@renderer/data/memberOrgs'
import { readAppletRuntime, type AppletRuntimeDescriptor } from '@shared/appletRuntime'

export type WorkKind =
  | 'join_request'
  | 'approval'
  | 'leave'
  | 'travel'
  | 'expense'
  | 'meeting'
  | 'seal'
  | 'notice'
  | 'correction'
  | 'probation'
  | 'ai_resource'
export type WorkCreateKind = 'approval' | 'leave' | 'travel' | 'expense' | 'meeting' | 'seal'
export type WorkStatus =
  | 'pending'
  | 'approved'
  | 'rejected'
  | 'cancelled'
  | 'draft'
  | 'returned'
  | 'paid'
  | 'fulfillment_exception'
export type WorkBox = 'todo' | 'done' | 'cc' | 'sent'
export type WorkDecision = 'approve' | 'reject' | 'cancel'

export type WorkTrailMark = {
  action: string
  node_name: string
  actor_name: string
}

export type WorkItem = {
  id: string
  org_id: string
  org_name: string
  kind: WorkKind
  title: string
  summary: string
  status: WorkStatus
  initiator_name: string
  current_node: string
  created_at: string
  actionable: boolean
  approvable?: boolean
  rejectable?: boolean
  cancellable: boolean
  fields: Record<string, unknown>
  decision_comment: string
  assignee_names: string[]
  decider_name: string
  decided_at: string
  schedule_retry: boolean
  engine: boolean
  trail: WorkTrailMark[]
}

export type WorkApprover = { id: string; name: string }

export type WorkSummary = {
  todo: number
  done: number
  cc: number
  sent: number
}

export type WorkListQuery = {
  box: WorkBox
  kind?: WorkKind
  status?: WorkStatus
  q?: string
  days?: number
  department?: string
}

export type WorkPreview = {
  version: number
  nodes: { key: string; name: string; assignees: string[] }[]
}

const EMPTY_SUMMARY: WorkSummary = { todo: 0, done: 0, cc: 0, sent: 0 }
const WORK_KINDS = new Set<WorkKind>([
  'join_request',
  'approval',
  'leave',
  'travel',
  'expense',
  'meeting',
  'seal',
  'notice',
  'correction',
  'probation',
  'ai_resource'
])
const WORK_STATUSES = new Set<WorkStatus>([
  'pending',
  'approved',
  'rejected',
  'cancelled',
  'draft',
  'returned',
  'paid',
  'fulfillment_exception'
])
const PROCESS_WINDOWS = new Set(['leave', 'expense', 'travel', 'correction', 'probation', 'ai-resource', 'clock'])

/** Office applets may create one approval. Notice is not an approval. */
const OFFICE_CREATE = new Set(['approval', 'leave', 'travel', 'expense', 'meeting', 'seal'])

/**
 * Inbox reads and writes with its own ticket.
 * An office applet uses that ticket only for POST /work-items.
 * The main window keeps the shell bearer on /api/member.
 */
function appletWorkTarget(
  memberPath: string,
  method: 'GET' | 'POST'
): { runtime: AppletRuntimeDescriptor; path: string } | null {
  const runtime = readAppletRuntime()
  if (!runtime) return null
  const pathOnly = memberPath.split('?')[0]
  const create = method === 'POST' && pathOnly === '/api/member/work-items' && OFFICE_CREATE.has(runtime.appId)
  const process =
    method === 'POST' &&
    (pathOnly === '/api/member/work-items/process-preview' || pathOnly === '/api/member/work-items/process') &&
    PROCESS_WINDOWS.has(runtime.appId)
  const approvers =
    method === 'GET' &&
    pathOnly === '/api/member/work-items/approvers' &&
    (OFFICE_CREATE.has(runtime.appId) || PROCESS_WINDOWS.has(runtime.appId))
  const travel = method === 'GET' && pathOnly === '/api/member/work-items/travel-links' && runtime.appId === 'expense'
  const schedule =
    method === 'POST' && /^\/api\/member\/work-items\/[^/]+\/schedule$/.test(pathOnly) && runtime.appId === 'inbox'
  if (runtime.appId !== 'inbox' && !create && !approvers && !travel && !schedule && !process) return null
  return { runtime, path: memberPath.replace('/api/member', `/api/applet-runtime/${runtime.appId}`) }
}

async function workRequest(
  memberPath: string,
  init: { method: 'GET' | 'POST'; body?: string },
  fetchImpl: typeof fetch
): Promise<unknown> {
  const applet = appletWorkTarget(memberPath, init.method)
  if (!applet) return memberRequest(memberPath, init, fetchImpl)
  return ticketRequest(applet.runtime, applet.path, init, fetchImpl)
}

async function ticketRequest(
  runtime: AppletRuntimeDescriptor,
  path: string,
  init: { method: 'GET' | 'POST'; body?: string },
  fetchImpl: typeof fetch
): Promise<unknown> {
  const response = await fetchImpl(`${runtime.apiBaseUrl}${path}`, {
    method: init.method,
    cache: 'no-store',
    headers: {
      accept: 'application/json',
      authorization: `Bearer ${runtime.appletTicket}`,
      ...(init.body ? { 'content-type': 'application/json' } : {})
    },
    body: init.body
  })
  const text = await response.text()
  let payload: unknown = null
  if (text) {
    try {
      payload = JSON.parse(text) as unknown
    } catch {
      throw new MemberOrgError(text || `HTTP ${response.status}`, response.status)
    }
  }
  if (payload && typeof payload === 'object' && 'code' in payload && 'data' in payload) {
    const record = payload as { code?: unknown; message?: unknown; data?: unknown; success?: unknown }
    const code = typeof record.code === 'number' ? record.code : response.status
    const ok = (code === 0 || (code >= 200 && code < 300)) && record.success !== false
    if (!ok) {
      const message = typeof record.message === 'string' && record.message ? record.message : `API error ${code}`
      throw new MemberOrgError(message, response.status >= 400 ? response.status : code)
    }
    return record.data
  }
  if (payload && typeof payload === 'object' && 'error' in payload && response.status >= 400) {
    const error = (payload as { error?: unknown }).error
    throw new MemberOrgError(typeof error === 'string' && error ? error : `HTTP ${response.status}`, response.status)
  }
  if (response.status >= 400) throw new MemberOrgError(`HTTP ${response.status}`, response.status)
  return payload
}

/** Team selected in the main window. `null` when this page is not an applet. */
export async function loadWorkCenter(fetchImpl: typeof fetch = fetch): Promise<string | null> {
  const runtime = readAppletRuntime()
  if (
    !runtime ||
    (runtime.appId !== 'inbox' && !OFFICE_CREATE.has(runtime.appId) && !PROCESS_WINDOWS.has(runtime.appId))
  )
    return null
  const data = await ticketRequest(runtime, `/api/applet-runtime/${runtime.appId}/center`, { method: 'GET' }, fetchImpl)
  if (!data || typeof data !== 'object') return 'personal'
  const id = (data as { id?: unknown }).id
  return typeof id === 'string' && id.trim() ? id.trim() : 'personal'
}

function asItem(value: unknown): WorkItem | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.id !== 'string' || typeof row.title !== 'string' || typeof row.status !== 'string') return null
  if (typeof row.kind !== 'string' || !WORK_KINDS.has(row.kind as WorkKind)) return null
  if (!WORK_STATUSES.has(row.status as WorkStatus)) return null
  return {
    id: row.id,
    org_id: typeof row.org_id === 'string' ? row.org_id : '',
    org_name: typeof row.org_name === 'string' ? row.org_name : '',
    kind: row.kind as WorkKind,
    title: row.title,
    summary: typeof row.summary === 'string' ? row.summary : '',
    status: row.status as WorkStatus,
    initiator_name: typeof row.initiator_name === 'string' ? row.initiator_name : '',
    current_node: typeof row.current_node === 'string' ? row.current_node : '',
    created_at: typeof row.created_at === 'string' ? row.created_at : '',
    actionable: row.actionable === true,
    approvable: row.approvable === true,
    rejectable: row.rejectable === true,
    cancellable: row.cancellable === true,
    fields:
      row.fields && typeof row.fields === 'object' && !Array.isArray(row.fields)
        ? (row.fields as Record<string, unknown>)
        : {},
    decision_comment: typeof row.decision_comment === 'string' ? row.decision_comment : '',
    assignee_names: Array.isArray(row.assignee_names)
      ? row.assignee_names.filter((name): name is string => typeof name === 'string')
      : [],
    decider_name: typeof row.decider_name === 'string' ? row.decider_name : '',
    decided_at: typeof row.decided_at === 'string' ? row.decided_at : '',
    schedule_retry: row.schedule_retry === true,
    engine: row.engine === true,
    trail: asTrail(row.trail)
  }
}

function asTrail(value: unknown): WorkTrailMark[] {
  if (!Array.isArray(value)) return []
  const marks: WorkTrailMark[] = []
  for (const entry of value) {
    if (!entry || typeof entry !== 'object') continue
    const mark = entry as Record<string, unknown>
    if (typeof mark.action !== 'string' || typeof mark.node_name !== 'string') continue
    marks.push({
      action: mark.action,
      node_name: mark.node_name,
      actor_name: typeof mark.actor_name === 'string' ? mark.actor_name : ''
    })
  }
  return marks
}

function asSummary(value: unknown): WorkSummary {
  if (!value || typeof value !== 'object') return EMPTY_SUMMARY
  const row = value as Record<string, unknown>
  const count = (key: keyof WorkSummary) => (typeof row[key] === 'number' && row[key] >= 0 ? row[key] : 0)
  return { todo: count('todo'), done: count('done'), cc: count('cc'), sent: count('sent') }
}

/** The shell replaces this value with the signed-in member's team. */
function scopePath(memberPath: string): string {
  if (readAppletRuntime()) return memberPath
  const join = memberPath.includes('?') ? '&' : '?'
  return `${memberPath}${join}org=${encodeURIComponent(readCenterId())}`
}

export async function loadWorkSummary(fetchImpl: typeof fetch = fetch): Promise<WorkSummary> {
  try {
    return asSummary(await workRequest(scopePath('/api/member/work-items/summary'), { method: 'GET' }, fetchImpl))
  } catch {
    return EMPTY_SUMMARY
  }
}

export async function loadWorkItems(query: WorkListQuery, fetchImpl: typeof fetch = fetch): Promise<WorkItem[]> {
  const params = new URLSearchParams({ box: query.box })
  if (query.kind) params.set('kind', query.kind)
  if (query.status) params.set('status', query.status)
  if (query.q?.trim()) params.set('q', query.q.trim())
  if (query.days) params.set('days', String(query.days))
  if (query.department?.trim()) params.set('department', query.department.trim())
  const data = await workRequest(scopePath(`/api/member/work-items?${params.toString()}`), { method: 'GET' }, fetchImpl)
  return Array.isArray(data) ? data.map(asItem).filter((row): row is WorkItem => row !== null) : []
}

export async function previewWorkProcess(
  input: { orgId: string; processKey: string; amount?: string; form: Record<string, string> },
  fetchImpl: typeof fetch = fetch
): Promise<WorkPreview> {
  const data = await workRequest(
    '/api/member/work-items/process-preview',
    {
      method: 'POST',
      body: JSON.stringify({
        org_id: input.orgId,
        process_key: input.processKey,
        amount: input.amount ?? '',
        form: input.form
      })
    },
    fetchImpl
  )
  if (!data || typeof data !== 'object' || !Array.isArray((data as { nodes?: unknown }).nodes)) {
    throw new Error('invalid preview')
  }
  const row = data as { version?: unknown; nodes: unknown[] }
  return {
    version: typeof row.version === 'number' ? row.version : 0,
    nodes: row.nodes.flatMap((entry) => {
      if (!entry || typeof entry !== 'object') return []
      const node = entry as { key?: unknown; name?: unknown; assignees?: unknown }
      if (typeof node.name !== 'string') return []
      return [
        {
          key: typeof node.key === 'string' ? node.key : '',
          name: node.name,
          assignees: Array.isArray(node.assignees)
            ? node.assignees.filter((id): id is string => typeof id === 'string')
            : []
        }
      ]
    })
  }
}

export async function startWorkProcess(
  input: {
    orgId: string
    idempotencyKey: string
    processKey: string
    amount?: string
    form: Record<string, string>
  },
  fetchImpl: typeof fetch = fetch
): Promise<WorkItem> {
  const data = await workRequest(
    '/api/member/work-items/process',
    {
      method: 'POST',
      body: JSON.stringify({
        org_id: input.orgId,
        idempotency_key: input.idempotencyKey,
        process_key: input.processKey,
        amount: input.amount ?? '',
        form: input.form
      })
    },
    fetchImpl
  )
  const item = asItem(data)
  if (!item) throw new Error('invalid work item')
  return item
}

export async function createWorkItem(
  input: {
    orgId: string
    kind?: WorkCreateKind
    idempotencyKey: string
    fields: Record<string, string>
  },
  fetchImpl: typeof fetch = fetch
): Promise<WorkItem> {
  const data = await workRequest(
    '/api/member/work-items',
    {
      method: 'POST',
      body: JSON.stringify({
        org_id: input.orgId,
        kind: input.kind ?? 'approval',
        idempotency_key: input.idempotencyKey,
        fields: input.fields
      })
    },
    fetchImpl
  )
  const item = asItem(data)
  if (!item) throw new Error('invalid work item')
  return item
}

export async function loadApprovers(fetchImpl: typeof fetch = fetch): Promise<WorkApprover[]> {
  const data = await workRequest(scopePath('/api/member/work-items/approvers'), { method: 'GET' }, fetchImpl)
  if (!Array.isArray(data)) return []
  return data.flatMap((row) => {
    if (!row || typeof row !== 'object') return []
    const record = row as { id?: unknown; name?: unknown }
    if (typeof record.id !== 'string' || typeof record.name !== 'string' || !record.name.trim()) return []
    return [{ id: record.id, name: record.name }]
  })
}

export async function loadTravelLinks(fetchImpl: typeof fetch = fetch): Promise<WorkItem[]> {
  const data = await workRequest(scopePath('/api/member/work-items/travel-links'), { method: 'GET' }, fetchImpl)
  return Array.isArray(data) ? data.map(asItem).filter((row): row is WorkItem => row !== null) : []
}

export async function retryMeetingSchedule(id: string, fetchImpl: typeof fetch = fetch): Promise<WorkItem> {
  const data = await workRequest(
    `/api/member/work-items/${encodeURIComponent(id)}/schedule`,
    { method: 'POST' },
    fetchImpl
  )
  const item = asItem(data)
  if (!item) throw new Error('invalid work item')
  return item
}

export async function decideWorkItem(
  id: string,
  decision: WorkDecision,
  comment: string,
  fetchImpl: typeof fetch = fetch
): Promise<WorkItem> {
  const data = await workRequest(
    `/api/member/work-items/${encodeURIComponent(id)}/decision`,
    { method: 'POST', body: JSON.stringify({ decision, comment }) },
    fetchImpl
  )
  const item = asItem(data)
  if (!item) throw new Error('invalid work item')
  return item
}

export const WORK_INBOX_CHANGED = 'a3s-work-inbox-changed'

export function notifyWorkInboxChanged(): void {
  window.dispatchEvent(new Event(WORK_INBOX_CHANGED))
}
