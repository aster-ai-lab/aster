import { memberRequest, MemberOrgError, readCenterId, PERSONAL_CENTER } from '@renderer/data/memberOrgs'
import { publishMailToBotPolicy } from '@renderer/data/studioAgentPlane'
import { loadMailToBotPolicy, mailToBotNote } from '@renderer/pages/mail/mailToBot'

export type MailFolder = 'inbox' | 'unread' | 'flagged' | 'drafts' | 'sent' | 'spam' | 'trash'

export type Mailbox = {
  id: string
  address: string
  unread: number
}

export type MailSummary = {
  id: string
  folder: string
  from_address: string
  from_name: string
  to: string[]
  cc: string[]
  subject: string
  preview: string
  unread: boolean
  flagged: boolean
  thread_id: string
  created_at: string
}

export type MailFile = {
  name: string
  mime: string
  data: string
}

export type MailMessage = MailSummary & {
  body: string
  attachments: MailFile[]
  can_update?: boolean
  can_delete?: boolean
}

export type MailDraft = {
  to: string[]
  cc: string[]
  subject: string
  body: string
  draft: boolean
  thread_id?: string
  attachments?: MailFile[]
}

const FOLDERS: readonly MailFolder[] = ['inbox', 'unread', 'flagged', 'drafts', 'sent', 'spam', 'trash']

export function isMailFolder(value: string): value is MailFolder {
  return FOLDERS.includes(value as MailFolder)
}

function asStringList(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value.flatMap((item) => (typeof item === 'string' && item.trim() ? [item.trim()] : []))
}

function asSummary(value: unknown): MailSummary | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.id !== 'string' || !row.id) return null
  if (typeof row.created_at !== 'string' || !row.created_at) return null
  return {
    id: row.id,
    folder: typeof row.folder === 'string' ? row.folder : 'inbox',
    from_address: typeof row.from_address === 'string' ? row.from_address : '',
    from_name: typeof row.from_name === 'string' ? row.from_name : '',
    to: asStringList(row.to),
    cc: asStringList(row.cc),
    subject: typeof row.subject === 'string' ? row.subject : '',
    preview: typeof row.preview === 'string' ? row.preview : '',
    unread: row.unread === true,
    flagged: row.flagged === true,
    thread_id: typeof row.thread_id === 'string' ? row.thread_id : '',
    created_at: row.created_at
  }
}

function asFiles(value: unknown): MailFile[] {
  if (!Array.isArray(value)) return []
  return value.flatMap((item) => {
    if (!item || typeof item !== 'object') return []
    const row = item as Record<string, unknown>
    if (typeof row.name !== 'string' || !row.name || typeof row.data !== 'string' || !row.data) return []
    return [
      { name: row.name, mime: typeof row.mime === 'string' ? row.mime : 'application/octet-stream', data: row.data }
    ]
  })
}

function asMessage(value: unknown): MailMessage | null {
  const summary = asSummary(value)
  if (!summary || !value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  return {
    ...summary,
    body: typeof row.body === 'string' ? row.body : '',
    attachments: asFiles(row.attachments),
    can_update: row.can_update === true,
    can_delete: row.can_delete === true
  }
}

function asMailbox(value: unknown): Mailbox | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.id !== 'string' || !row.id) return null
  if (typeof row.address !== 'string' || !row.address) return null
  const unread = typeof row.unread === 'number' && row.unread > 0 ? Math.floor(row.unread) : 0
  return { id: row.id, address: row.address, unread }
}

function orgQuery(orgId: string): URLSearchParams {
  if (!orgId.trim() || orgId === PERSONAL_CENTER) throw new MemberOrgError('invalid')
  return new URLSearchParams({ org_id: orgId })
}

export async function loadMailbox(orgId: string): Promise<Mailbox> {
  const mailbox = asMailbox(await memberRequest(`/api/mail/mailbox?${orgQuery(orgId)}`, { method: 'GET' }, fetch))
  if (!mailbox) throw new MemberOrgError('invalid')
  return mailbox
}

export async function listMail(
  orgId: string,
  folder: MailFolder,
  query = '',
  limit = 50
): Promise<{ items: MailSummary[]; unread: number }> {
  const params = orgQuery(orgId)
  params.set('folder', folder)
  params.set('limit', String(limit))
  const trimmed = query.trim()
  if (trimmed) params.set('q', trimmed)
  const data = await memberRequest(`/api/mail/messages?${params.toString()}`, { method: 'GET' }, fetch)
  if (!data || typeof data !== 'object') throw new MemberOrgError('invalid')
  const row = data as { items?: unknown; unread?: unknown }
  if (!Array.isArray(row.items)) throw new MemberOrgError('invalid')
  const unread = typeof row.unread === 'number' && row.unread > 0 ? Math.floor(row.unread) : 0
  return {
    items: row.items.flatMap((item) => {
      const summary = asSummary(item)
      return summary ? [summary] : []
    }),
    unread
  }
}

export async function openMail(orgId: string, id: string): Promise<MailMessage> {
  const message = asMessage(
    await memberRequest(`/api/mail/messages/${encodeURIComponent(id)}?${orgQuery(orgId)}`, { method: 'GET' }, fetch)
  )
  if (!message) throw new MemberOrgError('invalid')
  return message
}

export async function sendMail(orgId: string, draft: MailDraft): Promise<MailMessage> {
  orgQuery(orgId)
  const payload: Record<string, unknown> = {
    org_id: orgId,
    to: draft.to,
    cc: draft.cc,
    subject: draft.subject,
    body: draft.body,
    draft: draft.draft
  }
  if (draft.thread_id) payload.thread_id = draft.thread_id
  if (draft.attachments && draft.attachments.length > 0) payload.attachments = draft.attachments
  const message = asMessage(
    await memberRequest('/api/mail/messages', { method: 'POST', body: JSON.stringify(payload) }, fetch)
  )
  if (!message) throw new MemberOrgError('invalid')
  return message
}

export type MailColleague = {
  userId: string
  name: string
  position: string
  /** 会员头像 data URL。没有照片，或值不是图片 data URL 时缺省。 */
  avatar?: string
}

export type ResolvedColleague = {
  userId: string
  name: string
  address: string
}

/** 与消息头像同一条规则：只接受图片 data URL，其余值不能成为 img 的地址。 */
function savedAvatar(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined
  const avatar = value.trim()
  if (!avatar.startsWith('data:image/') || !avatar.includes(';base64,') || avatar.length > 400_000) return undefined
  return avatar
}

function asColleague(value: unknown): MailColleague | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.user_id !== 'string' || !row.user_id) return null
  const name = typeof row.name === 'string' ? row.name.trim() : ''
  if (!name) return null
  const avatar = savedAvatar(row.avatar)
  return {
    userId: row.user_id,
    name,
    position: typeof row.position === 'string' ? row.position.trim() : '',
    ...(avatar ? { avatar } : {})
  }
}

function asResolved(value: unknown): ResolvedColleague | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.user_id !== 'string' || !row.user_id) return null
  if (typeof row.address !== 'string' || !row.address.trim()) return null
  return {
    userId: row.user_id,
    name: typeof row.name === 'string' ? row.name.trim() : '',
    address: row.address.trim().toLowerCase()
  }
}

/** Directory of teammates who can receive mail. This call does not create mailboxes. */
export async function listMailColleagues(orgId: string): Promise<MailColleague[]> {
  const data = await memberRequest(`/api/mail/colleagues?org_id=${encodeURIComponent(orgId)}`, { method: 'GET' }, fetch)
  if (!Array.isArray(data)) throw new MemberOrgError('invalid')
  return data.map((item) => {
    const row = asColleague(item)
    if (!row) throw new MemberOrgError('invalid')
    return row
  })
}

/** Mailbox addresses for teammates the caller selected. Unknown people create nothing. */
export async function resolveMailColleagues(orgId: string, userIds: string[]): Promise<ResolvedColleague[]> {
  const data = await memberRequest(
    '/api/mail/colleagues',
    { method: 'POST', body: JSON.stringify({ org_id: orgId, user_ids: userIds }) },
    fetch
  )
  if (!Array.isArray(data)) throw new MemberOrgError('invalid')
  return data.map((item) => {
    const row = asResolved(item)
    if (!row) throw new MemberOrgError('invalid')
    return row
  })
}

export async function setMailState(
  orgId: string,
  id: string,
  patch: { unread?: boolean; flagged?: boolean; folder?: string }
): Promise<MailMessage> {
  orgQuery(orgId)
  const message = asMessage(
    await memberRequest(
      `/api/mail/messages/${encodeURIComponent(id)}/state`,
      {
        method: 'POST',
        body: JSON.stringify({ ...patch, org_id: orgId })
      },
      fetch
    )
  )
  if (!message) throw new MemberOrgError('invalid')
  return message
}

/** Short note for one ToBot turn. A closed mailbox stays silent. A denied grant does not list subjects. */
export async function mailTurnNote(): Promise<string> {
  const policy = loadMailToBotPolicy()
  try {
    await publishMailToBotPolicy(policy)
  } catch {
    /* An older plane still enforces nothing until it is restarted. The note still states the grant. */
  }
  if (!policy.allow || !policy.list) return mailToBotNote(policy, '', 0, 'none')
  try {
    const orgId = readCenterId()
    const mailbox = await loadMailbox(orgId)
    const folder = policy.folders.includes('unread') ? 'unread' : (policy.folders[0] ?? 'inbox')
    const page = await listMail(orgId, folder, '', 3)
    const subjects = page.items.slice(0, 3).map((item) => item.subject.trim() || '(no subject)')
    const latest = subjects.length > 0 ? subjects.join('; ') : 'none'
    return mailToBotNote(policy, mailbox.address, mailbox.unread, latest, orgId)
  } catch {
    return ''
  }
}
