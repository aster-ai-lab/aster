import { readCenterId } from '@renderer/data/memberOrgs'
import { publishWorkToBotPolicy } from '@renderer/data/studioAgentPlane'

import { loadWorkItems, type WorkKind } from './client'

export const WORK_TOBOT_KEY = 'work.tobot.v1'

export const WORK_TOBOT_KINDS = [
  'join_request',
  'approval',
  'leave',
  'travel',
  'expense',
  'meeting',
  'seal',
  'notice'
] as const satisfies readonly WorkKind[]

export type WorkToBotKind = (typeof WORK_TOBOT_KINDS)[number]

export type WorkToBotPolicy = {
  allow: boolean
  list: boolean
  read: boolean
  kinds: WorkToBotKind[]
}

const KIND_LABEL: Record<WorkToBotKind, string> = {
  join_request: 'work.inbox.kind.join',
  approval: 'work.inbox.kind.approval',
  leave: 'work.inbox.kind.leave',
  travel: 'work.inbox.kind.travel',
  expense: 'work.inbox.kind.expense',
  meeting: 'work.inbox.kind.meeting',
  seal: 'work.inbox.kind.seal',
  notice: 'work.inbox.kind.notice'
}

const TEAM_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

export function workToBotKindLabel(kind: WorkToBotKind): string {
  return KIND_LABEL[kind]
}

export function defaultWorkToBotPolicy(): WorkToBotPolicy {
  return { allow: false, list: true, read: false, kinds: [...WORK_TOBOT_KINDS] }
}

function isKind(value: string): value is WorkToBotKind {
  return (WORK_TOBOT_KINDS as readonly string[]).includes(value)
}

export function normalizeWorkToBotPolicy(value: unknown): WorkToBotPolicy {
  const record = value && typeof value === 'object' ? (value as Record<string, unknown>) : {}
  const picked = Array.isArray(record.kinds)
    ? record.kinds.filter((item): item is WorkToBotKind => typeof item === 'string' && isKind(item))
    : []
  const kinds = picked.length > 0 ? [...new Set(picked)] : [...WORK_TOBOT_KINDS]
  return {
    allow: record.allow === true,
    list: record.list !== false,
    read: record.read === true,
    kinds
  }
}

export function loadWorkToBotPolicy(): WorkToBotPolicy {
  try {
    return normalizeWorkToBotPolicy(JSON.parse(localStorage.getItem(WORK_TOBOT_KEY) ?? ''))
  } catch {
    return defaultWorkToBotPolicy()
  }
}

export function saveWorkToBotPolicy(policy: WorkToBotPolicy): WorkToBotPolicy {
  const next = normalizeWorkToBotPolicy(policy)
  localStorage.setItem(WORK_TOBOT_KEY, JSON.stringify(next))
  return next
}

/** One sentence the ToBot turn can follow. The tool call is still checked on the plane. */
export function workToBotNote(policy: WorkToBotPolicy, orgId: string | undefined, titles: readonly string[]): string {
  if (!policy.allow || !policy.list) {
    return 'ToBot is not allowed to manage these tasks. Do not use member-work tools or invent tasks.'
  }
  const team =
    orgId && TEAM_ID.test(orgId) ? `Use org_id=${orgId} for member-work tools. ` : 'A team org_id is required. '
  const read = policy.read ? ' read_work is allowed for those kinds.' : ' read_work is not allowed.'
  const latest = titles.length > 0 ? titles.join('; ') : 'none'
  const note = `${team}Tasks pending. Latest titles: ${latest}. ToBot may list_work for kinds ${policy.kinds.join(',')}.${read} Do not approve, reject, cancel, or create tasks. Do not invent tasks.`
  return note.slice(0, 480)
}

/** Short note for one ToBot turn. A denied grant does not list titles. */
export async function workTurnNote(): Promise<string> {
  const policy = loadWorkToBotPolicy()
  try {
    await publishWorkToBotPolicy(policy)
  } catch {
    /* An older plane still enforces nothing until it is restarted. The note still states the grant. */
  }
  if (!policy.allow || !policy.list) return workToBotNote(policy, undefined, [])
  try {
    const orgId = readCenterId()
    const items = await loadWorkItems({ box: 'todo' })
    const titles = items
      .filter((item) => policy.kinds.includes(item.kind))
      .slice(0, 3)
      .map((item) => item.title.trim() || '(no title)')
    return workToBotNote(policy, orgId, titles)
  } catch {
    return ''
  }
}
