/**
 * Org IM client over the Studio shell's loopback lane `/api/im/{org}/…`.
 * The page only holds the runtime token; the shell attaches the member session upstream.
 */
import { readStudioRuntime } from '@shared/studioRuntime'

export type ConversationKind = 'direct' | 'group' | 'system' | 'product' | 'transfer' | 'boss'
export type BodyKind = 'text' | 'file' | 'image' | 'voice' | 'system'

export type ImSender = { kind: 'member' | 'service'; id: string }

export type ImAttachment = {
  name: string
  sizeBytes: number
  objectKey: string
  mime: string | null
}

export type ImBody =
  | { kind: 'text'; text: string }
  | { kind: 'file' | 'image'; attachment: ImAttachment }
  | { kind: 'voice'; attachment: ImAttachment; durationSecs: number }
  | { kind: 'system'; code: string; params: Record<string, string> }
  /** A recalled message keeps only its kind. */
  | { kind: 'recalled'; original: BodyKind }

export type ImMessage = {
  id: string
  conversationId: string
  sequence: number
  sender: ImSender
  body: ImBody
  recalled: boolean
  createdAt: number
}

export type ImPreview = {
  kind: BodyKind
  messageId: string
  sequence: number
  recalled: boolean
  createdAt: number
  sender: ImSender | null
  text: string | null
  name: string | null
  code: string | null
  params: Record<string, string>
}

export type ImInboxEntry = {
  id: string
  kind: ConversationKind
  title: string | null
  peerUserId: string | null
  memberCount: number
  lastSequence: number
  lastReadSequence: number
  unread: number
  preview: ImPreview | null
  muted: boolean
  pinned: boolean
  /** Personal alias. Empty means the real title is shown. */
  remark?: string | null
  updatedAt: number
}

export type ImMember = {
  userId: string
  role: 'owner' | 'admin' | 'member'
  left: boolean
  /** Name shown inside this group. Empty falls back to the directory name. */
  nickname?: string | null
  speechBanned?: boolean
  lastReadSequence: number
}

export type ImConversationDetail = {
  id: string
  kind: ConversationKind
  title: string | null
  memberCount: number
  muteAll?: boolean
  members: ImMember[]
}

export type ImPerson = {
  userId: string
  displayName: string
  email: string
  departmentId: string
  position: string | null
}

export type ImDepartment = { id: string; parentId: string | null; name: string; sortOrder: number }

export type ImMe = { userId: string; displayName: string }

export type ImSyncResult = { conversationId: string; entry: ImInboxEntry | null; messages: ImMessage[] }

export type ImOutgoing = { kind: 'text'; text: string } | { kind: 'file' | 'image' | 'voice'; attachment: ImAttachment }

export class ImApiError extends Error {
  readonly status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = 'ImApiError'
    this.status = status
  }
}

export const IM_PAGE_LIMIT = 50
export const IM_ATTACHMENT_MAX_BYTES = 20 * 1024 * 1024

export interface ImApi {
  readonly orgId: string
  me(): Promise<ImMe>
  directory(): Promise<ImPerson[]>
  departments(): Promise<ImDepartment[]>
  conversations(): Promise<ImInboxEntry[]>
  detail(conversationId: string): Promise<ImConversationDetail>
  messages(conversationId: string, beforeSequence?: number): Promise<ImMessage[]>
  send(conversationId: string, clientMsgId: string, body: ImOutgoing): Promise<ImMessage>
  recall(messageId: string): Promise<ImMessage>
  markRead(conversationId: string, sequence?: number): Promise<number>
  markUnread(conversationId: string): Promise<number>
  setMuted(conversationId: string, muted: boolean): Promise<void>
  setPinned(conversationId: string, pinned: boolean): Promise<void>
  startDirect(peerUserId: string): Promise<string>
  /** The member's private file transfer conversation; created on first call. */
  fileTransfer(): Promise<string>
  createGroup(title: string, memberUserIds: string[]): Promise<string>
  addMembers(conversationId: string, userIds: string[]): Promise<void>
  removeMember(conversationId: string, userId: string): Promise<void>
  leave(conversationId: string): Promise<void>
  /** Owner only. Every current member leaves, and the group leaves every inbox. */
  dissolve(conversationId: string): Promise<void>
  rename(conversationId: string, title: string): Promise<void>
  /** Personal alias for this conversation. An empty string clears it. */
  setRemark(conversationId: string, remark: string): Promise<void>
  /** The caller's display name inside one group. An empty string clears it. */
  setNickname(conversationId: string, nickname: string): Promise<void>
  /** Mute every ordinary member. Owners and admins can still send. */
  setMuteAll(conversationId: string, muteAll: boolean): Promise<void>
  /** Mute or unmute one member. The owner cannot be muted. */
  setMemberSpeech(conversationId: string, userId: string, banned: boolean): Promise<void>
  transferOwner(conversationId: string, userId: string): Promise<void>
  setMemberRole(conversationId: string, userId: string, role: 'admin' | 'member'): Promise<void>
  typing(conversationId: string): Promise<void>
  /** False when the presence cache is down. Callers hide presence instead of guessing. */
  renewPresence(): Promise<boolean>
  presence(userIds: string[]): Promise<string[]>
  sync(cursors: Array<{ conversationId: string; sequence: number }>): Promise<ImSyncResult[]>
  upload(file: Blob, name: string): Promise<ImAttachment>
  download(messageId: string): Promise<Blob>
  /** Opens the shell-relayed event stream; the shell fetches the ticket. */
  stream(deviceId: string, signal: AbortSignal): Promise<Response>
}

type Runtime = { apiBaseUrl: string; token: string }

function runtimeOrThrow(): Runtime {
  let runtime: Runtime | undefined
  try {
    runtime = readStudioRuntime()
  } catch {
    runtime = undefined
  }
  if (!runtime) throw new ImApiError('runtime', 0)
  return runtime
}

export function imRuntimeAvailable(): boolean {
  try {
    return Boolean(readStudioRuntime())
  } catch {
    return false
  }
}

export function createImApi(orgId: string, fetchImpl: typeof fetch = (input, init) => fetch(input, init)): ImApi {
  const base = () => `${runtimeOrThrow().apiBaseUrl}/api/im/${encodeURIComponent(orgId)}`
  const auth = () => ({ authorization: `Bearer ${runtimeOrThrow().token}` })

  const call = async (method: 'GET' | 'POST' | 'PUT' | 'DELETE', path: string, body?: unknown): Promise<unknown> => {
    const sendsBody = body !== undefined
    let response: Response
    try {
      response = await fetchImpl(`${base()}${path}`, {
        method,
        cache: 'no-store',
        headers: {
          accept: 'application/json',
          ...auth(),
          ...(sendsBody ? { 'content-type': 'application/json' } : {})
        },
        body: sendsBody ? JSON.stringify(body) : undefined
      })
    } catch (error) {
      if (error instanceof ImApiError) throw error
      throw new ImApiError('network', 0)
    }
    return unwrap(response)
  }

  return {
    orgId,
    async me() {
      const row = record(await call('GET', '/me'))
      return { userId: str(row.user_id), displayName: str(row.display_name) }
    },
    async directory() {
      return list(await call('GET', '/directory')).map((value) => {
        const row = record(value)
        return {
          userId: str(row.user_id),
          displayName: str(row.display_name),
          email: str(row.email),
          departmentId: str(row.department_id),
          position: optStr(row.position)
        }
      })
    },
    async departments() {
      return list(await call('GET', '/departments')).map((value) => {
        const row = record(value)
        return { id: str(row.id), parentId: optStr(row.parent_id), name: str(row.name), sortOrder: num(row.sort_order) }
      })
    },
    async conversations() {
      return list(await call('GET', '/conversations?limit=100')).map(toEntry)
    },
    async detail(conversationId) {
      return toDetail(await call('GET', `/conversations/${conversationId}`))
    },
    async messages(conversationId, beforeSequence) {
      const query = beforeSequence
        ? `?before_sequence=${beforeSequence}&limit=${IM_PAGE_LIMIT}`
        : `?limit=${IM_PAGE_LIMIT}`
      return list(await call('GET', `/conversations/${conversationId}/messages${query}`)).map(toMessage)
    },
    async send(conversationId, clientMsgId, body) {
      const payload =
        body.kind === 'text'
          ? { client_msg_id: clientMsgId, kind: 'text', text: body.text }
          : {
              client_msg_id: clientMsgId,
              kind: body.kind,
              name: body.attachment.name,
              size_bytes: body.attachment.sizeBytes,
              object_key: body.attachment.objectKey,
              mime: body.attachment.mime ?? undefined
            }
      const row = record(await call('POST', `/conversations/${conversationId}/messages`, payload))
      return toMessage(row.message)
    },
    async recall(messageId) {
      return toMessage(await call('DELETE', `/messages/${messageId}`))
    },
    async markRead(conversationId, sequence) {
      const row = record(
        await call('PUT', `/conversations/${conversationId}/read-cursor`, sequence ? { sequence } : {})
      )
      return num(row.last_read_sequence)
    },
    async markUnread(conversationId) {
      return num(record(await call('PUT', `/conversations/${conversationId}/unread-cursor`, {})).last_read_sequence)
    },
    async setMuted(conversationId, muted) {
      await call('PUT', `/conversations/${conversationId}/muted`, { muted })
    },
    async setPinned(conversationId, pinned) {
      await call('PUT', `/conversations/${conversationId}/pinned`, { pinned })
    },
    async startDirect(peerUserId) {
      const row = record(await call('POST', '/direct-conversations', { peer_user_id: peerUserId }))
      return str(record(row.conversation).id)
    },
    async fileTransfer() {
      return str(record(await call('PUT', '/file-transfer')).id)
    },
    async createGroup(title, memberUserIds) {
      return str(record(await call('POST', '/group-conversations', { title, member_user_ids: memberUserIds })).id)
    },
    async addMembers(conversationId, userIds) {
      await call('POST', `/conversations/${conversationId}/members`, { user_ids: userIds })
    },
    async removeMember(conversationId, userId) {
      await call('DELETE', `/conversations/${conversationId}/members/${userId}`)
    },
    async leave(conversationId) {
      await call('DELETE', `/conversations/${conversationId}/membership`)
    },
    async dissolve(conversationId) {
      await call('DELETE', `/conversations/${conversationId}`)
    },
    async rename(conversationId, title) {
      await call('PUT', `/conversations/${conversationId}`, { title })
    },
    async setRemark(conversationId, remark) {
      await call('PUT', `/conversations/${conversationId}/remark`, { remark })
    },
    async setNickname(conversationId, nickname) {
      await call('PUT', `/conversations/${conversationId}/nickname`, { nickname })
    },
    async setMuteAll(conversationId, muteAll) {
      await call('PUT', `/conversations/${conversationId}/speech`, { mute_all: muteAll })
    },
    async setMemberSpeech(conversationId, userId, banned) {
      await call('PUT', `/conversations/${conversationId}/members/${userId}/speech`, { banned })
    },
    async transferOwner(conversationId, userId) {
      await call('PUT', `/conversations/${conversationId}/owner`, { user_id: userId })
    },
    async setMemberRole(conversationId, userId, role) {
      await call('PUT', `/conversations/${conversationId}/members/${userId}/role`, { role })
    },
    async typing(conversationId) {
      await call('POST', `/conversations/${conversationId}/typing`, {})
    },
    async renewPresence() {
      return record(await call('PUT', '/presence', {})).renewed === true
    },
    async presence(userIds) {
      const query = userIds.length ? `?user_ids=${userIds.map((id) => encodeURIComponent(id)).join(',')}` : ''
      return list(await call('GET', `/presence${query}`)).map((value) => str(value))
    },
    async sync(cursors) {
      const rows = list(
        await call('POST', '/message-syncs', {
          cursors: cursors.map((cursor) => ({ conversation_id: cursor.conversationId, sequence: cursor.sequence }))
        })
      )
      return rows.map((value) => {
        const row = record(value)
        return {
          conversationId: str(row.conversation_id),
          entry: row.entry ? toEntry(row.entry) : null,
          messages: list(row.messages).map(toMessage)
        }
      })
    },
    async upload(file, name) {
      const mime = file.type || 'application/octet-stream'
      const query = `?name=${encodeURIComponent(name)}&mime=${encodeURIComponent(mime)}`
      let response: Response
      try {
        response = await fetchImpl(`${base()}/attachments${query}`, {
          method: 'POST',
          cache: 'no-store',
          headers: { accept: 'application/json', ...auth(), 'content-type': 'application/octet-stream' },
          body: file
        })
      } catch (error) {
        if (error instanceof ImApiError) throw error
        throw new ImApiError('network', 0)
      }
      const row = record(await unwrap(response))
      return {
        name: str(row.name),
        sizeBytes: num(row.size_bytes),
        objectKey: str(row.object_key),
        mime: optStr(row.mime)
      }
    },
    async download(messageId) {
      let response: Response
      try {
        response = await fetchImpl(`${base()}/attachments/${messageId}`, {
          method: 'GET',
          cache: 'no-store',
          headers: { accept: '*/*', ...auth() }
        })
      } catch (error) {
        if (error instanceof ImApiError) throw error
        throw new ImApiError('network', 0)
      }
      if (!response.ok) await unwrap(response)
      return response.blob()
    },
    async stream(deviceId, signal) {
      const response = await fetchImpl(`${base()}/stream?device=${encodeURIComponent(deviceId)}`, {
        method: 'GET',
        cache: 'no-store',
        headers: { accept: 'text/event-stream', ...auth() },
        signal
      })
      if (!response.ok) await unwrap(response)
      if (!response.body) throw new ImApiError('stream', response.status)
      return response
    }
  }
}

async function unwrap(response: Response): Promise<unknown> {
  const text = await response.text()
  let payload: unknown = null
  if (text) {
    try {
      payload = JSON.parse(text)
    } catch {
      throw new ImApiError(text || `HTTP ${response.status}`, response.status)
    }
  }
  if (payload && typeof payload === 'object' && 'code' in payload && 'data' in payload) {
    const envelope = payload as { code?: unknown; message?: unknown; data?: unknown; success?: unknown }
    const code = typeof envelope.code === 'number' ? envelope.code : response.status
    const ok = response.ok && (code === 0 || (code >= 200 && code < 300)) && envelope.success !== false
    if (!ok) {
      const message =
        typeof envelope.message === 'string' && envelope.message ? envelope.message : `HTTP ${response.status}`
      throw new ImApiError(message, response.status >= 400 ? response.status : code)
    }
    return envelope.data
  }
  if (!response.ok) {
    const message =
      payload && typeof payload === 'object' && 'error' in payload ? String(payload.error) : `HTTP ${response.status}`
    throw new ImApiError(message, response.status)
  }
  return payload
}

function record(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : {}
}

function list(value: unknown): unknown[] {
  return Array.isArray(value) ? value : []
}

function str(value: unknown): string {
  return typeof value === 'string' ? value : ''
}

function optStr(value: unknown): string | null {
  return typeof value === 'string' && value ? value : null
}

function num(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : 0
}

function time(value: unknown): number {
  const parsed = typeof value === 'string' ? Date.parse(value) : Number.NaN
  return Number.isFinite(parsed) ? parsed : 0
}

function stringMap(value: unknown): Record<string, string> {
  const out: Record<string, string> = {}
  for (const [key, item] of Object.entries(record(value))) {
    if (typeof item === 'string') out[key] = item
  }
  return out
}

function bodyKind(value: unknown): BodyKind {
  return value === 'file' || value === 'image' || value === 'voice' || value === 'system' ? value : 'text'
}

function toSender(value: unknown): ImSender | null {
  const row = record(value)
  if (row.kind !== 'member' && row.kind !== 'service') return null
  return { kind: row.kind, id: str(row.id) }
}

function toAttachment(row: Record<string, unknown>): ImAttachment {
  return { name: str(row.name), sizeBytes: num(row.size_bytes), objectKey: str(row.object_key), mime: optStr(row.mime) }
}

export function toBody(value: unknown, recalled: boolean): ImBody {
  const row = record(value)
  const kind = bodyKind(row.kind)
  if (recalled) return { kind: 'recalled', original: kind }
  switch (kind) {
    case 'file':
    case 'image':
      return { kind, attachment: toAttachment(row) }
    case 'voice':
      return { kind, attachment: toAttachment(row), durationSecs: num(row.duration_secs) }
    case 'system':
      return { kind, code: str(row.code), params: stringMap(row.params) }
    default:
      return { kind: 'text', text: str(row.text) }
  }
}

export function toMessage(value: unknown): ImMessage {
  const row = record(value)
  const recalled = row.recalled === true
  return {
    id: str(row.id),
    conversationId: str(row.conversation_id),
    sequence: num(row.sequence),
    sender: toSender(row.sender) ?? { kind: 'service', id: '' },
    body: toBody(row.body, recalled),
    recalled,
    createdAt: time(row.created_at)
  }
}

function conversationKind(value: unknown): ConversationKind {
  return value === 'group' || value === 'system' || value === 'product' || value === 'transfer' ? value : 'direct'
}

function toPreview(value: unknown): ImPreview | null {
  const row = record(value)
  if (!row.message_id) return null
  return {
    kind: bodyKind(row.kind),
    messageId: str(row.message_id),
    sequence: num(row.sequence),
    recalled: row.recalled === true,
    createdAt: time(row.created_at),
    sender: toSender(row.sender),
    text: typeof row.text === 'string' ? row.text : null,
    name: optStr(row.name),
    code: optStr(row.code),
    params: stringMap(row.params)
  }
}

export function toEntry(value: unknown): ImInboxEntry {
  const row = record(value)
  return {
    id: str(row.conversation_id),
    kind: conversationKind(row.kind),
    title: optStr(row.title),
    peerUserId: optStr(row.peer_user_id),
    memberCount: num(row.member_count),
    lastSequence: num(row.last_sequence),
    lastReadSequence: num(row.last_read_sequence),
    unread: num(row.unread),
    preview: toPreview(row.preview),
    muted: row.muted === true,
    pinned: row.pinned === true,
    remark: optStr(row.remark),
    updatedAt: time(row.updated_at)
  }
}

function toDetail(value: unknown): ImConversationDetail {
  const row = record(value)
  return {
    id: str(row.id),
    kind: conversationKind(row.kind),
    title: optStr(row.title),
    memberCount: num(row.member_count),
    muteAll: row.mute_all === true,
    members: list(row.members).map((item) => {
      const member = record(item)
      const role = member.role === 'owner' || member.role === 'admin' ? member.role : 'member'
      return {
        userId: str(member.user_id),
        role,
        left: member.left === true,
        nickname: optStr(member.nickname),
        speechBanned: member.speech_banned === true,
        lastReadSequence: num(member.last_read_sequence)
      }
    })
  }
}
