import { afterEach, describe, expect, it, vi } from 'vitest'

import { writeCenterId } from '@renderer/data/memberOrgs'

import {
  loadWorkToBotPolicy,
  normalizeWorkToBotPolicy,
  saveWorkToBotPolicy,
  workToBotNote,
  workTurnNote
} from './workToBot'

const ORG = '0197e5a2-89f1-7d33-a11c-5f53d9b3af20'

afterEach(() => {
  localStorage.removeItem('work.tobot.v1')
  writeCenterId('personal')
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

describe('workToBot', () => {
  it('stays closed until the member allows it, and keeps every task kind', () => {
    expect(normalizeWorkToBotPolicy(null)).toEqual({
      allow: false,
      list: true,
      read: false,
      kinds: ['join_request', 'approval', 'leave', 'travel', 'expense', 'meeting', 'seal', 'notice']
    })
    expect(normalizeWorkToBotPolicy({ allow: true, kinds: ['nope', 'leave', 'leave'] })).toEqual({
      allow: true,
      list: true,
      read: false,
      kinds: ['leave']
    })
    const denied = workToBotNote(normalizeWorkToBotPolicy({ allow: true, list: false }), ORG, ['采购'])
    expect(denied).toBe('ToBot is not allowed to manage these tasks. Do not use member-work tools or invent tasks.')
    const open = workToBotNote(normalizeWorkToBotPolicy({ allow: true, kinds: ['seal'] }), ORG, ['用印'])
    expect(open).toContain(`org_id=${ORG}`)
    expect(open).toContain('kinds seal')
    expect(open).toContain('用印')
    expect(open).toContain('Do not approve, reject, cancel, or create tasks.')
    expect(open).not.toContain('join_request')
  })

  it('remembers a narrowed grant and lists pending titles only after it is open', async () => {
    saveWorkToBotPolicy({ allow: true, list: true, read: true, kinds: ['notice', 'notice', 'meeting'] })
    expect(loadWorkToBotPolicy()).toEqual({
      allow: true,
      list: true,
      read: true,
      kinds: ['notice', 'meeting']
    })

    localStorage.setItem('work.tobot.v1', JSON.stringify({ allow: false, list: true, read: false, kinds: ['leave'] }))
    const fetchMock = vi.fn(async () => new Response('{}', { status: 500 }))
    vi.stubGlobal('fetch', fetchMock)
    await expect(workTurnNote()).resolves.toBe(
      'ToBot is not allowed to manage these tasks. Do not use member-work tools or invent tasks.'
    )
    expect(fetchMock).not.toHaveBeenCalled()

    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) }
    })
    writeCenterId(ORG)
    localStorage.setItem(
      'work.tobot.v1',
      JSON.stringify({ allow: true, list: true, read: false, kinds: ['approval', 'leave'] })
    )
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        const text = String(url)
        if (new URL(text).pathname === '/api/agent-plane') {
          return new Response(JSON.stringify({ baseUrl: 'http://127.0.0.1:9' }), { status: 200 })
        }
        if (text.includes('/mcp/member-work/policy')) return new Response('{}', { status: 200 })
        return new Response(
          JSON.stringify({
            success: true,
            code: 0,
            data: [
              {
                id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af10',
                org_id: ORG,
                title: '采购',
                kind: 'approval',
                status: 'pending'
              },
              {
                id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af11',
                org_id: ORG,
                title: '事假',
                kind: 'leave',
                status: 'pending'
              },
              {
                id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af12',
                org_id: ORG,
                title: '公告',
                kind: 'notice',
                status: 'pending'
              }
            ]
          }),
          { status: 200 }
        )
      })
    )
    const note = await workTurnNote()
    expect(note).toContain('采购')
    expect(note).toContain('事假')
    expect(note).not.toContain('公告')
    expect(note).toContain('kinds approval,leave')
    expect(note).toContain('read_work is not allowed')
  })
})
