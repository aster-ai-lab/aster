import { afterEach, describe, expect, it, vi } from 'vitest'

import { writeCenterId } from '@renderer/data/memberOrgs'

import { listMail, listMailColleagues, loadMailbox, mailTurnNote, sendMail } from './mailApi'

const ORG = '0197e5a2-89f1-7d33-a11c-5f53d9b3af20'
const TOKEN = 'ab'.repeat(32)

function envelope(data: unknown, status = 200): Response {
  return new Response(JSON.stringify({ success: true, code: 0, message: 'ok', data }), { status })
}

const summary = {
  id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af10',
  folder: 'inbox',
  from_address: 'crates@mail.towork',
  from_name: 'crates.io',
  to: ['roy@mail.towork'],
  cc: [],
  subject: 'Successfully published a3s@0.22.1',
  preview: 'A new version',
  unread: true,
  flagged: false,
  thread_id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af11',
  created_at: '2026-10-06T11:14:11Z'
}

afterEach(() => {
  writeCenterId('personal')
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  localStorage.removeItem('mail.tobot.v1')
})

function installRuntime(): void {
  writeCenterId(ORG)
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: TOKEN }
  })
}

describe('mailApi', () => {
  it('loads the assigned mailbox from the member envelope', async () => {
    installRuntime()
    const fetchMock = vi.fn<(url: string, init?: RequestInit) => Promise<Response>>(async (url) => {
      expect(String(url)).toBe(`http://127.0.0.1:9/api/mail/mailbox?org_id=${ORG}`)
      return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 2 })
    })
    vi.stubGlobal('fetch', fetchMock)

    await expect(loadMailbox(ORG)).resolves.toEqual({ id: 'mb-1', address: 'roy@mail.towork', unread: 2 })
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({
      method: 'GET',
      headers: { authorization: `Bearer ${TOKEN}` }
    })
  })

  it('lists a folder and posts a composed message', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (init?.method === 'POST') {
          expect(text).toBe('http://127.0.0.1:9/api/mail/messages')
          expect(JSON.parse(String(init.body))).toEqual({
            to: ['crates@mail.towork'],
            cc: [],
            subject: 'hello',
            body: 'body',
            draft: false,
            org_id: ORG
          })
          return envelope({ ...summary, folder: 'sent', body: 'body', unread: false })
        }
        const parsed = new URL(text)
        expect(parsed.pathname).toBe('/api/mail/messages')
        expect(parsed.searchParams.get('folder')).toBe('unread')
        expect(parsed.searchParams.get('q')).toBe('a b')
        return envelope({ items: [summary], unread: 1 })
      })
    )

    const page = await listMail(ORG, 'unread', 'a b', 3)
    expect(page.items[0]?.subject).toBe('Successfully published a3s@0.22.1')
    expect(page.unread).toBe(1)
    const sent = await sendMail(ORG, {
      to: ['crates@mail.towork'],
      cc: [],
      subject: 'hello',
      body: 'body',
      draft: false
    })
    expect(sent.folder).toBe('sent')
    expect(sent.body).toBe('body')
  })

  it('writes a short mailbox note and stays quiet when the mailbox is unreachable', async () => {
    installRuntime()
    const fetchMock = vi.fn(async (url: string) => {
      const text = String(url)
      if (new URL(text).pathname === '/api/mail/mailbox')
        return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 1 })
      return envelope({ items: [summary], unread: 1 })
    })
    vi.stubGlobal('fetch', fetchMock)
    const denied = await mailTurnNote()
    expect(denied).toBe('ToBot is not allowed to manage this mailbox. Do not use member-mail tools or invent messages.')
    expect(fetchMock.mock.calls.some((call) => String(call[0]).includes('/api/mail/mailbox'))).toBe(false)

    localStorage.setItem(
      'mail.tobot.v1',
      JSON.stringify({ allow: true, list: true, read: true, folders: ['inbox', 'unread', 'flagged', 'sent'] })
    )
    const note = await mailTurnNote()
    expect(note).toContain('Mailbox roy@mail.towork')
    expect(note).toContain(`org_id=${ORG}`)
    expect(note).toContain('Unread 1')
    expect(note).toContain('Successfully published a3s@0.22.1')
    expect(note).toContain('list_mail')
    expect(note).toContain('read_mail is allowed')
    expect(note).toContain('Do not send, delete, or change messages.')
    expect(note.length).toBeLessThanOrEqual(480)

    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('no', { status: 500 }))
    )
    await expect(mailTurnNote()).resolves.toBe('')
  })

  it('keeps an image avatar and drops any other portrait value', async () => {
    installRuntime()
    const photo = 'data:image/webp;base64,AA'
    vi.stubGlobal(
      'fetch',
      vi.fn(async () =>
        envelope([
          { user_id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af21', name: '温和的风筝', position: '', avatar: ` ${photo} ` },
          { user_id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af22', name: '林书豪', position: '财务总监' },
          {
            user_id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af23',
            name: 'Leon',
            position: '',
            avatar: 'https://example.com/a.png'
          },
          {
            user_id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af24',
            name: 'RoyLin',
            position: '',
            avatar: `data:image/png;base64,${'A'.repeat(400_001)}`
          }
        ])
      )
    )

    await expect(listMailColleagues('0197e5a2-89f1-7d33-a11c-5f53d9b3af10')).resolves.toEqual([
      { userId: '0197e5a2-89f1-7d33-a11c-5f53d9b3af21', name: '温和的风筝', position: '', avatar: photo },
      { userId: '0197e5a2-89f1-7d33-a11c-5f53d9b3af22', name: '林书豪', position: '财务总监' },
      { userId: '0197e5a2-89f1-7d33-a11c-5f53d9b3af23', name: 'Leon', position: '' },
      { userId: '0197e5a2-89f1-7d33-a11c-5f53d9b3af24', name: 'RoyLin', position: '' }
    ])
  })
})
