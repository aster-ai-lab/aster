import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { createImApi, ImApiError } from '../imApi'
import { ORG } from './fakeImApi'

const TOKEN = 'a'.repeat(64)
const BASE = 'http://127.0.0.1:4567'

function envelope(data: unknown, status = 200) {
  return new Response(JSON.stringify({ code: status, message: 'ok', data }), { status })
}

describe('createImApi', () => {
  beforeEach(() => {
    ;(globalThis as Record<string, unknown>).__A3S_STUDIO_RUNTIME__ = { apiBaseUrl: BASE, token: TOKEN }
  })
  afterEach(() => {
    delete (globalThis as Record<string, unknown>).__A3S_STUDIO_RUNTIME__
  })

  it('calls the loopback IM lane with the runtime token, never a member token', async () => {
    const fetchImpl = vi.fn(async () =>
      envelope([
        {
          conversation_id: 'c-1',
          kind: 'direct',
          title: null,
          peer_user_id: 'u-2',
          member_count: 2,
          last_sequence: 4,
          last_read_sequence: 3,
          unread: 1,
          preview: {
            kind: 'text',
            message_id: 'm-4',
            sequence: 4,
            recalled: false,
            created_at: '2026-10-02T08:00:00Z',
            sender: { kind: 'member', id: 'u-2' },
            text: 'hi'
          },
          muted: false,
          pinned: true,
          updated_at: '2026-10-02T08:00:00Z'
        }
      ])
    )
    const api = createImApi(ORG, fetchImpl)
    const [entry] = await api.conversations()
    const [url, init] = fetchImpl.mock.calls[0] as unknown as [string, RequestInit]
    expect(url).toBe(`${BASE}/api/im/${ORG}/conversations?limit=100`)
    expect((init.headers as Record<string, string>).authorization).toBe(`Bearer ${TOKEN}`)
    expect(entry).toMatchObject({ id: 'c-1', peerUserId: 'u-2', memberCount: 2, unread: 1, pinned: true })
    expect(entry.preview).toMatchObject({ text: 'hi', sender: { kind: 'member', id: 'u-2' } })
  })

  it('sends text and attachment bodies in the server shape', async () => {
    const wire = {
      id: 'm-9',
      conversation_id: 'c-1',
      sequence: 9,
      sender: { kind: 'member', id: 'u-1' },
      recalled: false,
      created_at: '2026-10-02T08:00:00Z'
    }
    const fetchImpl = vi
      .fn()
      .mockResolvedValueOnce(envelope({ created: true, message: { ...wire, body: { kind: 'text', text: 'hi' } } }, 201))
      .mockResolvedValueOnce(
        envelope(
          {
            created: true,
            message: {
              ...wire,
              body: { kind: 'file', name: 'a.pdf', size_bytes: 3, object_key: 'k', mime: 'application/pdf' }
            }
          },
          201
        )
      )
    const api = createImApi(ORG, fetchImpl)
    await api.send('c-1', 'cli-1', { kind: 'text', text: 'hi' })
    const sent = await api.send('c-1', 'cli-2', {
      kind: 'file',
      attachment: { name: 'a.pdf', sizeBytes: 3, objectKey: 'k', mime: 'application/pdf' }
    })
    expect(JSON.parse(fetchImpl.mock.calls[0][1].body)).toEqual({ client_msg_id: 'cli-1', kind: 'text', text: 'hi' })
    expect(JSON.parse(fetchImpl.mock.calls[1][1].body)).toEqual({
      client_msg_id: 'cli-2',
      kind: 'file',
      name: 'a.pdf',
      size_bytes: 3,
      object_key: 'k',
      mime: 'application/pdf'
    })
    expect(sent.body).toEqual({
      kind: 'file',
      attachment: { name: 'a.pdf', sizeBytes: 3, objectKey: 'k', mime: 'application/pdf' }
    })
  })

  it('renames, transfers, sets roles, and reads presence without inventing a body', async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValueOnce(envelope({}))
      .mockResolvedValueOnce(envelope({}))
      .mockResolvedValueOnce(envelope({}))
      .mockResolvedValueOnce(envelope({ ok: true }))
      .mockResolvedValueOnce(envelope({ ok: true }))
      .mockResolvedValueOnce(envelope({ renewed: false }))
      .mockResolvedValueOnce(envelope(['u-2']))
    const api = createImApi(ORG, fetchImpl)
    await api.rename('c-1', '设计评审')
    await api.transferOwner('c-1', 'u-2')
    await api.setMemberRole('c-1', 'u-2', 'admin')
    await api.dissolve('c-1')
    await api.typing('c-1')
    expect(await api.renewPresence()).toBe(false)
    expect(await api.presence(['u-2', 'u-3'])).toEqual(['u-2'])
    expect(fetchImpl.mock.calls.map((call) => [call[0], call[1].method, JSON.parse(call[1].body ?? 'null')])).toEqual([
      [`${BASE}/api/im/${ORG}/conversations/c-1`, 'PUT', { title: '设计评审' }],
      [`${BASE}/api/im/${ORG}/conversations/c-1/owner`, 'PUT', { user_id: 'u-2' }],
      [`${BASE}/api/im/${ORG}/conversations/c-1/members/u-2/role`, 'PUT', { role: 'admin' }],
      [`${BASE}/api/im/${ORG}/conversations/c-1`, 'DELETE', null],
      [`${BASE}/api/im/${ORG}/conversations/c-1/typing`, 'POST', {}],
      [`${BASE}/api/im/${ORG}/presence`, 'PUT', {}],
      [`${BASE}/api/im/${ORG}/presence?user_ids=u-2,u-3`, 'GET', null]
    ])
  })

  it('uploads raw bytes with name and mime in the query', async () => {
    const fetchImpl = vi.fn(async () =>
      envelope({ object_key: 'im/o/u/x', name: '周报.pdf', size_bytes: 3, mime: 'application/pdf' }, 201)
    )
    const api = createImApi(ORG, fetchImpl)
    const file = new Blob(['abc'], { type: 'application/pdf' })
    const uploaded = await api.upload(file, '周报.pdf')
    const [url, init] = fetchImpl.mock.calls[0] as unknown as [string, RequestInit]
    expect(url).toBe(`${BASE}/api/im/${ORG}/attachments?name=${encodeURIComponent('周报.pdf')}&mime=application%2Fpdf`)
    expect(init.body).toBe(file)
    expect((init.headers as Record<string, string>)['content-type']).toBe('application/octet-stream')
    expect(uploaded).toEqual({ objectKey: 'im/o/u/x', name: '周报.pdf', sizeBytes: 3, mime: 'application/pdf' })
  })

  it('maps recalled messages to a contentless body and surfaces envelope errors', async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValueOnce(
        envelope([
          {
            id: 'm-1',
            conversation_id: 'c',
            sequence: 1,
            sender: { kind: 'member', id: 'u' },
            body: { kind: 'text' },
            recalled: true,
            created_at: '2026-10-02T08:00:00Z'
          }
        ])
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ code: 403, message: 'not a member', data: null }), { status: 403 })
      )
    const api = createImApi(ORG, fetchImpl)
    const [recalled] = await api.messages('c', 5)
    expect(fetchImpl.mock.calls[0][0]).toBe(`${BASE}/api/im/${ORG}/conversations/c/messages?before_sequence=5&limit=50`)
    expect(recalled.body).toEqual({ kind: 'recalled', original: 'text' })
    await expect(api.detail('c')).rejects.toEqual(new ImApiError('not a member', 403))
  })

  it('puts an empty object on the unread cursor', async () => {
    const fetchImpl = vi.fn(async () => envelope({ last_read_sequence: 2 }))
    const api = createImApi(ORG, fetchImpl)
    expect(await api.markUnread('c')).toBe(2)
    expect(fetchImpl.mock.calls[0]).toEqual([
      `${BASE}/api/im/${ORG}/conversations/c/unread-cursor`,
      expect.objectContaining({ method: 'PUT', body: '{}' })
    ])
  })

  it('fails closed without a shell runtime', async () => {
    delete (globalThis as Record<string, unknown>).__A3S_STUDIO_RUNTIME__
    const fetchImpl = vi.fn()
    await expect(createImApi(ORG, fetchImpl).me()).rejects.toBeInstanceOf(ImApiError)
    expect(fetchImpl).not.toHaveBeenCalled()
  })
})
