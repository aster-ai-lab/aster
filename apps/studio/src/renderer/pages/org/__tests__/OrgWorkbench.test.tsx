import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { WORKBENCH_APPLET_IDS } from '@renderer/pages/agents/homeApplets'
import { openWorkbenchAppletWindow } from '@renderer/pages/agents/openWorkbenchAppletWindow'
import { installCapability, listCapabilities } from '@renderer/pages/capabilities/capabilitiesData'

import { OrgWorkbench } from '../OrgWorkbench'

const orgId = '11111111-1111-4111-8111-111111111111'

vi.mock('@renderer/pages/agents/openWorkbenchAppletWindow', () => ({
  openWorkbenchAppletWindow: vi.fn(() => Promise.resolve(true))
}))

vi.mock('@renderer/pages/capabilities/capabilitiesData', () => ({
  listCapabilities: vi.fn(async () => ({ items: [] })),
  installCapability: vi.fn(async () => undefined)
}))

afterEach(() => {
  localStorage.removeItem(`org.workbench.favorites.v1:${orgId}`)
  vi.mocked(openWorkbenchAppletWindow).mockClear()
  vi.mocked(installCapability).mockClear()
  vi.unstubAllGlobals()
})

describe('team workbench', () => {
  it('shows favorites and the people, finance, and administration sections', () => {
    render(<OrgWorkbench orgId={orgId} />)
    expect(screen.getByRole('heading', { name: '我的常用' })).toBeInTheDocument()
    expect(screen.getByRole('region', { name: '人事专区' })).toBeInTheDocument()
    expect(screen.getByRole('region', { name: '财务专区' })).toBeInTheDocument()
    expect(screen.getByRole('region', { name: '研发专区' })).toBeInTheDocument()
    expect(screen.getByRole('region', { name: '行政专区' })).toBeInTheDocument()
    const people = screen.getByRole('region', { name: '人事专区' })
    expect(within(people).getByRole('button', { name: '请假' })).toBeInTheDocument()
    expect(within(people).getByRole('button', { name: '转正' })).toBeInTheDocument()
    expect(within(people).getByRole('button', { name: '打卡' })).toBeInTheDocument()
    const finance = screen.getByRole('region', { name: '财务专区' })
    expect(within(finance).getByRole('button', { name: '报销' })).toBeInTheDocument()
    expect(within(finance).getByRole('button', { name: '出差' })).toBeInTheDocument()
    const research = screen.getByRole('region', { name: '研发专区' })
    expect(within(research).getByRole('button', { name: 'AI 资源' })).toBeInTheDocument()
    const admin = screen.getByRole('region', { name: '行政专区' })
    expect(within(admin).getByRole('button', { name: '审批' })).toBeInTheDocument()
    expect(within(admin).getByRole('button', { name: '会议' })).toBeInTheDocument()
    expect(within(admin).getByRole('button', { name: '用印' })).toBeInTheDocument()
    expect(screen.getAllByRole('button', { name: '审批' }).length).toBeGreaterThan(1)
    expect([...WORKBENCH_APPLET_IDS]).toEqual(['approval', 'leave', 'travel', 'expense', 'meeting', 'seal'])
  })

  it('installs the office AIX applet and opens its window', async () => {
    render(<OrgWorkbench orgId={orgId} />)
    const user = userEvent.setup()
    await user.click(screen.getAllByRole('button', { name: '审批' })[0])
    await waitFor(() => expect(installCapability).toHaveBeenCalledWith('a3s-approval', expect.any(Function)))
    await waitFor(() => expect(openWorkbenchAppletWindow).toHaveBeenCalledWith('approval', '审批'))
    expect(listCapabilities).toHaveBeenCalled()
  })

  it('opens a favorite from a tap and does not capture the pointer', async () => {
    render(<OrgWorkbench orgId={orgId} />)
    const slot = screen.getByRole('button', { name: '待办' }).closest('[data-applet]') as HTMLElement
    const capture = vi.fn()
    slot.setPointerCapture = capture
    fireEvent.pointerDown(slot, { clientX: 10, clientY: 10, button: 0, pointerId: 1 })
    fireEvent.pointerUp(slot, { clientX: 12, clientY: 11, pointerId: 1 })
    expect(capture).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: '待办' }))
    await waitFor(() => expect(openWorkbenchAppletWindow).toHaveBeenCalledWith('inbox', '待办'))
  })

  it('reorders favorites by dragging a card', () => {
    render(<OrgWorkbench orgId={orgId} />)
    expect(screen.queryByRole('button', { name: '排序' })).not.toBeInTheDocument()
    const slot = screen.getByRole('button', { name: '待办' }).closest('[data-applet]') as HTMLElement
    const cards = [...slot.parentElement!.querySelectorAll('[data-applet]')] as HTMLElement[]
    cards.forEach((node, index) => {
      node.getBoundingClientRect = () =>
        ({
          x: index * 120,
          y: 0,
          left: index * 120,
          top: 0,
          width: 100,
          height: 72,
          right: index * 120 + 100,
          bottom: 72,
          toJSON: () => ({})
        }) as DOMRect
    })
    class TestPointerEvent extends MouseEvent {
      pointerId: number
      constructor(type: string, init: MouseEventInit & { pointerId?: number } = {}) {
        super(type, init)
        this.pointerId = init.pointerId ?? 1
      }
    }
    vi.stubGlobal('PointerEvent', TestPointerEvent)
    fireEvent.pointerDown(slot, { clientX: 10, clientY: 10, button: 0, pointerId: 1 })
    fireEvent.pointerMove(slot, { clientX: 290, clientY: 10, pointerId: 1 })
    const ghost = document.querySelector('[data-ui="org.workbench.favorite.drag"]')
    expect(ghost).toBeTruthy()
    expect(slot).toHaveAttribute('data-dragging', 'true')
    fireEvent.pointerUp(slot, { clientX: 290, clientY: 10, pointerId: 1 })
    expect(document.querySelector('[data-ui="org.workbench.favorite.drag"]')).toBeNull()
    expect(openWorkbenchAppletWindow).not.toHaveBeenCalled()
    const order = [...slot.parentElement!.querySelectorAll('[data-applet]')].map((node) =>
      node.getAttribute('data-applet')
    )
    expect(order[2]).toBe('inbox')
    expect(JSON.parse(localStorage.getItem(`org.workbench.favorites.v1:${orgId}`) ?? '[]')[2]).toBe('inbox')
  })

  it('leaves a stored favorites list unchanged when the catalog grows', () => {
    const stored = JSON.stringify(['inbox', 'approval'])
    localStorage.setItem(`org.workbench.favorites.v1:${orgId}`, stored)
    render(<OrgWorkbench orgId={orgId} />)
    expect(localStorage.getItem(`org.workbench.favorites.v1:${orgId}`)).toBe(stored)
    expect(screen.getByRole('button', { name: '待办' })).toBeInTheDocument()
    expect(screen.getAllByRole('button', { name: '审批' }).length).toBeGreaterThan(0)
    expect(
      screen.queryByRole('button', { name: '待办' })?.closest('ul')?.querySelectorAll('[data-applet]')
    ).toHaveLength(2)
  })

  it('opens the fullscreen applet board from the add button and pins from it', () => {
    localStorage.setItem(`org.workbench.favorites.v1:${orgId}`, JSON.stringify(['inbox']))
    render(<OrgWorkbench orgId={orgId} />)
    const add = screen.getByRole('button', { name: '添加' })
    expect(add.querySelector('svg')).toBeTruthy()
    fireEvent.click(add)
    const board = document.querySelector('[data-ui="workbench.applet.launcher"]') as HTMLElement
    expect(board).toHaveClass('workbench-launchpad')
    expect(board).toHaveAttribute('data-state', 'open')
    expect(add).toHaveAttribute('aria-expanded', 'true')
    const approval = board.querySelector('[data-applet="approval"]') as HTMLElement
    fireEvent.click(approval.querySelector('[data-pin]')!)
    expect(JSON.parse(localStorage.getItem(`org.workbench.favorites.v1:${orgId}`) ?? '[]')).toEqual([
      'inbox',
      'approval'
    ])
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(board).toHaveAttribute('data-state', 'closing')
  })

  it('opens to-do as its applet window without installing an office package', async () => {
    render(<OrgWorkbench orgId={orgId} />)
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: '待办' }))
    await waitFor(() => expect(openWorkbenchAppletWindow).toHaveBeenCalledWith('inbox', '待办'))
    expect(installCapability).not.toHaveBeenCalled()
  })
})
