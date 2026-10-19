import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SWRConfig, useSWRConfig } from 'swr'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { writeCenterId } from '@renderer/data/memberOrgs'

import SchedulePage from '../SchedulePage'

vi.mock('@a3s-lab/ui', async () => vi.importActual('@a3s-lab/ui'))

const orgId = '11111111-1111-4111-8111-111111111111'
let flags: Record<string, boolean>
let rules: { action: string; subject: string }[]
let requests: { method: string; path: string; body: unknown }[]
let restoreFetch: () => void
let revalidate: () => Promise<unknown>
let deferredList: Promise<Response> | undefined
let deferredWrite: Promise<Response> | undefined
let events: Record<string, unknown>[]
let orgIndexStatus: number

function envelope(data: unknown) {
  return new Response(JSON.stringify({ code: 0, data }))
}

beforeEach(() => {
  flags = { can_read_schedule: true, can_create_schedule: true, can_update_schedule: true, can_delete_schedule: true }
  rules = [{ action: 'read', subject: 'ScheduleEvent' }]
  requests = []
  deferredList = undefined
  deferredWrite = undefined
  orgIndexStatus = 200
  const start = new Date()
  start.setHours(13, 0, 0, 0)
  events = [
    {
      id: 'event-1',
      title: '权限验证日程',
      starts_at: start.toISOString(),
      ends_at: new Date(start.getTime() + 3600000).toISOString(),
      can_update: true,
      can_delete: true
    }
  ]
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32) }
  })
  const fetchSpy = vi.spyOn(globalThis, 'fetch').mockImplementation(async (url, init) => {
    const path = new URL(String(url)).pathname
    const method = init?.method ?? 'GET'
    const body = init?.body ? JSON.parse(String(init.body)) : null
    requests.push({ method, path, body })
    if (path === '/api/member-orgs' && orgIndexStatus !== 200)
      return new Response('policy unavailable', { status: orgIndexStatus })
    if (path === '/api/member-orgs')
      return envelope([
        { id: orgId, name: '日程权限团队', logo: '', membership: 'admin', creator: false, ...flags, rules }
      ])
    if (path === '/api/member/me') return envelope({ display_name: '本人', phone: '' })
    if (path.endsWith('/schedule/events')) {
      if (method === 'GET') return deferredList ?? envelope(events)
      if (deferredWrite) return deferredWrite
      const event = { ...(body as object), id: 'created-event', can_update: true, can_delete: true }
      events.push(event)
      return envelope(event)
    }
    if (method === 'DELETE') {
      events = []
      return envelope({ ok: true })
    }
    return envelope([])
  })
  restoreFetch = () => fetchSpy.mockRestore()
  writeCenterId(orgId)
})

afterEach(() => {
  restoreFetch()
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
  localStorage.removeItem('a3s.org.center')
})

function PolicyPage() {
  const { mutate } = useSWRConfig()
  revalidate = () => mutate('studio/member-orgs')
  return <SchedulePage />
}

function openPage() {
  render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      <PolicyPage />
    </SWRConfig>
  )
}

describe('schedule authorization through the real member client', () => {
  it('withdraws cached calendar authority after a failed policy read and restores only fresh events', async () => {
    const user = userEvent.setup()
    rules.push({ action: 'create', subject: 'ScheduleEvent' }, { action: 'update', subject: 'ScheduleEvent' })
    openPage()
    await user.click(await screen.findByRole('button', { name: /权限验证日程/ }))
    expect(screen.getByRole('button', { name: '保存' })).toBeEnabled()
    orgIndexStatus = 404
    await act(async () => {
      await revalidate().catch(() => undefined)
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /权限验证日程/ })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '创建日程' })).not.toBeInTheDocument()
    events = [{ ...events[0], title: '恢复后的日程' }]
    orgIndexStatus = 200
    await act(async () => {
      await revalidate()
    })
    expect(await screen.findByRole('button', { name: /恢复后的日程/ })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /权限验证日程/ })).not.toBeInTheDocument()
  })

  it('requires functional grants, server data scope and per-event mutation capabilities', async () => {
    const user = userEvent.setup()
    flags.can_read_schedule = false
    openPage()
    await screen.findByText('你没有查看此团队日程的权限。')
    expect(requests.some((request) => request.path.endsWith('/schedule/events'))).toBe(false)
    flags.can_read_schedule = true
    await act(async () => {
      await revalidate()
    })
    await user.click(await screen.findByRole('button', { name: /权限验证日程/ }))
    expect(screen.queryByRole('button', { name: '保存' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '删除' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '创建日程' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '取消' }))
    rules.push({ action: 'update', subject: 'ScheduleEvent' })
    flags.can_update_schedule = false
    await act(async () => {
      await revalidate()
    })
    await user.click(screen.getByRole('button', { name: /权限验证日程/ }))
    expect(screen.queryByRole('button', { name: '保存' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '取消' }))
    events[0].can_update = undefined
    flags.can_update_schedule = true
    await act(async () => {
      await revalidate()
    })
    await user.click(screen.getAllByRole('button', { name: '向后' })[0])
    await user.click(screen.getAllByRole('button', { name: '向前' })[0])
    await user.click(await screen.findByRole('button', { name: /权限验证日程/ }))
    expect(screen.queryByRole('button', { name: '保存' })).not.toBeInTheDocument()
    expect(requests.filter((request) => request.method !== 'GET' && request.path.includes('/schedule/events'))).toEqual(
      []
    )
  })

  it('creates with a create grant and deletes with a separate delete grant without enabling updates', async () => {
    const user = userEvent.setup()
    rules.push({ action: 'create', subject: 'ScheduleEvent' }, { action: 'delete', subject: 'ScheduleEvent' })
    openPage()
    await user.click(await screen.findByRole('button', { name: '创建日程' }))
    await user.type(screen.getByPlaceholderText('添加主题'), '授权新建')
    await user.click(screen.getByRole('button', { name: '保存' }))
    await screen.findByRole('button', { name: /授权新建/ })
    expect(
      requests.find((request) => request.method === 'POST' && request.path.endsWith('/schedule/events'))?.body
    ).toMatchObject({ title: '授权新建' })
    await user.click(screen.getByRole('button', { name: /权限验证日程/ }))
    expect(screen.queryByRole('button', { name: '保存' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '删除' }))
    await user.click(
      within(screen.getByRole('dialog', { name: '删除这条日程？' })).getByRole('button', { name: '删除' })
    )
    await waitFor(() => expect(screen.queryByRole('button', { name: /权限验证日程/ })).not.toBeInTheDocument())
    expect(
      requests.some((request) => request.method === 'DELETE' && request.path.endsWith('/schedule/events/event-1'))
    ).toBe(true)
    expect(requests.some((request) => request.method === 'PUT')).toBe(false)
  })

  it('discards revoked drafts and ignores a delayed failed save after permission revocation', async () => {
    const user = userEvent.setup()
    rules.push({ action: 'create', subject: 'ScheduleEvent' })
    let finishWrite: (response: Response) => void = () => undefined
    deferredWrite = new Promise((resolve) => {
      finishWrite = resolve
    })
    openPage()
    await user.click(await screen.findByRole('button', { name: '创建日程' }))
    await user.type(screen.getByPlaceholderText('添加主题'), '撤权前草稿')
    await user.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() =>
      expect(requests.some((request) => request.method === 'POST' && request.path.endsWith('/schedule/events'))).toBe(
        true
      )
    )
    flags.can_create_schedule = false
    await act(async () => {
      await revalidate()
    })
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    await act(async () => {
      finishWrite(new Response(JSON.stringify({ message: 'not found' }), { status: 404 }))
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    flags.can_create_schedule = true
    await act(async () => {
      await revalidate()
    })
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '创建日程' }))
    expect(screen.getByPlaceholderText('添加主题')).toHaveValue('')
  })

  it('ignores an older event response after read permission is revoked', async () => {
    let finishList: (response: Response) => void = () => undefined
    deferredList = new Promise((resolve) => {
      finishList = resolve
    })
    openPage()
    await waitFor(() => expect(requests.some((request) => request.path.endsWith('/schedule/events'))).toBe(true))
    flags.can_read_schedule = false
    await act(async () => {
      await revalidate()
    })
    await act(async () => {
      finishList(envelope(events))
    })
    expect(screen.queryByRole('button', { name: /权限验证日程/ })).not.toBeInTheDocument()
    expect(screen.getByText('你没有查看此团队日程的权限。')).toBeInTheDocument()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })
})
