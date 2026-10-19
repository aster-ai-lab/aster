import { memberRequest, MemberOrgError } from '@renderer/data/memberOrgs'

export type ScheduleGuest = {
  member_id?: string
  email?: string
}

export type ScheduleEvent = {
  id: string
  title: string
  location: string
  note: string
  minutes: string
  starts_at: string
  ends_at: string
  all_day: boolean
  time_zone: string
  utc_offset_minutes: number
  repeat_rule: string
  visibility: 'default' | 'public' | 'private'
  availability: 'busy' | 'free'
  guest_permission: 'invite' | 'invite_notes'
  reminders: number[]
  guests: ScheduleGuest[]
  can_update?: boolean
  can_delete?: boolean
}

export type ScheduleDraft = {
  title: string
  location: string
  note: string
  minutes?: string
  starts_at: string
  ends_at: string
  all_day?: boolean
  time_zone?: string
  utc_offset_minutes?: number
  repeat_rule?: string
  visibility?: ScheduleEvent['visibility']
  availability?: ScheduleEvent['availability']
  guest_permission?: ScheduleEvent['guest_permission']
  reminders?: number[]
  guests?: ScheduleGuest[]
}

function asEvent(value: unknown): ScheduleEvent | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.id !== 'string' || !row.id) return null
  if (typeof row.title !== 'string' || !row.title.trim()) return null
  if (typeof row.starts_at !== 'string' || typeof row.ends_at !== 'string') return null
  const start = Date.parse(row.starts_at)
  const end = Date.parse(row.ends_at)
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) return null
  return {
    id: row.id,
    can_update: row.can_update === true,
    can_delete: row.can_delete === true,
    title: row.title.trim(),
    location: typeof row.location === 'string' ? row.location : '',
    note: typeof row.note === 'string' ? row.note : '',
    minutes: typeof row.minutes === 'string' ? row.minutes : '',
    starts_at: row.starts_at,
    ends_at: row.ends_at,
    all_day: row.all_day === true,
    time_zone: typeof row.time_zone === 'string' && row.time_zone ? row.time_zone : 'UTC',
    utc_offset_minutes:
      typeof row.utc_offset_minutes === 'number' && Number.isFinite(row.utc_offset_minutes)
        ? row.utc_offset_minutes
        : 0,
    repeat_rule: typeof row.repeat_rule === 'string' && row.repeat_rule ? row.repeat_rule : 'none',
    visibility: row.visibility === 'public' || row.visibility === 'private' ? row.visibility : 'default',
    availability: row.availability === 'free' ? 'free' : 'busy',
    guest_permission: row.guest_permission === 'invite' ? 'invite' : 'invite_notes',
    reminders: Array.isArray(row.reminders)
      ? row.reminders.filter((item): item is number => typeof item === 'number')
      : [5],
    guests: Array.isArray(row.guests) ? row.guests.flatMap(asGuest) : []
  }
}

function asGuest(value: unknown): ScheduleGuest[] {
  if (!value || typeof value !== 'object') return []
  const row = value as Record<string, unknown>
  const member_id = typeof row.member_id === 'string' && row.member_id ? row.member_id : undefined
  const email = typeof row.email === 'string' ? row.email.trim() : ''
  if (member_id) return [{ member_id }]
  if (email) return [{ email }]
  return []
}

function eventsPath(orgId: string, eventId = ''): string {
  const base = `/api/member/orgs/${encodeURIComponent(orgId)}/schedule/events`
  return eventId ? `${base}/${encodeURIComponent(eventId)}` : base
}

export async function listScheduleEvents(orgId: string, from: Date, to: Date): Promise<ScheduleEvent[]> {
  const params = new URLSearchParams({ from: from.toISOString(), to: to.toISOString() })
  const data = await memberRequest(`${eventsPath(orgId)}?${params.toString()}`, { method: 'GET' }, fetch)
  if (!Array.isArray(data)) throw new MemberOrgError('invalid')
  return data.flatMap((row) => {
    const event = asEvent(row)
    return event ? [event] : []
  })
}

export async function createScheduleEvent(orgId: string, draft: ScheduleDraft): Promise<ScheduleEvent> {
  const event = asEvent(await memberRequest(eventsPath(orgId), { method: 'POST', body: JSON.stringify(draft) }, fetch))
  if (!event) throw new MemberOrgError('invalid')
  return event
}

export async function updateScheduleEvent(
  orgId: string,
  eventId: string,
  draft: ScheduleDraft
): Promise<ScheduleEvent> {
  const event = asEvent(
    await memberRequest(eventsPath(orgId, eventId), { method: 'PUT', body: JSON.stringify(draft) }, fetch)
  )
  if (!event) throw new MemberOrgError('invalid')
  return event
}

export async function deleteScheduleEvent(orgId: string, eventId: string): Promise<void> {
  await memberRequest(eventsPath(orgId, eventId), { method: 'DELETE' }, fetch)
}
