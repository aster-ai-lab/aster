import { commandMod, desktopCapabilities } from '@renderer/host/desktopHost'
import type { MailFile, MailMessage } from '@renderer/mail/mailApi'

export const RECIPIENT_CAP = 20
export const SUBJECT_CAP = 200
export const BODY_CAP = 100_000

export type Recipient = { value: string; valid: boolean; label?: string; userId?: string }

export type ComposeKind = 'new' | 'reply' | 'draft'

export type ComposeState = {
  to: Recipient[]
  toDraft: string
  cc: Recipient[]
  ccDraft: string
  subject: string
  body: string
  attachments: MailFile[]
  threadId?: string
  kind: ComposeKind
  draftId?: string
}

export type ComposeFocus = 'to' | 'body'

export const EMPTY_COMPOSE: ComposeState = {
  to: [],
  toDraft: '',
  cc: [],
  ccDraft: '',
  subject: '',
  body: '',
  attachments: [],
  kind: 'new'
}

const MAIL_SUFFIX = '@mail.towork'
const ORG_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

/** Team compose can offer teammates only for a real organization id. Personal center stays out. */
export function teamMailOrgId(centerId: string): string | undefined {
  const value = centerId.trim()
  return ORG_ID.test(value) ? value : undefined
}

/** Same rule as the mail API: local@mail.towork, local is 2–49 lowercase letters, digits, or hyphens. */
export function addressOk(value: string): boolean {
  if (!value.endsWith(MAIL_SUFFIX)) return false
  const local = value.slice(0, -MAIL_SUFFIX.length)
  if (local.length < 2 || local.length > 49) return false
  return /^[a-z0-9][a-z0-9-]*$/.test(local)
}

/** A deliverable mailbox, a real address on another domain, or text that is not an email. */
export function recipientTone(value: string): 'local' | 'outside' | 'bad' {
  if (addressOk(value)) return 'local'
  const at = value.indexOf('@')
  if (at <= 0 || at !== value.lastIndexOf('@')) return 'bad'
  const local = value.slice(0, at)
  const domain = value.slice(at + 1)
  if (domain === 'mail.towork' || !emailLocal(local) || !emailDomain(domain)) return 'bad'
  return 'outside'
}

function emailLocal(local: string): boolean {
  return (
    local.length >= 1 &&
    local.length <= 64 &&
    !local.includes('..') &&
    /^[a-z0-9](?:[a-z0-9._%+-]*[a-z0-9])?$/.test(local)
  )
}

function emailDomain(domain: string): boolean {
  if (domain.length > 253 || domain.includes('..')) return false
  const labels = domain.split('.')
  const tld = labels.at(-1) ?? ''
  if (labels.length < 2 || !/^[a-z]{2,}$/.test(tld)) return false
  return labels.every((label) => /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(label))
}

export function consume(draft: string, force: boolean): { committed: string[]; rest: string } {
  if (!force && !/[,;\s]/.test(draft)) return { committed: [], rest: draft }
  const ended = /[,;\s]$/.test(draft)
  const pieces = draft
    .split(/[,;\s]+/)
    .map((part) => part.trim().toLowerCase())
    .filter(Boolean)
  if (!force && !ended) {
    const rest = pieces.pop() ?? ''
    return { committed: pieces, rest }
  }
  return { committed: pieces, rest: '' }
}

export function addNamedRecipients(
  tokens: Recipient[],
  people: { address: string; name?: string; userId?: string }[]
): Recipient[] {
  const next = [...tokens]
  for (const person of people) {
    const value = person.address.trim().toLowerCase()
    if (!value || next.some((item) => item.value === value)) continue
    const label = person.name?.trim() ?? ''
    const userId = person.userId?.trim() ?? ''
    next.push({
      value,
      valid: addressOk(value),
      ...(label && label.toLowerCase() !== value ? { label } : {}),
      ...(userId ? { userId } : {})
    })
  }
  return next
}

export function mergeTokens(tokens: Recipient[], values: string[]): Recipient[] {
  const next = [...tokens]
  for (const value of values) {
    if (next.some((item) => item.value === value)) continue
    next.push({ value, valid: addressOk(value) })
  }
  return next
}

export function flushTokens(tokens: Recipient[], pending: string): Recipient[] {
  return mergeTokens(tokens, consume(pending, true).committed)
}

function validPending(draft: string): string | null {
  const value = draft.trim().toLowerCase()
  if (!value || /[,;\s]/.test(value)) return null
  return addressOk(value) ? value : null
}

export function listedRecipients(value: ComposeState): {
  to: string[]
  cc: string[]
  invalid: boolean
  outside: boolean
  bad: boolean
  count: number
} {
  const to = [...value.to]
  const pendingTo = validPending(value.toDraft)
  if (pendingTo && !to.some((item) => item.value === pendingTo)) to.push({ value: pendingTo, valid: true })
  const cc = value.cc.filter((item) => !to.some((row) => row.value === item.value))
  const pendingCc = validPending(value.ccDraft)
  if (pendingCc && !to.some((item) => item.value === pendingCc) && !cc.some((item) => item.value === pendingCc)) {
    cc.push({ value: pendingCc, valid: true })
  }
  const rows = [...to, ...cc]
  const bad = rows.some((item) => recipientTone(item.value) === 'bad')
  const outside = rows.some((item) => recipientTone(item.value) === 'outside')
  return {
    to: to.filter((item) => item.valid).map((item) => item.value),
    cc: cc.filter((item) => item.valid).map((item) => item.value),
    invalid: bad || outside,
    outside,
    bad,
    count: rows.length
  }
}

export function tokensFrom(addresses: string[]): Recipient[] {
  return mergeTokens([], addresses.map((address) => address.trim().toLowerCase()).filter(Boolean))
}

export function subjectText(subject: string, empty: string): string {
  const trimmed = subject.trim()
  return trimmed || empty
}

export function whenLabel(value: string): string {
  const time = Date.parse(value)
  if (!Number.isFinite(time)) return ''
  return new Date(time).toLocaleString(undefined, {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  })
}

export function composeDirty(value: ComposeState, baseline = ''): boolean {
  return (
    value.to.length > 0 ||
    value.cc.length > 0 ||
    value.toDraft.trim().length > 0 ||
    value.ccDraft.trim().length > 0 ||
    value.subject.trim().length > 0 ||
    value.attachments.length > 0 ||
    value.body !== baseline
  )
}

export function sameRecipients(left: ComposeState, right: ComposeState): boolean {
  const key = (items: Recipient[]) => items.map((item) => item.value).join('\n')
  return key(left.to) === key(right.to) && key(left.cc) === key(right.cc)
}

function replySubject(subject: string): string {
  return subject.trim().toLowerCase().startsWith('re:') ? subject : `Re: ${subject}`.trim()
}

export function replyDraft(message: MailMessage, body: string): ComposeState {
  const to = message.folder === 'sent' ? message.to : [message.from_address]
  return {
    to: tokensFrom(to),
    toDraft: '',
    cc: [],
    ccDraft: '',
    subject: replySubject(message.subject),
    body,
    attachments: [],
    threadId: message.thread_id || undefined,
    kind: 'reply'
  }
}

export function replyAllDraft(message: MailMessage, self: string, body: string): ComposeState | null {
  const mine = self.trim().toLowerCase()
  if (!mine) return null
  const to: string[] = []
  const addTo = (value: string) => {
    const key = value.trim().toLowerCase()
    if (!key || key === mine || to.includes(key)) return
    to.push(key)
  }
  if (message.folder === 'sent') message.to.forEach(addTo)
  else {
    addTo(message.from_address)
    message.to.forEach(addTo)
  }
  const cc: string[] = []
  for (const value of message.cc) {
    const key = value.trim().toLowerCase()
    if (!key || key === mine || to.includes(key) || cc.includes(key)) continue
    cc.push(key)
  }
  return {
    to: tokensFrom(to),
    toDraft: '',
    cc: tokensFrom(cc),
    ccDraft: '',
    subject: replySubject(message.subject),
    body,
    attachments: [],
    threadId: message.thread_id || undefined,
    kind: 'reply'
  }
}

export function continueDraft(message: MailMessage): ComposeState {
  return {
    to: tokensFrom(message.to),
    toDraft: '',
    cc: tokensFrom(message.cc),
    ccDraft: '',
    subject: message.subject,
    body: message.body,
    attachments: message.attachments ?? [],
    threadId: message.thread_id || undefined,
    kind: 'draft',
    draftId: message.id
  }
}

function usesCommandKey(): boolean {
  return commandMod(desktopCapabilities().platform) === 'cmd'
}

export function commandEnterLabel(): string {
  return usesCommandKey() ? '⌘ Enter' : 'Ctrl Enter'
}

/** Visible hint for a chord such as `Mod+S` or `Mod+Shift+C`. */
export function chordLabel(chord: string): string {
  const mac = usesCommandKey()
  return chord
    .split('+')
    .map((part) => {
      if (part === 'Mod') return mac ? '⌘' : 'Ctrl'
      if (part === 'Shift') return mac ? '⇧' : 'Shift'
      if (part === 'Alt') return mac ? '⌥' : 'Alt'
      if (part === 'Enter') return mac ? '↩' : 'Enter'
      return part
    })
    .join(mac ? '' : '+')
}

/** `aria-keyshortcuts` value for both modifier keys. */
export function chordKeys(chord: string): string {
  const rest = chord.startsWith('Mod+') ? chord.slice('Mod+'.length) : chord
  return `Meta+${rest} Control+${rest}`
}

export function composeFailure(message: string, translate: (key: string) => string): string {
  if (message === 'unknown recipient') return translate('mail.compose.unknown')
  if (message === 'colleague was not found') return translate('mail.compose.pick_missing')
  if (message === 'you are not a member of this enterprise') return translate('mail.compose.pick_error')
  if (message === 'recipient must be an @mail.towork address') return translate('mail.compose.outside')
  if (message === 'a recipient is required') return translate('mail.compose.invalid')
  if (message === 'a message can name at most 20 recipients') return translate('mail.compose.limit')
  if (message === 'subject must be at most 200 characters' || message === 'subject must be one line') {
    return translate('mail.compose.subject_limit')
  }
  if (message === 'body must be at most 100000 characters') return translate('mail.compose.body_limit')
  if (message === 'an attachment is too large') return translate('mail.attachment.too_large')
  if (message === 'attachments are too large') return translate('mail.attachment.total')
  if (message === 'a message can have at most 8 attachments') return translate('mail.attachment.too_many')
  if (message === 'attachment name is invalid' || message === 'attachment data is invalid') {
    return translate('mail.attachment.invalid')
  }
  if (message.includes('unknown field') && message.includes('attachments'))
    return translate('mail.attachment.unavailable')
  return message
}

export type MailComposeSeed = {
  kind: ComposeKind
  to: string[]
  cc: string[]
  subject: string
  body: string
  threadId: string
  draftId: string
  focus: ComposeFocus
  from: string
  title: string
  attachments: MailFile[]
  orgId: string
}

export function composeSeed(
  value: ComposeState,
  focus: ComposeFocus,
  from: string,
  title: string,
  orgId = ''
): MailComposeSeed {
  return {
    kind: value.kind,
    to: value.to.map((item) => item.value),
    cc: value.cc.map((item) => item.value),
    subject: value.subject,
    body: value.body,
    threadId: value.threadId ?? '',
    draftId: value.draftId ?? '',
    focus,
    from,
    title,
    attachments: value.attachments,
    orgId
  }
}
