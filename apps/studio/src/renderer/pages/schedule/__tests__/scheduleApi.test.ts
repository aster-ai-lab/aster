import { afterEach, describe, expect, it, vi } from 'vitest'

import { createScheduleEvent, deleteScheduleEvent, listScheduleEvents, updateScheduleEvent } from '../scheduleApi'

const runtime = {
  apiBaseUrl: 'http://127.0.0.1:9',
  token: 'ab'.repeat(32)
}

const row = {
  id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af10',
  title: '周例会',
  location: '9F',
  note: '',
  starts_at: '2026-10-06T05:00:00.000Z',
  ends_at: '2026-10-06T07:00:00.000Z',
  phone: '18700000000',
  token: 'a3s_ms_secret'
}

function envelope(data: unknown, status = 200) {
  return new Response(JSON.stringify({ code: 0, message: 'ok', data }), { status })
}

afterEach(() => {
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
  vi.unstubAllGlobals()
})

describe('scheduleApi', () => {
  it('reads and writes the member schedule path and drops fields outside the event', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { value: runtime, configurable: true })
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(envelope([row]))
    vi.stubGlobal('fetch', fetchMock)

    const from = new Date('2026-10-04T00:00:00.000Z')
    const to = new Date('2026-10-11T00:00:00.000Z')
    const events = await listScheduleEvents('org-1', from, to)
    expect(events[0].can_update).toBe(false)
    expect(events[0].can_delete).toBe(false)
    expect(events[0]).not.toHaveProperty('phone')
    expect(events[0].repeat_rule).toBe('none')
    expect(events[0].reminders).toEqual([5])
    expect(events[0].all_day).toBe(false)
    expect(events[0].guests).toEqual([])
    expect(events[0].availability).toBe('busy')
    expect(events[0].visibility).toBe('default')
    expect(events[0].guest_permission).toBe('invite_notes')
    expect(events[0].minutes).toBe('')
    expect(events[0].time_zone).toBe('UTC')
    expect(events[0].utc_offset_minutes).toBe(0)
    expect(JSON.stringify(events[0])).not.toContain('a3s_ms_')
    const listUrl = String(fetchMock.mock.calls[0][0])
    expect(listUrl).toContain('/api/member/orgs/org-1/schedule/events?')
    expect(listUrl).toContain('from=2026-10-04T00%3A00%3A00.000Z')
    expect(fetchMock.mock.calls[0][1]).toEqual(expect.objectContaining({ method: 'GET' }))

    fetchMock.mockResolvedValueOnce(envelope(row, 201))
    const draft = {
      title: '周例会',
      location: '9F',
      note: '',
      starts_at: row.starts_at,
      ends_at: row.ends_at
    }
    await createScheduleEvent('org-1', draft)
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/schedule/events',
      expect.objectContaining({ method: 'POST', body: JSON.stringify(draft) })
    )

    fetchMock.mockResolvedValueOnce(envelope(row))
    await updateScheduleEvent('org-1', row.id, draft)
    expect(fetchMock).toHaveBeenLastCalledWith(
      `http://127.0.0.1:9/api/member/orgs/org-1/schedule/events/${row.id}`,
      expect.objectContaining({ method: 'PUT' })
    )

    fetchMock.mockResolvedValueOnce(envelope({ ok: true }))
    await deleteScheduleEvent('org-1', row.id)
    expect(fetchMock).toHaveBeenLastCalledWith(
      `http://127.0.0.1:9/api/member/orgs/org-1/schedule/events/${row.id}`,
      expect.objectContaining({ method: 'DELETE' })
    )
  })
})
