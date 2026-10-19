import { ipcApi } from '@renderer/ipc'
import type { AgentFormChunkPayload, StreamChunkPayload } from '@shared/ai/transport'
import {
  AGENT_FORM_SPEC_MAX_BYTES,
  agentFormBindingSchema,
  agentFormPartSchema,
  agentFormValuesSchema
} from '@shared/types/agentForm'

export const TOBOT_TOPIC = 'agent-session:tobot'

const TOBOT_SESSION = 'tobot'

/** Topic for one member's ToBot session. An empty id stays on the legacy topic. */
export function tobotTopic(sessionId: string): string {
  const id = sessionId.trim()
  if (!id || id.includes('/') || id.includes('\\') || id.includes('..')) return TOBOT_TOPIC
  return `agent-session:${id}`
}

let boundSessionId = TOBOT_SESSION
const sessionWatchers = new Set<() => void>()

export function currentTobotSession(): string {
  return boundSessionId
}

/** The messages page binds the session `ensure` returned. Other listeners follow it. */
export function bindTobotSession(sessionId: string): void {
  const id = sessionId.trim()
  if (!id || id === boundSessionId) return
  boundSessionId = id
  for (const watcher of sessionWatchers) watcher()
}

export function subscribeTobotSession(listener: () => void): () => void {
  sessionWatchers.add(listener)
  return () => {
    sessionWatchers.delete(listener)
  }
}

export function sessionIdFromEnsure(ready: unknown): string | null {
  if (!ready || typeof ready !== 'object') return null
  const id = (ready as { sessionId?: unknown }).sessionId
  if (typeof id !== 'string') return null
  const trimmed = id.trim()
  if (!trimmed || trimmed.includes('/') || trimmed.includes('\\') || trimmed.includes('..')) return null
  return trimmed
}

export type TobotPart = {
  type: string
  text?: string
  data?: Record<string, unknown>
  state?: string
  toolName?: string
  toolCallId?: string
  input?: unknown
  output?: unknown
  errorText?: string
  approval?: { id: string }
}

export type TobotLive = { anchorId: string; text: string; parts: TobotPart[] }

export type TobotApproval = { approvalId: string; toolName: string }

const STREAM_PARTS = new Set([
  'text-delta',
  'text-end',
  'reasoning-delta',
  'reasoning-end',
  'tool-input-start',
  'tool-input-available',
  'tool-output-available',
  'tool-output-error',
  'tool-approval-request',
  'form-request',
  'form-answered',
  'data-form'
])

function textOf(parts: readonly TobotPart[]): string {
  return parts
    .filter((part) => part.type === 'text' && part.text)
    .map((part) => part.text)
    .join('')
}

function toolIndex(parts: readonly TobotPart[], toolCallId: string): number {
  return parts.findIndex((part) => part.type === 'dynamic-tool' && part.toolCallId === toolCallId)
}

/** Fold one AI SDK chunk into the live turn. A chunk for another topic is ignored. */
function foldChunk(live: TobotLive, chunk: Record<string, unknown>): TobotLive | null {
  if (chunk.type === 'data-form') {
    const parsed = agentFormPartSchema.safeParse(chunk.data)
    if (!parsed.success || chunk.id !== parsed.data.id) return null
    const data = parsed.data
    if (
      data.state === 'answered' &&
      live.parts.some((part) => part.type === 'data-form' && part.data?.id === data.id && part.data.spec !== data.spec)
    )
      return null
    return foldChunk(live, {
      type: data.state === 'answered' ? 'form-answered' : 'form-request',
      formId: data.id,
      format: data.format,
      spec: data.spec,
      binding: data.binding,
      ...(data.state === 'answered' ? { values: data.values } : {})
    })
  }
  const type = typeof chunk.type === 'string' ? chunk.type : ''
  const parts = live.parts.slice()
  if (type === 'text-delta' || type === 'reasoning-delta') {
    const delta = typeof chunk.delta === 'string' ? chunk.delta : ''
    if (!delta) return null
    const kind = type === 'text-delta' ? 'text' : 'reasoning'
    const open = parts.findIndex((part) => part.type === kind && part.state === 'streaming')
    if (open < 0) parts.push({ type: kind, text: delta, state: 'streaming' })
    else {
      const part = parts[open]
      parts[open] = { ...part, text: `${part?.text ?? ''}${delta}` }
    }
  } else if (type === 'text-end' || type === 'reasoning-end') {
    const kind = type === 'text-end' ? 'text' : 'reasoning'
    const open = parts.findIndex((part) => part.type === kind && part.state === 'streaming')
    if (open < 0) return null
    const part = parts[open]
    if (!part) return null
    parts[open] = { ...part, state: 'done' }
  } else if (type === 'tool-input-start' || type === 'tool-input-available') {
    const toolCallId = typeof chunk.toolCallId === 'string' ? chunk.toolCallId : ''
    const toolName = typeof chunk.toolName === 'string' ? chunk.toolName : ''
    if (!toolCallId) return null
    const index = toolIndex(parts, toolCallId)
    const input = type === 'tool-input-available' ? chunk.input : undefined
    if (index < 0) {
      parts.push({
        type: 'dynamic-tool',
        toolName: toolName || 'tool',
        toolCallId,
        state: 'input-available',
        ...(input !== undefined ? { input } : {})
      })
    } else {
      const part = parts[index]
      if (!part) return null
      parts[index] = {
        ...part,
        toolName: toolName || part.toolName,
        ...(input !== undefined ? { input } : {})
      }
    }
  } else if (type === 'tool-output-available' || type === 'tool-output-error') {
    const toolCallId = typeof chunk.toolCallId === 'string' ? chunk.toolCallId : ''
    const index = toolIndex(parts, toolCallId)
    if (index < 0) return null
    const part = parts[index]
    if (!part) return null
    parts[index] =
      type === 'tool-output-available'
        ? { ...part, state: 'output-available', output: chunk.output }
        : {
            ...part,
            state: 'output-error',
            errorText: typeof chunk.errorText === 'string' ? chunk.errorText : 'Tool failed'
          }
  } else if (type === 'tool-approval-request') {
    const toolCallId = typeof chunk.toolCallId === 'string' ? chunk.toolCallId : ''
    const approvalId = typeof chunk.approvalId === 'string' ? chunk.approvalId : ''
    const index = toolIndex(parts, toolCallId)
    if (index < 0 || !approvalId) return null
    const part = parts[index]
    if (!part) return null
    parts[index] = { ...part, state: 'approval-requested', approval: { id: approvalId } }
  } else if (type === 'form-request' || type === 'form-answered') {
    const formId = typeof chunk.formId === 'string' ? chunk.formId : ''
    if (!formId) return null
    const binding = chunk.binding === undefined ? undefined : agentFormBindingSchema.safeParse(chunk.binding)
    if (binding && (!binding.success || binding.data.requestId !== formId)) return null
    const existing = parts.findIndex(
      (part) => part.type === 'data-form' && (part as { data?: { id?: string } }).data?.id === formId
    )
    if (type === 'form-answered') {
      if (existing < 0) return null
      const part = parts[existing]
      if (!part?.data || part.data.state === 'answered') return null
      const current = agentFormBindingSchema.safeParse(part.data.binding)
      if (current.success && (!binding?.success || JSON.stringify(current.data) !== JSON.stringify(binding.data)))
        return null
      const values = agentFormValuesSchema.safeParse(chunk.values)
      if (!values.success) return null
      parts[existing] = {
        ...part,
        data: { ...part.data, state: 'answered', values: values.data }
      }
    } else {
      const spec = typeof chunk.spec === 'string' ? chunk.spec : ''
      if (!spec || new globalThis.TextEncoder().encode(spec).byteLength > AGENT_FORM_SPEC_MAX_BYTES) return null
      if (binding && chunk.format !== 'openui') return null
      if (existing >= 0 && parts[existing]?.data?.binding !== undefined) return null
      const data = {
        id: formId,
        ...(chunk.format === 'openui' ? { format: 'openui' } : {}),
        spec,
        ...(binding?.success ? { binding: binding.data } : {}),
        state: 'form-requested'
      }
      if (existing >= 0) {
        parts[existing] = { type: 'data-form', data }
      } else {
        parts.push({ type: 'data-form', data })
      }
    }
  } else {
    return null
  }
  return { anchorId: live.anchorId, parts, text: textOf(parts) }
}

type ChunkFields = { type?: string; toolCallId?: string; toolName?: string; approvalId?: string; formId?: string }

/** Remember a tool name so the later approval card can name the same call. */
export function noteTobotTool(names: Map<string, string>, payload: StreamChunkPayload, topicId = TOBOT_TOPIC): void {
  if (payload.topicId !== topicId) return
  const chunk = payload.chunk as ChunkFields
  if (chunk.type !== 'tool-input-available' && chunk.type !== 'tool-input-start') return
  if (typeof chunk.toolCallId !== 'string' || typeof chunk.toolName !== 'string' || !chunk.toolName) return
  names.set(chunk.toolCallId, chunk.toolName)
}

/** The approval the thread must answer. `null` when this chunk is not that request. */
export function approvalFromChunk(
  payload: StreamChunkPayload,
  names: ReadonlyMap<string, string>,
  topicId = TOBOT_TOPIC
): TobotApproval | null {
  if (payload.topicId !== topicId) return null
  const chunk = payload.chunk as ChunkFields
  if (chunk.type !== 'tool-approval-request' || typeof chunk.approvalId !== 'string' || !chunk.approvalId) return null
  const toolName = typeof chunk.toolCallId === 'string' ? (names.get(chunk.toolCallId) ?? '') : ''
  return { approvalId: chunk.approvalId, toolName }
}

/** Approval still waiting on the newest pending assistant. A finished row does not count. */
export function approvalFromLines(
  lines: readonly { role: string; status: string; parts?: readonly TobotPart[] }[]
): TobotApproval | null {
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    const line = lines[index]
    if (!line || line.role !== 'assistant' || line.status !== 'pending') continue
    for (const part of line.parts ?? []) {
      const id = part.approval?.id
      if (part.state === 'approval-requested' && id) {
        return { approvalId: id, toolName: part.toolName ?? '' }
      }
    }
    return null
  }
  return null
}

/** Only the newest pending assistant is the live turn. Older pending rows already stopped. */
export function livePendingId(lines: readonly { id: string; role: string; status: string }[]): string | null {
  let id: string | null = null
  for (const line of lines) {
    if (line.role === 'assistant' && line.status === 'pending') id = line.id
  }
  return id
}

export function applyTobotChunk(
  live: TobotLive | null,
  payload: StreamChunkPayload | AgentFormChunkPayload,
  topicId = TOBOT_TOPIC
): TobotLive | null {
  if (payload.topicId !== topicId) return live
  const anchorId = payload.anchorMessageId
  const chunk = payload.chunk as { type?: string; delta?: string }
  if (!anchorId || !chunk.type || !STREAM_PARTS.has(chunk.type)) return live
  if ((chunk.type === 'text-delta' || chunk.type === 'reasoning-delta') && !chunk.delta) return live
  if (chunk.type === 'form-request' || chunk.type === 'form-answered' || chunk.type === 'data-form') {
    const form = payload.chunk as { binding?: unknown; data?: { binding?: unknown } }
    const binding = chunk.type === 'data-form' ? form.data?.binding : form.binding
    if (binding !== undefined) {
      const parsed = agentFormBindingSchema.safeParse(binding)
      if (
        !parsed.success ||
        parsed.data.owner.anchorMessageId !== anchorId ||
        `agent-session:${parsed.data.owner.sessionId}` !== payload.topicId ||
        parsed.data.owner.executionId !== payload.executionId ||
        parsed.data.owner.attemptId !== payload.attemptId
      )
        return live
    }
  }
  const base: TobotLive = live && live.anchorId === anchorId ? live : { anchorId, text: '', parts: [] }
  return foldChunk(base, payload.chunk) ?? live
}

export function overlayAgentLines<
  T extends { id: string; role: string; text: string; status: string; parts?: TobotPart[] }
>(
  lines: readonly T[],
  live: TobotLive | null,
  pendingAssistant: (id: string, text: string, parts: TobotPart[]) => T
): readonly T[] {
  if (!live || (!live.text && live.parts.length === 0)) return lines
  const index = lines.findIndex((line) => line.id === live.anchorId)
  if (index < 0) return [...lines, pendingAssistant(live.anchorId, live.text, live.parts)]
  const line = lines[index]
  if (!line || line.role !== 'assistant') return lines
  if (line.status !== 'pending' && line.text.length > 0) return lines
  const parts = live.parts.length > 0 ? live.parts : line.parts
  if (line.text === live.text && parts === line.parts) return lines
  const next = lines.slice()
  next[index] = { ...line, text: live.text || line.text, parts }
  return next
}

export function overlayPendingReply<T extends { text: string; status: string; parts?: TobotPart[] }>(
  replies: readonly T[],
  live: TobotLive | null
): readonly T[] {
  if (!live || (!live.text && live.parts.length === 0)) return replies
  let index = -1
  replies.forEach((line, position) => {
    if (line.status === 'pending') index = position
  })
  const line = index >= 0 ? replies[index] : undefined
  if (!line || (line.text === live.text && line.parts === live.parts)) return replies
  const next = replies.slice()
  next[index] = { ...line, text: live.text || line.text, parts: live.parts.length > 0 ? live.parts : line.parts }
  return next
}

/** Text of a ToBot `text-delta`, or `''` when the chunk is not one. */
export function tobotTextDelta(payload: StreamChunkPayload, topicId = TOBOT_TOPIC): string {
  if (payload.topicId !== topicId || payload.chunk.type !== 'text-delta') return ''
  return 'delta' in payload.chunk && typeof payload.chunk.delta === 'string' ? payload.chunk.delta : ''
}

type TobotClient = {
  topicId: string
  onChunk: (payload: StreamChunkPayload) => void
  onSettle: () => void
  replay: boolean
  stopped: boolean
}

let clients: TobotClient[] = []
let offs: Array<() => void> | null = null

function fanout(run: (client: TobotClient) => void): void {
  for (const client of clients.slice()) {
    if (!client.stopped) run(client)
  }
}

function ensureListeners(): void {
  if (offs) return
  offs = [
    ipcApi.on('ai.stream.chunk', (payload) =>
      fanout((client) => {
        if (client.topicId === payload.topicId) client.onChunk(payload)
      })
    ),
    ipcApi.on('ai.stream.done', (payload) => {
      fanout((client) => {
        if (client.topicId === payload.topicId) client.onSettle()
      })
    }),
    ipcApi.on('ai.stream.error', (payload) => {
      fanout((client) => {
        if (client.topicId === payload.topicId) client.onSettle()
      })
    })
  ]
}

function topicStillListened(topicId: string): boolean {
  return clients.some((client) => !client.stopped && client.topicId === topicId)
}

/** Drops a listener a test left attached. Production callers stop themselves. */
export function resetTobotStreamForTests(): void {
  clients = []
  boundSessionId = TOBOT_SESSION
  sessionWatchers.clear()
  if (!offs) return
  for (const off of offs) off()
  offs = null
}

/**
 * Live `ai.stream.chunk` for one ToBot session.
 * `replay` delivers that session's open turn. A chunk for another session is ignored.
 */
export function subscribeTobotStream(
  onChunk: (payload: StreamChunkPayload) => void,
  onSettle: () => void,
  options?: { replay?: boolean; sessionId?: string }
): () => void {
  const topicId = tobotTopic(options?.sessionId ?? TOBOT_SESSION)
  const client: TobotClient = {
    topicId,
    onChunk,
    onSettle,
    replay: options?.replay !== false,
    stopped: false
  }
  clients.push(client)
  ensureListeners()
  const attach = client.replay || clients.filter((item) => !item.stopped && item.topicId === topicId).length === 1
  if (attach) {
    void ipcApi
      .request('ai.stream.attach', { topicId })
      .then((result) => {
        if (client.stopped || !client.replay || result.status !== 'attached') return
        for (const payload of result.bufferedChunks) {
          if (client.stopped || payload.topicId !== topicId) return
          client.onChunk(payload)
        }
      })
      .catch(() => undefined)
  }
  return () => {
    client.stopped = true
    clients = clients.filter((item) => item !== client)
    if (!topicStillListened(topicId)) {
      void ipcApi.request('ai.stream.detach', { topicId }).catch(() => undefined)
    }
    if (clients.length > 0 || !offs) return
    for (const off of offs) off()
    offs = null
  }
}
