import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SWRConfig, useSWRConfig } from 'swr'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { writeCenterId } from '@renderer/data/memberOrgs'

import MailPage from '../MailPage'

const ORG = '0197e5a2-89f1-7d33-a11c-5f53d9b3af20'
const ID = '0197e5a2-89f1-7d33-a11c-5f53d9b3af10'
const fullPolicy = {
  id: ORG,
  name: 'Team',
  can_read_mail: true,
  can_create_mail: true,
  can_send_mail: true,
  can_update_mail: true,
  can_delete_mail: true,
  rules: ['read', 'create', 'send', 'update', 'delete'].map((action) => ({ action, subject: 'MailMessage' })),
  data_scope: { kind: 'all', department_ids: [] }
}
const message = {
  id: ID,
  folder: 'inbox',
  from_address: 'ada@mail.towork',
  from_name: 'Ada',
  to: ['roy@mail.towork'],
  cc: [],
  subject: 'Confidential',
  preview: 'Preview summary',
  body: 'Private body',
  unread: true,
  flagged: false,
  created_at: '2026-10-09T00:00:00Z',
  thread_id: ID,
  can_update: true,
  can_delete: true
}
let policy: Record<string, unknown>
let orgIndexStatus: number
let refresh: () => Promise<unknown>
function envelope(data: unknown) {
  return new Response(JSON.stringify({ success: true, code: 0, data }), { status: 200 })
}
function Authority() {
  const { mutate } = useSWRConfig()
  refresh = () => mutate('studio/member-orgs')
  return <MailPage />
}
function mount(network: (url: string, init?: RequestInit) => Promise<Response>) {
  vi.stubGlobal('fetch', (url: Parameters<typeof fetch>[0], init?: RequestInit) =>
    new URL(String(url)).pathname === '/api/member-orgs'
      ? Promise.resolve(
          orgIndexStatus === 200 ? envelope([policy]) : new Response('policy unavailable', { status: orgIndexStatus })
        )
      : network(String(url), init)
  )
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      <Authority />
    </SWRConfig>
  )
}
beforeEach(() => {
  policy = fullPolicy
  orgIndexStatus = 200
  writeCenterId(ORG)
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) }
  })
})
afterEach(() => {
  cleanup()
  writeCenterId('personal')
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

describe('Mail authorization', () => {
  it('withdraws cached mailbox access on a failed policy read and restores fresh messages after recovery', async () => {
    const user = userEvent.setup()
    let recovered = false
    mount(async (url) => {
      if (url.includes('/mailbox')) return envelope({ id: 'mb', address: 'roy@mail.towork', unread: 1 })
      const current = recovered ? { ...message, subject: 'Recovered subject', body: 'Recovered body' } : message
      if (url.includes(`/messages/${ID}`)) return envelope(current)
      return envelope({ items: [current], unread: 1 })
    })
    await user.click(await screen.findByRole('button', { name: /Confidential/ }))
    expect(await screen.findByText('Private body')).toBeInTheDocument()
    orgIndexStatus = 404
    await act(async () => {
      await refresh().catch(() => undefined)
    })
    expect(screen.queryByText('Private body')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /Confidential/ })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '写邮件' })).toBeDisabled()
    recovered = true
    orgIndexStatus = 200
    await act(async () => {
      await refresh()
    })
    await user.click(await screen.findByRole('button', { name: /Recovered subject/ }))
    expect(await screen.findByText('Recovered body')).toBeInTheDocument()
    expect(screen.queryByText('Private body')).not.toBeInTheDocument()
  })

  it('refuses message mutations when the response omits row capabilities', async () => {
    const user = userEvent.setup()
    mount(async (url) => {
      if (url.includes('/mailbox')) return envelope({ id: 'mb', address: 'roy@mail.towork', unread: 1 })
      if (url.includes(`/messages/${ID}`)) return envelope({ ...message, can_update: undefined, can_delete: undefined })
      return envelope({ items: [message], unread: 1 })
    })
    await user.click(await screen.findByRole('button', { name: /Confidential/ }))
    expect(await screen.findByRole('heading', { name: 'Confidential' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '回复' })).toBeEnabled()
    for (const name of ['标为红旗', '标为垃圾邮件', '删除']) {
      expect(screen.queryByRole('button', { name })).not.toBeInTheDocument()
    }
  })

  it('keeps readonly mail readable while refusing compose and state controls', async () => {
    policy = { ...fullPolicy, rules: [{ action: 'read', subject: 'MailMessage' }] }
    const user = userEvent.setup()
    const writes: string[] = []
    mount(async (url, init) => {
      expect(new URL(url).searchParams.get('org_id')).toBe(ORG)
      if (init?.method === 'POST') writes.push(String(init.body))
      if (url.includes('/mailbox')) return envelope({ id: 'mb', address: 'roy@mail.towork', unread: 1 })
      if (url.includes(`/messages/${ID}`)) return envelope(message)
      return envelope({ items: [message], unread: 1 })
    })
    await user.click(await screen.findByRole('button', { name: /Confidential/ }))
    expect(await screen.findByText('Private body')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '写邮件' })).toBeDisabled()
    for (const name of ['回复', '标为红旗', '标为已读', '标为垃圾邮件', '删除']) {
      expect(screen.queryByRole('button', { name })).not.toBeInTheDocument()
    }
    expect(writes).toEqual([])
  })

  it('requires server capabilities as well as grants and does not trust admin flags', async () => {
    const network = vi.fn(async () => envelope({ items: [message], unread: 1 }))
    policy = {
      ...fullPolicy,
      membership: 'admin',
      can_read_mail: undefined,
      can_create_mail: undefined,
      can_send_mail: undefined
    }
    mount(network)
    expect(await screen.findByText('你没有访问此团队邮箱的权限。')).toBeInTheDocument()
    const compose = screen.getByRole('button', { name: '写邮件' })
    expect(compose).toBeDisabled()
    expect(compose).toHaveAttribute('title', '你没有访问此团队邮箱的权限。')
    expect(compose.className).toContain('disabled:opacity-40')
    expect(screen.queryByText('从列表打开一封，或使用左上角的写邮件')).not.toBeInTheDocument()
    expect(network).not.toHaveBeenCalled()
    policy = { ...fullPolicy, membership: 'admin', rules: [] }
    await act(async () => {
      await refresh()
    })
    expect(screen.getByRole('button', { name: '写邮件' })).toBeDisabled()
    expect(network).not.toHaveBeenCalled()
  })

  it('ignores a pending open after revocation and restores only fresh scoped results', async () => {
    const user = userEvent.setup()
    let finish: ((response: Response) => void) | undefined
    let opens = 0
    mount(async (url) => {
      if (url.includes('/mailbox')) return envelope({ id: 'mb', address: 'roy@mail.towork', unread: 1 })
      if (url.includes(`/messages/${ID}`)) {
        if (++opens === 1)
          return new Promise<Response>((done) => {
            finish = done
          })
        return envelope({ ...message, subject: 'Fresh subject', body: 'Fresh body' })
      }
      return envelope({ items: [message], unread: 1 })
    })
    await user.click(await screen.findByRole('button', { name: /Confidential/ }))
    await waitFor(() => expect(finish).toBeDefined())
    policy = { ...fullPolicy, can_read_mail: false, can_create_mail: false, can_send_mail: false }
    await act(async () => {
      await refresh()
    })
    await act(async () => {
      finish?.(envelope(message))
    })
    expect(screen.queryByRole('heading', { name: 'Confidential' })).not.toBeInTheDocument()
    expect(screen.queryByText('roy@mail.towork')).not.toBeInTheDocument()
    policy = { ...fullPolicy, data_scope: { kind: 'self', department_ids: [] } }
    await act(async () => {
      await refresh()
    })
    await user.click(await screen.findByRole('button', { name: /Confidential/ }))
    expect(await screen.findByText('Fresh body')).toBeInTheDocument()
    expect(screen.queryByText('Private body')).not.toBeInTheDocument()
  })
})
