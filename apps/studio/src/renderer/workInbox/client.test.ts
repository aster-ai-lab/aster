import { afterEach, describe, expect, it, vi } from 'vitest'

import { createWorkItem, decideWorkItem, loadWorkCenter, loadWorkItems, loadWorkSummary } from './client'

const TICKET = 'ab'.repeat(32)
const SHELL = 'cd'.repeat(32)
const BASE = 'http://127.0.0.1:9'

const approval = {
  id: '11111111-1111-4111-8111-111111111111',
  org_id: 'org-1',
  org_name: '通友科技',
  kind: 'approval',
  title: '采购',
  summary: '说明',
  status: 'pending',
  initiator_name: '林',
  current_node: '审批',
  created_at: '2026-10-07T00:00:00Z',
  actionable: false,
  approvable: false,
  rejectable: false,
  cancellable: true,
  fields: { title: '采购', detail: '说明' },
  decision_comment: '',
  assignee_names: ['创建人', '静默审批人'],
  decider_name: '创建人',
  decided_at: '2026-10-14T03:51:53Z',
  schedule_retry: false,
  engine: false,
  trail: []
}

afterEach(() => {
  Reflect.deleteProperty(globalThis, '__A3S_APPLET_RUNTIME__')
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
  localStorage.removeItem('a3s.org.center')
})

function installApplet(appId: string) {
  Object.defineProperty(globalThis, '__A3S_APPLET_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: BASE, appletTicket: TICKET, appId, tenant: 'toyou', permissions: [] }
  })
}

function installShell() {
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: BASE, token: SHELL }
  })
}

function jsonResponse(data: unknown, status = 200) {
  return Promise.resolve(new Response(JSON.stringify({ code: 0, message: 'ok', data }), { status }))
}

describe('work inbox client', () => {
  it('uses the inbox ticket for one summary read and one list read', async () => {
    installApplet('inbox')
    installShell()
    const fetchImpl = vi.fn((url: string) => {
      const target = String(url)
      if (target.endsWith('/work-items/summary')) return jsonResponse({ todo: 2, done: 1, cc: 0, sent: 3 })
      return jsonResponse([approval])
    })

    await expect(loadWorkSummary(fetchImpl)).resolves.toEqual({ todo: 2, done: 1, cc: 0, sent: 3 })
    await expect(loadWorkItems({ box: 'todo', kind: 'approval', q: '采购' }, fetchImpl)).resolves.toEqual([approval])

    expect(fetchImpl).toHaveBeenCalledTimes(2)
    const [summaryUrl, summaryInit] = fetchImpl.mock.calls[0]
    const [listUrl, listInit] = fetchImpl.mock.calls[1]
    expect(String(summaryUrl)).toBe(`${BASE}/api/applet-runtime/inbox/work-items/summary`)
    expect(String(listUrl)).toBe(
      `${BASE}/api/applet-runtime/inbox/work-items?box=todo&kind=approval&q=%E9%87%87%E8%B4%AD`
    )
    expect(summaryInit?.headers).toMatchObject({ authorization: `Bearer ${TICKET}` })
    expect(listInit?.headers).toMatchObject({ authorization: `Bearer ${TICKET}` })
    expect(JSON.stringify(fetchImpl.mock.calls)).not.toContain(SHELL)
  })

  it('keeps the main window on the shell member path', async () => {
    installShell()
    const fetchImpl = vi.fn(() => jsonResponse({ todo: 0, done: 0, cc: 0, sent: 0 }))
    await loadWorkSummary(fetchImpl)
    expect(String(fetchImpl.mock.calls[0][0])).toBe(`${BASE}/api/member/work-items/summary?org=personal`)
    expect(fetchImpl.mock.calls[0][1]?.headers).toMatchObject({ authorization: `Bearer ${SHELL}` })
    await expect(loadWorkCenter(fetchImpl)).resolves.toBeNull()
    expect(fetchImpl).toHaveBeenCalledTimes(1)
  })

  it('does not call the network when neither runtime is present', async () => {
    const fetchImpl = vi.fn()
    await expect(loadWorkSummary(fetchImpl)).resolves.toEqual({ todo: 0, done: 0, cc: 0, sent: 0 })
    await expect(loadWorkItems({ box: 'todo' }, fetchImpl)).rejects.toThrow('runtime')
    expect(fetchImpl).not.toHaveBeenCalled()
  })

  it('lets a leave window create an approval and refuses its list', async () => {
    installApplet('leave')
    const fetchImpl = vi.fn((url: string, init?: RequestInit) => {
      if (init?.method === 'POST') return jsonResponse(approval, 201)
      if (String(url).endsWith('/center'))
        return Promise.resolve(new Response(JSON.stringify({ id: 'org-1' }), { status: 200 }))
      return Promise.resolve(new Response(JSON.stringify({ error: 'not_found' }), { status: 404 }))
    })

    await expect(loadWorkCenter(fetchImpl)).resolves.toBe('org-1')
    await expect(
      createWorkItem(
        {
          orgId: 'org-1',
          kind: 'leave',
          idempotencyKey: '11111111-1111-4111-8111-111111111111',
          fields: { leave_type: '事假', start_date: '2026-10-01', end_date: '2026-10-02', reason: '事假' }
        },
        fetchImpl
      )
    ).resolves.toMatchObject({
      id: approval.id,
      title: '采购'
    })
    await expect(loadWorkItems({ box: 'todo' }, fetchImpl)).rejects.toThrow('runtime')

    expect(String(fetchImpl.mock.calls[0][0])).toBe(`${BASE}/api/applet-runtime/leave/center`)
    expect(String(fetchImpl.mock.calls[1][0])).toBe(`${BASE}/api/applet-runtime/leave/work-items`)
    expect(fetchImpl.mock.calls[1][1]?.headers).toMatchObject({ authorization: `Bearer ${TICKET}` })
    expect(fetchImpl.mock.calls[1][1]?.body).toBe(
      JSON.stringify({
        org_id: 'org-1',
        kind: 'leave',
        idempotency_key: '11111111-1111-4111-8111-111111111111',
        fields: { leave_type: '事假', start_date: '2026-10-01', end_date: '2026-10-02', reason: '事假' }
      })
    )
    expect(fetchImpl).toHaveBeenCalledTimes(2)
  })

  it('decides through the inbox ticket', async () => {
    installApplet('inbox')
    const fetchImpl = vi.fn(() => jsonResponse({ ...approval, status: 'approved', actionable: false }))
    await expect(decideWorkItem(approval.id, 'approve', '同意', fetchImpl)).resolves.toMatchObject({
      status: 'approved'
    })
    expect(String(fetchImpl.mock.calls[0][0])).toBe(
      `${BASE}/api/applet-runtime/inbox/work-items/${approval.id}/decision`
    )
    expect(fetchImpl.mock.calls[0][1]?.body).toBe(JSON.stringify({ decision: 'approve', comment: '同意' }))
    expect(fetchImpl).toHaveBeenCalledTimes(1)
  })
})
