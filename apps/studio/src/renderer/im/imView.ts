import { chatRecordTitle } from './forwardRecord'
/** Pure display rules for IM data: titles, previews, system notices and read state. */
import type { ImConversationDetail, ImInboxEntry, ImMessage, ImPreview, ImSender } from './imApi'
import type { ImSnapshot } from './imStore'
import { decodeBossWire, visibleText } from './quote'

export type Translate = (key: string, options?: Record<string, unknown>) => string

export const RECALL_WINDOW_MS = 5 * 60 * 1000

/** The sidebar badge: muted conversations still show their own count but never add to the total. */
export function totalUnread(conversations: readonly Pick<ImInboxEntry, 'unread' | 'muted'>[]): number {
  return conversations.reduce((sum, entry) => (entry.muted ? sum : sum + entry.unread), 0)
}

/** ActivityBar count. Personal mode keeps ToBot unread; a signed-out or loading inbox does not. */
export function activityBarUnread(snapshot: Pick<ImSnapshot, 'status' | 'conversations'>): number {
  if (snapshot.status !== 'ready' && snapshot.status !== 'no-org') return 0
  return totalUnread(snapshot.conversations)
}

export function personName(snapshot: Pick<ImSnapshot, 'me' | 'people'>, userId: string): string {
  if (snapshot.me && userId === snapshot.me.userId) return snapshot.me.displayName
  return snapshot.people[userId]?.displayName || userId.slice(0, 8)
}

export function senderName(snapshot: Pick<ImSnapshot, 'me' | 'people'>, sender: ImSender | null, t: Translate): string {
  if (!sender) return ''
  if (sender.kind === 'service') return t('messages.system_sender')
  return personName(snapshot, sender.id)
}

export function conversationTitle(
  entry: Pick<ImInboxEntry, 'kind' | 'title' | 'peerUserId' | 'remark'>,
  snapshot: Pick<ImSnapshot, 'me' | 'people'>,
  t: Translate
): string {
  const remark = entry.remark?.trim()
  if (remark) return remark
  if (entry.title) return entry.title
  if (entry.kind === 'direct' && entry.peerUserId) return personName(snapshot, entry.peerUserId)
  if (entry.kind === 'product' || entry.kind === 'transfer') return t('messages.file_transfer')
  if (entry.kind === 'boss') return t('messages.toboss')
  if (entry.kind === 'group') return t('messages.new_group')
  return t('messages.new_conversation')
}

export function systemNotice(
  code: string,
  params: Record<string, string>,
  snapshot: Pick<ImSnapshot, 'me' | 'people'>,
  t: Translate
): string {
  const name = (userId: string | undefined) => (userId ? personName(snapshot, userId) : '')
  switch (code) {
    case 'member_added':
      return t('messages.system.member_added', {
        actor: name(params.actor_id),
        names: (params.added_ids ?? '')
          .split(',')
          .map((id) => id.trim())
          .filter(Boolean)
          .map((id) => name(id))
          .join(t('messages.list_separator'))
      })
    case 'member_removed':
      return t('messages.system.member_removed', { actor: name(params.actor_id), name: name(params.removed_id) })
    case 'member_left':
      return t('messages.system.member_left', { actor: name(params.actor_id) })
    case 'group_dissolved':
      return t('messages.system.group_dissolved', { actor: name(params.actor_id) })
    case 'title_changed':
      return t('messages.system.title_changed', { actor: name(params.actor_id), title: params.title ?? '' })
    case 'owner_transferred':
      return t('messages.system.owner_transferred', { actor: name(params.actor_id), name: name(params.user_id) })
    case 'role_changed':
      return t('messages.system.role_changed', {
        actor: name(params.actor_id),
        name: name(params.user_id),
        role: t(params.role === 'admin' ? 'messages.role.admin' : 'messages.role.member')
      })
    case 'mute_all_on':
      return t('messages.system.mute_all_on', { actor: name(params.actor_id) })
    case 'mute_all_off':
      return t('messages.system.mute_all_off', { actor: name(params.actor_id) })
    case 'speech_banned':
      return t('messages.system.speech_banned', { actor: name(params.actor_id), name: name(params.user_id) })
    case 'speech_unbanned':
      return t('messages.system.speech_unbanned', { actor: name(params.actor_id), name: name(params.user_id) })
    case 'join_request':
      return t('messages.system.join_request', { name: params.actor_name ?? '' })
    default:
      return t('messages.system.unknown')
  }
}

export function recallNotice(
  sender: ImSender | null,
  snapshot: Pick<ImSnapshot, 'me' | 'people'>,
  t: Translate
): string {
  if (sender && snapshot.me && sender.kind === 'member' && sender.id === snapshot.me.userId)
    return t('messages.recall_self')
  return t('messages.recall', { name: senderName(snapshot, sender, t) })
}

export function previewText(
  entry: ImInboxEntry,
  snapshot: Pick<ImSnapshot, 'me' | 'people'> & Partial<Pick<ImSnapshot, 'details'>>,
  t: Translate
): string {
  const preview = entry.preview
  if (!preview) return ''
  if (preview.recalled) return recallNotice(preview.sender, snapshot, t)
  if (preview.text) {
    const wire = decodeBossWire(preview.text)
    if (wire?.kind === 'join') return t('messages.toboss.joined')
    if (wire?.kind === 'leave') return t('messages.toboss.left')
    if (wire?.kind === 'reply') {
      const content = wire.text.trim()
      return entry.kind === 'group' ? `${t('messages.toboss')}: ${content}` : content
    }
  }
  const content = previewContent(preview, snapshot, t)
  const fromOther = preview.sender && preview.sender.kind === 'member' && preview.sender.id !== snapshot.me?.userId
  if (entry.kind === 'group' && fromOther && preview.kind !== 'system' && preview.sender) {
    const nickname = snapshot.details?.[entry.id]?.members
      .find((member) => member.userId === preview.sender?.id && !member.left)
      ?.nickname?.trim()
    const name = nickname || senderName(snapshot, preview.sender, t)
    return `${name}: ${content}`
  }
  return content
}

function previewContent(preview: ImPreview, snapshot: Pick<ImSnapshot, 'me' | 'people'>, t: Translate): string {
  switch (preview.kind) {
    case 'file':
      return `${t('messages.preview.file')} ${preview.name ?? ''}`.trim()
    case 'image':
      return t('messages.preview.image')
    case 'voice':
      return t('messages.preview.voice')
    case 'system':
      return preview.code ? systemNotice(preview.code, preview.params, snapshot, t) : ''
    default: {
      const text = visibleText(preview.text ?? '')
      return chatRecordTitle(text) ?? text
    }
  }
}

export type ReadState = { kind: 'read' } | { kind: 'unread' } | { kind: 'unread_count'; count: number } | null

/** Read state of one of my messages, from the other members' read cursors. */
export function readState(
  message: ImMessage,
  detail: ImConversationDetail | undefined,
  meId: string | undefined
): ReadState {
  if (!detail || !meId || message.sender.kind !== 'member' || message.sender.id !== meId) return null
  if (detail.kind !== 'direct' && detail.kind !== 'group') return null
  const others = detail.members.filter((member) => !member.left && member.userId !== meId)
  if (others.length === 0) return null
  const behind = others.filter((member) => member.lastReadSequence < message.sequence).length
  if (behind === 0) return { kind: 'read' }
  if (detail.kind === 'direct') return { kind: 'unread' }
  return { kind: 'unread_count', count: behind }
}

export function canRecall(message: ImMessage, meId: string | undefined, now: number): boolean {
  return (
    !message.recalled &&
    message.body.kind !== 'system' &&
    message.sender.kind === 'member' &&
    message.sender.id === meId &&
    now - message.createdAt < RECALL_WINDOW_MS
  )
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${Math.round(bytes)}B`
  const kilobytes = bytes / 1024
  if (kilobytes < 1024) return formatUnit(kilobytes, 'K')
  return formatUnit(bytes / (1024 * 1024), 'M')
}

function formatUnit(value: number, unit: string): string {
  const rounded = Math.round(value * 10) / 10
  return `${Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1)}${unit}`
}

export function formatConversationTime(atTime: number, now: number, yesterday: string, justNow: string): string {
  if (now - atTime < 60_000) return justNow
  const date = new Date(atTime)
  const today = new Date(now)
  const clock = `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`
  if (sameDay(date, today)) return clock
  const previous = new Date(today)
  previous.setDate(today.getDate() - 1)
  if (sameDay(date, previous)) return `${yesterday} ${clock}`
  return `${String(date.getMonth() + 1).padStart(2, '0')}/${String(date.getDate()).padStart(2, '0')}`
}

function sameDay(left: Date, right: Date): boolean {
  return (
    left.getFullYear() === right.getFullYear() &&
    left.getMonth() === right.getMonth() &&
    left.getDate() === right.getDate()
  )
}

export function showTimeDivider(times: readonly number[], index: number): boolean {
  if (index === 0) return true
  return times[index] - times[index - 1] >= 5 * 60 * 1000
}

export type DepartmentNode = { id: string; name: string; depth: number }

/** Department rows in tree order (parents before children, siblings by sort order then name). */
export function departmentTree(
  departments: ReadonlyArray<{ id: string; parentId: string | null; name: string; sortOrder: number }>
): DepartmentNode[] {
  const ids = new Set(departments.map((department) => department.id))
  const children = new Map<string | null, (typeof departments)[number][]>()
  for (const department of departments) {
    const parent = department.parentId && ids.has(department.parentId) ? department.parentId : null
    const siblings = children.get(parent) ?? []
    siblings.push(department)
    children.set(parent, siblings)
  }
  const out: DepartmentNode[] = []
  const visit = (parent: string | null, depth: number) => {
    const siblings = (children.get(parent) ?? [])
      .slice()
      .sort((left, right) => left.sortOrder - right.sortOrder || left.name.localeCompare(right.name))
    for (const department of siblings) {
      out.push({ id: department.id, name: department.name, depth })
      visit(department.id, depth + 1)
    }
  }
  visit(null, 0)
  return out
}
