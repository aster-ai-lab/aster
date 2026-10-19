import { cleanup, fireEvent, render as renderComponent, screen, waitFor, within } from '@testing-library/react'
import type { ReactElement } from 'react'
import { SWRConfig } from 'swr'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const state = vi.hoisted(() => ({ center: 'org-1' }))

vi.mock('@renderer/data/memberOrgs', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@renderer/data/memberOrgs')>()),
  PERSONAL_CENTER: 'personal',
  readCenterId: () => state.center,
  subscribeCenter: () => () => undefined,
  isSessionDenied: () => false,
  returnToLogin: vi.fn(),
  readMemberAccount: vi.fn(async () => ({ displayName: '林志霄', phone: '' })),
  listMemberOrgs: vi.fn(async () => [
    {
      id: 'org-1',
      name: 'Team',
      logo: '',
      creator: true,
      canReadSchedule: true,
      canCreateSchedule: true,
      canUpdateSchedule: true,
      canDeleteSchedule: true
    }
  ])
}))

vi.mock('@renderer/services/toast', () => ({
  toast: { success: vi.fn(), error: vi.fn() }
}))

vi.mock('../scheduleApi', () => ({
  listScheduleEvents: vi.fn(),
  createScheduleEvent: vi.fn(),
  updateScheduleEvent: vi.fn(),
  deleteScheduleEvent: vi.fn()
}))

vi.mock('../../org/orgApi', () => ({
  listOrgMembers: vi.fn(async () => []),
  listOrgDepartments: vi.fn(async () => [])
}))

import { listOrgDepartments, listOrgMembers } from '../../org/orgApi'
import { createScheduleEvent, deleteScheduleEvent, listScheduleEvents } from '../scheduleApi'
import SchedulePage from '../SchedulePage'

const listMock = vi.mocked(listScheduleEvents)
const createMock = vi.mocked(createScheduleEvent)
const deleteMock = vi.mocked(deleteScheduleEvent)
const membersMock = vi.mocked(listOrgMembers)
const departmentsMock = vi.mocked(listOrgDepartments)

function render(ui: ReactElement) {
  return renderComponent(ui, {
    wrapper: ({ children }) => (
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>{children}</SWRConfig>
    )
  })
}

function eventToday() {
  const start = new Date()
  start.setHours(13, 0, 0, 0)
  const end = new Date(start.getTime() + 2 * 60 * 60 * 1000)
  return {
    id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af10',
    can_update: true,
    can_delete: true,
    title: '周例会',
    location: '9F',
    note: '',
    minutes: '',
    all_day: false,
    time_zone: 'UTC',
    utc_offset_minutes: 0,
    repeat_rule: 'none',
    visibility: 'default' as const,
    availability: 'busy' as const,
    guest_permission: 'invite_notes' as const,
    reminders: [5],
    guests: [],
    starts_at: start.toISOString(),
    ends_at: end.toISOString()
  }
}

describe('SchedulePage', () => {
  beforeEach(() => {
    state.center = 'org-1'
    listMock.mockReset()
    createMock.mockReset()
    deleteMock.mockReset()
    membersMock.mockReset()
    departmentsMock.mockReset()
    listMock.mockResolvedValue([])
    membersMock.mockResolvedValue([])
    departmentsMock.mockResolvedValue([])
  })

  afterEach(() => {
    cleanup()
  })

  it('shows the team empty state and does not load events from the personal center', async () => {
    state.center = 'personal'
    render(<SchedulePage />)
    expect(await screen.findByText('先选择一个团队，再查看你在这个团队里的日程。')).toBeTruthy()
    expect(listMock).not.toHaveBeenCalled()
    expect(screen.queryByRole('button', { name: '创建日程' })).toBeNull()
  })

  it('paints a loaded event and saves a new one through the member schedule api', async () => {
    const existing = eventToday()
    listMock.mockResolvedValue([existing])
    createMock.mockResolvedValue({ ...existing, id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af11', title: '新日程' })
    render(<SchedulePage />)

    expect(await screen.findByRole('button', { name: /周例会/ })).toBeTruthy()
    expect(listMock).toHaveBeenCalledWith('org-1', expect.any(Date), expect.any(Date))

    fireEvent.click(screen.getByRole('button', { name: '创建日程' }))
    fireEvent.change(screen.getByPlaceholderText('添加主题'), { target: { value: '新日程' } })
    fireEvent.change(screen.getByLabelText('地点'), { target: { value: '9F' } })
    fireEvent.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(createMock).toHaveBeenCalledWith('org-1', expect.objectContaining({ title: '新日程', location: '9F' }))
    })
  })

  it('picks a day and a time from the in-page calendar', async () => {
    render(<SchedulePage />)
    const createButton = await screen.findByRole('button', { name: '创建日程' })
    expect(createButton.querySelector('svg')?.getAttribute('stroke')).toBe('#ffffff')
    fireEvent.click(createButton)
    expect(document.querySelector('input[type="date"]')).toBeNull()
    expect(document.querySelector('input[type="time"]')).toBeNull()
    expect(document.querySelector('input[type="datetime-local"]')).toBeNull()
    expect(document.querySelector('[data-ui="schedule.form.calendar"]')).toBeNull()
    expect(screen.getByLabelText('地点')).toBeTruthy()

    fireEvent.click(document.querySelector('[data-ui="schedule.form.start-date"]')!)
    const calendar = document.querySelector('[data-ui="schedule.form.calendar"]')
    expect(calendar).toBeTruthy()
    const day = calendar?.querySelector('td[data-day$="-15"]:not([data-outside]) button')
    expect(day).toBeTruthy()
    fireEvent.click(day!)
    expect(document.querySelector('[data-ui="schedule.form.start-date"]')?.textContent).toContain('15日')
    expect(document.querySelector('[data-ui="schedule.form.calendar"]')).toBeNull()

    fireEvent.click(document.querySelector('[data-ui="schedule.form.start-time"]')!)
    fireEvent.click(screen.getByRole('button', { name: '09:15' }))
    expect(document.querySelector('[data-ui="schedule.form.start-time"]')?.textContent).toBe('09:15')

    const before = document.querySelector('[data-ui="schedule.form.start-date"]')?.textContent
    expect(document.querySelector('[data-ui="schedule.draft.block"]')).toBeTruthy()
    fireEvent.click(document.querySelector('[data-ui="schedule.draft.next"]')!)
    expect(document.querySelector('[data-ui="schedule.form.start-date"]')?.textContent).not.toBe(before)
  })

  it('shows a real failure and retries the same range', async () => {
    listMock.mockRejectedValueOnce(new Error('schedule down')).mockResolvedValueOnce([])
    render(<SchedulePage />)
    expect(await screen.findByText('schedule down')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: '重试' }))
    await waitFor(() => expect(listMock).toHaveBeenCalledTimes(2))
  })

  it('hides an upstream route string and shows the schedule failure', async () => {
    listMock.mockRejectedValueOnce(new Error('GET /v1/api/org/members/orgs/org-1/schedule/events'))
    render(<SchedulePage />)
    expect(await screen.findByText('日程没有加载成功。')).toBeTruthy()
  })

  it('deletes the open event after confirmation', async () => {
    const existing = eventToday()
    listMock.mockResolvedValue([existing])
    deleteMock.mockResolvedValue(undefined)
    render(<SchedulePage />)
    fireEvent.click(await screen.findByRole('button', { name: /周例会/ }))
    fireEvent.click(screen.getByRole('button', { name: '删除' }))
    fireEvent.click(screen.getAllByRole('button', { name: '删除' }).at(-1)!)
    await waitFor(() => expect(deleteMock).toHaveBeenCalledWith('org-1', existing.id))
  })

  it('keeps the calendar still while another day loads', async () => {
    const existing = eventToday()
    let release: (rows: []) => void = () => undefined
    listMock.mockResolvedValueOnce([existing])
    listMock.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          release = resolve
        })
    )
    render(<SchedulePage />)
    expect(await screen.findByRole('button', { name: /周例会/ })).toBeTruthy()
    await waitFor(() => expect(screen.queryByText('正在加载')).toBeNull())

    const before = document.querySelector('[data-ui="schedule.view"] header p')?.textContent
    const grid = document.querySelector('[data-ui="schedule.grid"]') as HTMLElement
    const shown = new Set(
      [...grid.querySelectorAll('button')].map((node) => node.textContent?.match(/(\d{1,2})\s*$/)?.[1] ?? '')
    )
    const mini = document.querySelector('[data-ui="schedule.mini"]') as HTMLElement
    const day = within(mini)
      .getAllByRole('button')
      .find(
        (button) => /^\d{1,2}$/.test(button.textContent?.trim() ?? '') && !shown.has(button.textContent?.trim() ?? '')
      )
    expect(day).toBeTruthy()
    fireEvent.click(day!)

    expect(screen.queryByText('正在加载')).toBeNull()
    expect(screen.queryByRole('button', { name: /周例会/ })).toBeNull()
    expect(screen.queryByText('这段时间没有日程。')).toBeNull()
    expect(document.querySelector('[data-ui="schedule.view"] header p')?.textContent).not.toBe(before)

    release([])
    expect(await screen.findByText('这段时间没有日程。')).toBeTruthy()
    expect(screen.queryByText('正在加载')).toBeNull()
  })

  it('opens the reminder list outside the dialog scroller', async () => {
    render(<SchedulePage />)
    fireEvent.click(await screen.findByRole('button', { name: '创建日程' }))
    const before = document.querySelector('[data-ui="schedule.form.body"]')?.scrollHeight ?? 0
    fireEvent.click(document.querySelector('[data-ui="schedule.form.reminder"]')!)
    const menu = document.querySelector('[data-ui="schedule.form.menu"]')
    const body = document.querySelector('[data-ui="schedule.form.body"]')
    expect(menu?.className).toContain('fixed')
    expect(menu?.className).toContain('pointer-events-auto')
    expect(body?.contains(menu ?? null)).toBe(false)
    expect(body?.scrollHeight).toBe(before)
    expect(screen.getByRole('option', { name: '提前 2 天' })).toBeTruthy()
    fireEvent.click(screen.getByRole('option', { name: '提前 1 小时' }))
    expect(document.querySelector('[data-ui="schedule.form.menu"]')).toBeNull()
    expect(document.querySelector('[data-ui="schedule.form.reminder"]')?.textContent).toContain('提前 1 小时')
  })

  it('saves a daily repeat and the default reminder', async () => {
    createMock.mockResolvedValue(eventToday())
    render(<SchedulePage />)
    fireEvent.click(await screen.findByRole('button', { name: '创建日程' }))
    fireEvent.change(screen.getByPlaceholderText('添加主题'), { target: { value: '站会' } })
    fireEvent.click(document.querySelector('[data-ui="schedule.form.repeat"]')!)
    fireEvent.click(screen.getByRole('option', { name: '每天' }))
    fireEvent.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() => {
      expect(createMock).toHaveBeenCalledWith(
        'org-1',
        expect.objectContaining({
          title: '站会',
          repeat_rule: 'daily',
          reminders: [5],
          guest_permission: 'invite_notes'
        })
      )
    })
  })

  it('nests departments and shows a member photo', async () => {
    const avatar = 'data:image/png;base64,QQ'
    membersMock.mockResolvedValue([
      {
        id: 'member-roy',
        member_id: 'global-member-roy',
        name: 'RoyLin',
        email: 'roy@example.com',
        phone: '18700001111',
        avatar,
        status: 'active',
        admin: false,
        creator: false,
        department_id: 'dept-finance',
        joined_at: '2026-01-01T00:00:00Z',
        position_id: null,
        position_name: null,
        role_code: 'member',
        role_name: '成员',
        role_built_in: true
      },
      {
        id: 'member-wang',
        member_id: 'global-member-wang',
        name: '王敏',
        email: 'wang@example.com',
        phone: null,
        status: 'active',
        admin: false,
        creator: false,
        department_id: 'dept-rd',
        joined_at: '2026-01-01T00:00:00Z',
        position_id: null,
        position_name: null,
        role_code: 'member',
        role_name: '成员',
        role_built_in: true
      },
      {
        id: 'member-left',
        member_id: 'global-member-left',
        name: 'Left',
        email: 'left@example.com',
        phone: null,
        status: 'disabled',
        admin: false,
        creator: false,
        department_id: 'dept-finance',
        joined_at: '2026-01-01T00:00:00Z',
        position_id: null,
        position_name: null,
        role_code: 'member',
        role_name: '成员',
        role_built_in: true
      }
    ])
    departmentsMock.mockResolvedValue([
      { id: 'dept-root', parent_id: null, name: '全员', sort_order: 0 },
      { id: 'dept-finance', parent_id: 'dept-root', name: '财务部', sort_order: 1 },
      { id: 'dept-rd', parent_id: 'dept-root', name: '研发部', sort_order: 2 }
    ])
    createMock.mockResolvedValue(eventToday())
    render(<SchedulePage />)
    fireEvent.click(await screen.findByRole('button', { name: '创建日程' }))
    expect(membersMock).not.toHaveBeenCalled()

    fireEvent.click(document.querySelector('[data-ui="schedule.form.guests"]')!)
    const picker = await screen.findByRole('dialog', { name: '添加联系人、群或邮箱' })
    const finance = within(picker).getByRole('checkbox', { name: '财务部' })
    expect(finance.closest('[data-depth]')?.getAttribute('data-depth')).toBe('1')
    const roy = within(picker).getByRole('checkbox', { name: 'RoyLin' })
    expect(roy.closest('[data-depth]')?.getAttribute('data-depth')).toBe('2')
    expect(within(picker).queryByRole('checkbox', { name: 'Left' })).toBeNull()
    const photo = within(picker).getByRole('img', { name: 'RoyLin' })
    expect(photo.getAttribute('data-kind')).toBe('image')
    expect(photo.querySelector('img')?.getAttribute('src')).toBe(avatar)
    expect(within(picker).getByRole('img', { name: '王敏' }).getAttribute('data-kind')).toBe('text')

    fireEvent.click(finance)
    expect(roy).toBeChecked()
    expect(within(picker).getByRole('checkbox', { name: '王敏' })).not.toBeChecked()
    expect(picker.getAttribute('portalcontainer')).toBeNull()
    expect(document.querySelector('[data-ui="schedule.dialog"]')?.contains(picker)).toBe(false)
    fireEvent.click(within(picker).getByRole('button', { name: '确认' }))
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: '添加联系人、群或邮箱' })).toBeNull()
    })
    const trigger = document.querySelector('[data-ui="schedule.form.guests"]')
    expect(trigger?.textContent).toContain('RoyLin')
    expect(trigger?.textContent).not.toContain('王敏')
    expect(trigger?.querySelector('img')?.getAttribute('src')).toBe(avatar)

    membersMock.mockResolvedValue([
      {
        id: 'member-roy',
        member_id: 'global-member-roy',
        name: 'RoyLin',
        email: 'roy@example.com',
        phone: '18700001111',
        avatar,
        status: 'active',
        admin: false,
        creator: false,
        department_id: 'dept-finance',
        joined_at: '2026-01-01T00:00:00Z',
        position_id: null,
        position_name: null,
        role_code: 'member',
        role_name: '成员',
        role_built_in: true
      },
      {
        id: 'member-new',
        member_id: 'global-member-new',
        name: '赵六',
        email: 'zhao@example.com',
        phone: null,
        status: 'active',
        admin: false,
        creator: false,
        department_id: 'dept-finance',
        joined_at: '2026-01-01T00:00:00Z',
        position_id: null,
        position_name: null,
        role_code: 'member',
        role_name: '成员',
        role_built_in: true
      }
    ])
    fireEvent.click(document.querySelector('[data-ui="schedule.form.guests"]')!)
    const again = await screen.findByRole('dialog', { name: '添加联系人、群或邮箱' })
    expect(within(again).getByRole('checkbox', { name: '赵六' })).toBeTruthy()
    fireEvent.click(within(again).getByRole('button', { name: '关闭' }))
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: '添加联系人、群或邮箱' })).toBeNull()
    })

    fireEvent.change(screen.getByPlaceholderText('添加主题'), { target: { value: '评审' } })
    fireEvent.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() => {
      expect(createMock).toHaveBeenCalledWith(
        'org-1',
        expect.objectContaining({
          title: '评审',
          guests: [{ member_id: 'global-member-roy' }]
        })
      )
    })
  })
})
