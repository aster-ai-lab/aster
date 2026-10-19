export const MAIL_TOBOT_KEY = 'mail.tobot.v1'

export const MAIL_TOBOT_FOLDERS = ['inbox', 'unread', 'flagged', 'drafts', 'sent', 'spam', 'trash'] as const

export type MailToBotFolder = (typeof MAIL_TOBOT_FOLDERS)[number]

export type MailToBotPolicy = {
  allow: boolean
  list: boolean
  read: boolean
  folders: MailToBotFolder[]
}

const DEFAULT_FOLDERS: MailToBotFolder[] = ['inbox', 'unread', 'flagged', 'sent']

export function defaultMailToBotPolicy(): MailToBotPolicy {
  return { allow: false, list: true, read: false, folders: [...DEFAULT_FOLDERS] }
}

function isFolder(value: string): value is MailToBotFolder {
  return (MAIL_TOBOT_FOLDERS as readonly string[]).includes(value)
}

export function normalizeMailToBotPolicy(value: unknown): MailToBotPolicy {
  const record = value && typeof value === 'object' ? (value as Record<string, unknown>) : {}
  const picked = Array.isArray(record.folders)
    ? record.folders.filter((item): item is MailToBotFolder => typeof item === 'string' && isFolder(item))
    : []
  const folders = picked.length > 0 ? [...new Set(picked)] : [...DEFAULT_FOLDERS]
  return {
    allow: record.allow === true,
    list: record.list !== false,
    read: record.read === true,
    folders
  }
}

export function loadMailToBotPolicy(): MailToBotPolicy {
  try {
    return normalizeMailToBotPolicy(JSON.parse(localStorage.getItem(MAIL_TOBOT_KEY) ?? ''))
  } catch {
    return defaultMailToBotPolicy()
  }
}

export function saveMailToBotPolicy(policy: MailToBotPolicy): MailToBotPolicy {
  const next = normalizeMailToBotPolicy(policy)
  localStorage.setItem(MAIL_TOBOT_KEY, JSON.stringify(next))
  return next
}

/** One sentence the ToBot turn can follow. The tool call is still checked on the plane. */
export function mailToBotNote(
  policy: MailToBotPolicy,
  address: string,
  unread: number,
  subjects: string,
  orgId?: string
): string {
  if (!policy.allow || !policy.list) {
    return 'ToBot is not allowed to manage this mailbox. Do not use member-mail tools or invent messages.'
  }
  const folders = policy.folders.join(',')
  const read = policy.read ? ' read_mail is allowed for those folders.' : ' read_mail is not allowed.'
  const team = orgId ? `Use org_id=${orgId} for member-mail tools. ` : ''
  const note = `${team}Mailbox ${address}. Unread ${unread}. Latest subjects: ${subjects}. ToBot may list_mail in ${folders}.${read} Do not send, delete, or change messages. Do not invent messages.`
  return note.slice(0, 480)
}
