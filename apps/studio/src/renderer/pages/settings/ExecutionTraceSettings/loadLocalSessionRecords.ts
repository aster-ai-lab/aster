import { dataApiService } from '@renderer/data/DataApiService'

import {
  TRACE_SIGNAL_REVISION,
  type SessionTrace,
  type TraceBook,
  type TraceMessage,
  type TraceSessionSource
} from './extractExecutionTrace'

const PAGE_LIMIT = 200
const PAGE_CAP = 20

interface Page<T> {
  items?: T[]
  nextCursor?: string
}

interface SessionRow {
  id: string
  name?: string
  updatedAt?: string
  lastActivityAt?: string
  workspace?: { path?: string }
}

interface MessageRow {
  id: string
  role?: string
  status?: string
  createdAt: string
  updatedAt: string
  data?: { parts?: unknown[] }
  stats?: unknown
}

/**
 * Reads local agent-plane transcripts for the trajectory projector.
 * This does not start an agent turn.
 * Message downloads are skipped only when the book already stores a watermark
 * and the listed session time still covers that watermark.
 */
export async function loadLocalSessionRecords(book?: TraceBook): Promise<TraceSessionSource[]> {
  const sessions = await collect<SessionRow>((cursor) =>
    dataApiService.get('/agent-sessions', { query: { limit: PAGE_LIMIT, ...(cursor ? { cursor } : {}) } })
  )
  const prior = new Map((book?.sessions ?? []).map((session) => [session.sessionId, session]))
  const records: TraceSessionSource[] = []
  for (let index = 0; index < sessions.length; index += 4) {
    const batch = sessions.slice(index, index + 4)
    const loaded = await Promise.all(batch.map((session) => loadSession(session, book, prior.get(session.id))))
    records.push(...loaded)
  }
  return records
}

/** True when a message download would repeat a read the book already covers. */
export function canSkipSessionMessages(
  book: TraceBook | undefined,
  listedUpdatedAt: string,
  prior: SessionTrace | undefined
): boolean {
  if (!book || book.signalRevision !== TRACE_SIGNAL_REVISION || !prior) return false
  const watermark = prior.messageWatermark
  if (!watermark || prior.messageCount === undefined) return false
  if (!listedUpdatedAt || prior.updatedAt !== listedUpdatedAt) return false
  return listedUpdatedAt >= watermark
}

async function loadSession(
  session: SessionRow,
  book: TraceBook | undefined,
  prior: SessionTrace | undefined
): Promise<TraceSessionSource> {
  const updatedAt = session.updatedAt || session.lastActivityAt || ''
  const workspacePath = session.workspace?.path?.trim()
  if (canSkipSessionMessages(book, updatedAt, prior)) {
    return {
      id: session.id,
      name: session.name ?? '',
      updatedAt,
      workspacePath: workspacePath || undefined,
      messages: [],
      unchanged: true,
      messageCount: prior?.messageCount ?? 0
    }
  }
  const messages = await collect<MessageRow>((cursor) =>
    dataApiService.get(`/agent-sessions/${session.id}/messages` as '/agent-sessions/:sessionId/messages', {
      query: { limit: PAGE_LIMIT, ...(cursor ? { cursor } : {}) }
    })
  )
  return {
    id: session.id,
    name: session.name ?? '',
    updatedAt,
    workspacePath: workspacePath || undefined,
    messages: messages.map(toTraceMessage)
  }
}

function toTraceMessage(row: MessageRow): TraceMessage {
  const parts = row.data?.parts
  return {
    id: row.id,
    role: row.role?.trim() || undefined,
    status: row.status?.trim() || undefined,
    createdAt: row.createdAt,
    updatedAt: row.updatedAt,
    parts: Array.isArray(parts) ? parts : [],
    stats: row.stats && typeof row.stats === 'object' ? row.stats : undefined
  }
}

async function collect<T>(load: (cursor?: string) => Promise<Page<T>>): Promise<T[]> {
  const items: T[] = []
  let cursor: string | undefined
  for (let page = 0; page < PAGE_CAP; page += 1) {
    const response = await load(cursor)
    if (Array.isArray(response.items)) items.push(...response.items)
    if (!response.nextCursor || response.nextCursor === cursor) break
    cursor = response.nextCursor
  }
  return items
}
