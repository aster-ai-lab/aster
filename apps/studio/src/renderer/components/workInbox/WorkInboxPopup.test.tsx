import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { writeCenterId } from '@renderer/data/memberOrgs'
import { publishMailUnread } from '@renderer/mail/mailUnread'
import type { WorkDecision, WorkItem, WorkSummary } from '@renderer/workInbox/client'
import { WORK_INBOX_CHANGED } from '@renderer/workInbox/client'

import { WorkInboxButton, WorkInboxPopup } from './WorkInboxPopup'
import { WorkInboxView } from './WorkInboxView'
import { WorkInboxWindow } from './WorkInboxWindow'

const item: WorkItem = {
  id: 'item-1',
  org_id: 'org-1',
  org_name: 'ToWork',
  kind: 'join_request',
  title: '加入团队',
  summary: '周予安 申请加入 ToWork',
  status: 'pending',
  initiator_name: '周予安',
  current_node: '审批',
  created_at: '2026-09-23T07:59:46Z',
  actionable: true,
  approvable: true,
  rejectable: true,
  cancellable: false,
  fields: {},
  decision_comment: '',
  assignee_names: ['周予安'],
  decider_name: '',
  decided_at: '',
  schedule_retry: false,
  engine: false,
  trail: []
}

const summary: WorkSummary = { todo: 1, done: 0, cc: 0, sent: 0 }

afterEach(() => {
  localStorage.removeItem('a3s.org.center')
  publishMailUnread(0)
})

describe('work inbox', () => {
  it('separates approval and rejection capabilities and clears revoked controls after refresh', async () => {
    const user = userEvent.setup()
    let row: WorkItem = { ...item, approvable: true, rejectable: false }
    const decideItem = vi.fn()
    render(<WorkInboxView loadItems={async () => [row]} loadSummary={async () => summary} decideItem={decideItem} />)
    await user.click(await screen.findByRole('button', { name: /周予安 申请加入 ToWork/ }))
    expect(screen.getByRole('button', { name: '同意' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '拒绝' })).not.toBeInTheDocument()
    row = { ...row, approvable: false, rejectable: true }
    await act(async () => {
      window.dispatchEvent(new Event(WORK_INBOX_CHANGED))
    })
    await waitFor(() => expect(screen.queryByRole('button', { name: '同意' })).not.toBeInTheDocument())
    expect(screen.getByRole('button', { name: '拒绝' })).toBeInTheDocument()
    row = { ...row, approvable: undefined, rejectable: undefined }
    await act(async () => {
      window.dispatchEvent(new Event(WORK_INBOX_CHANGED))
    })
    await waitFor(() => expect(screen.queryByRole('button', { name: '拒绝' })).not.toBeInTheDocument())
    expect(decideItem).not.toHaveBeenCalled()
  })
  it('opens on the approval list and does not select a task until the user clicks one', async () => {
    const user = userEvent.setup()
    const decideItem = vi.fn()
    render(
      <WorkInboxPopup
        open
        anchor={document.body}
        onOpenChange={() => undefined}
        loadItems={async () => [item]}
        loadSummary={async () => summary}
        decideItem={decideItem}
      />
    )
    expect(await screen.findByRole('dialog')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /周予安 申请加入 ToWork/ })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '同意' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /周予安 申请加入 ToWork/ }))
    await user.type(screen.getByRole('textbox', { name: '审批意见' }), '同意')
    await user.click(screen.getByRole('button', { name: '同意' }))
    const agrees = screen.getAllByRole('button', { name: '同意' })
    await user.click(agrees[agrees.length - 1])
    expect(decideItem).toHaveBeenCalledWith('item-1', 'approve', '同意')
  })

  it('shows folder icons and does not show the request page', async () => {
    render(
      <WorkInboxPopup
        open
        anchor={document.body}
        onOpenChange={() => undefined}
        loadItems={async () => []}
        loadSummary={async () => summary}
      />
    )
    expect((await screen.findByRole('button', { name: /待办/ })).querySelector('svg')).not.toBeNull()
    expect(screen.getByRole('button', { name: '全部' }).querySelector('svg')).not.toBeNull()
    expect(screen.queryByRole('radio', { name: '发起申请' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '审批中心' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '请假' }).querySelector('svg')).not.toBeNull()
    expect(screen.getByText('类型')).toBeInTheDocument()
    expect(document.querySelector('[data-ui="work.inbox.detail.drawer"]')).toBeNull()
    expect(screen.queryByText('选择一条单据查看和处理')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '提交' })).not.toBeInTheDocument()
    expect(screen.queryByText('点一个小程序打开申请，或在下方直接提交。')).not.toBeInTheDocument()
  })

  it('filters type and department without searching the department name', async () => {
    const user = userEvent.setup()
    const expense: WorkItem = {
      ...item,
      id: 'expense-dept',
      kind: 'expense',
      title: '研发报销',
      summary: '高铁',
      fields: { department: '研发', amount: '12.00' }
    }
    const loadItems = vi.fn(async () => [expense])
    render(<WorkInboxView loadItems={loadItems} loadSummary={async () => summary} />)
    await screen.findByRole('button', { name: /研发报销/ })
    expect(document.querySelector('select, datalist')).toBeNull()

    await user.click(screen.getByRole('button', { name: '全部类型' }))
    await user.click(screen.getByRole('option', { name: '报销' }))
    await waitFor(() => expect(loadItems).toHaveBeenCalledWith(expect.objectContaining({ kind: 'expense' })))

    await user.click(screen.getByRole('button', { name: '全部部门' }))
    await user.click(screen.getByRole('option', { name: '研发' }))
    await waitFor(() =>
      expect(loadItems).toHaveBeenCalledWith(expect.objectContaining({ department: '研发', kind: 'expense' }))
    )
    expect(loadItems.mock.calls.some(([query]) => query.q === '研发')).toBe(false)
  })

  it('shows the saved process trail and hides the three-step diagram', async () => {
    const user = userEvent.setup()
    const expense: WorkItem = {
      ...item,
      id: 'expense-1',
      kind: 'expense',
      title: '其他 5001.00',
      summary: '流程提交',
      initiator_name: '林知夏',
      current_node: '财务',
      engine: true,
      trail: [
        { action: '提交', node_name: '发起', actor_name: '林知夏' },
        { action: '同意', node_name: '直属主管', actor_name: '创建人' },
        { action: '跳过', node_name: '人事', actor_name: '林知夏' },
        { action: '退回', node_name: '复核', actor_name: '创建人' }
      ],
      fields: { amount: '5001.00', category: '其他', detail: '流程提交' },
      assignee_names: ['创建人', '成员'],
      actionable: false,
      approvable: false,
      rejectable: false
    }
    render(<WorkInboxView loadItems={async () => [expense]} loadSummary={async () => summary} />)
    const card = await screen.findByRole('button', { name: /其他 5001.00/ })
    expect(card.querySelector('[data-ui="work.inbox.flow"]')).toBeNull()
    expect(card).toHaveTextContent('当前节点：财务')
    const trail = card.querySelector('[data-ui="work.inbox.trail"]')
    expect(trail).toHaveTextContent('提交')
    expect(trail).toHaveTextContent('同意')
    const trailText = (action: string) =>
      [...(trail?.querySelectorAll('li') ?? [])].find((row) => row.textContent?.includes(action))
    expect(trailText('提交')?.textContent).toMatch(/提交\s+发起 · 林知夏/)
    expect(trailText('跳过')?.querySelector('span.min-w-0')).toHaveClass('text-[#8a8a8a]')
    expect(trailText('退回')?.querySelector('span.min-w-0')).toHaveClass('text-[#b42318]')
    await user.click(card)
    const detail = document.querySelector('[data-ui="work.inbox.detail"]')
    expect(detail?.querySelector('[data-ui="work.inbox.flow"]')).toBeNull()
    expect(detail?.querySelector('[data-ui="work.inbox.trail"]')).toHaveTextContent('直属主管')
    expect(detail).toHaveTextContent('当前节点：财务')
  })

  it('shows a pending approval title and updates it from the primary action', async () => {
    const user = userEvent.setup()
    const approval: WorkItem = {
      id: 'approval-1',
      org_id: 'org-1',
      org_name: '通友科技',
      kind: 'approval',
      title: '差旅报销',
      summary: '北京往返',
      status: 'pending',
      initiator_name: '周予安',
      current_node: '主管审批',
      created_at: '2026-10-02T08:00:00Z',
      actionable: true,
      approvable: true,
      rejectable: true,
      cancellable: false,
      fields: { title: '差旅报销', detail: '北京往返' },
      decision_comment: '',
      assignee_names: ['创建人', '静默审批人'],
      decider_name: '',
      decided_at: '',
      schedule_retry: false,
      engine: false,
      trail: []
    }
    let current = approval
    const decideItem = vi.fn(async (_id: string, decision: WorkDecision, _comment: string) => {
      current = {
        ...current,
        status: decision === 'approve' ? 'approved' : decision === 'reject' ? 'rejected' : 'cancelled',
        actionable: false,
        approvable: false,
        rejectable: false
      }
    })
    render(
      <WorkInboxView loadItems={async () => [current]} loadSummary={async () => summary} decideItem={decideItem} />
    )
    const card = await screen.findByRole('button', { name: /差旅报销/ })
    expect(card).toHaveAttribute('data-ui', 'work.inbox.card')
    expect(card).toHaveTextContent('差旅报销')
    expect(card).toHaveTextContent('审批中')
    expect(card.querySelector('[data-step="start"]')).toHaveAttribute('data-state', 'done')
    expect(card.querySelector('[data-step="review"]')).toHaveAttribute('aria-current', 'step')
    expect(card.querySelector('[data-step="review"]')).toHaveTextContent('或签')
    expect(
      [
        ...(card.querySelector('[data-step="review"]')?.querySelectorAll('[data-ui="work.inbox.flow.person"]') ?? [])
      ].map((node) => node.textContent)
    ).toEqual(['创建人', '静默审批人'])
    expect(card.querySelector('[data-step="end"]')).toHaveAttribute('data-state', 'waiting')
    expect(card.textContent).not.toContain('会签')
    await user.click(card)
    expect(await screen.findByText('北京往返')).toBeInTheDocument()
    const detailTitle = await screen.findByRole('heading', { level: 3, name: '差旅报销' })
    expect(detailTitle.parentElement).toHaveTextContent('审批中')
    const detail = document.querySelector('[data-ui="work.inbox.detail"]')
    expect(detail?.querySelector('[data-step="review"]')).toHaveAttribute('aria-current', 'step')
    expect(detail?.querySelector('[data-step="start"]')).toHaveAttribute('data-state', 'done')
    expect(detail?.querySelector('[data-step="end"]')).toHaveAttribute('data-state', 'waiting')
    await user.type(screen.getByRole('textbox', { name: '审批意见' }), '同意')
    await user.click(screen.getByRole('button', { name: '同意' }))
    const confirms = screen.getAllByRole('button', { name: '同意' })
    await user.click(confirms[confirms.length - 1])
    await waitFor(() => expect(decideItem).toHaveBeenCalledWith('approval-1', 'approve', '同意'))
    await waitFor(() => {
      const updated = screen.getByRole('button', { name: /差旅报销/ })
      expect(updated).toHaveTextContent('已通过')
      expect(updated).not.toHaveTextContent('审批中')
      expect(updated.querySelector('[data-step="end"]')).toHaveAttribute('aria-current', 'step')
      expect(updated.querySelector('[data-step="review"]')).toHaveAttribute('data-state', 'done')
    })
    expect(document.querySelector('[data-ui="work.inbox.detail"] [data-step="end"]')).toHaveAttribute(
      'aria-current',
      'step'
    )
    expect(screen.getByRole('heading', { level: 3, name: '差旅报销' })).toBeInTheDocument()
  })

  it('does not overwrite the current team with a delayed previous-team response', async () => {
    let finishPrevious!: (items: WorkItem[]) => void
    const previous = new Promise<WorkItem[]>((resolve) => {
      finishPrevious = resolve
    })
    const current = { ...item, id: 'new-team-item', org_id: 'org-2', title: '新团队审批', summary: '新团队说明' }
    localStorage.setItem('a3s.org.center', 'org-1')
    render(
      <WorkInboxView
        loadItems={async () => (localStorage.getItem('a3s.org.center') === 'org-1' ? previous : [current])}
        loadSummary={async () => summary}
      />
    )
    act(() => writeCenterId('org-2'))
    expect(await screen.findByRole('button', { name: /新团队审批/ })).toBeInTheDocument()
    await act(async () => {
      finishPrevious([item])
      await previous
    })
    expect(screen.getByRole('button', { name: /新团队审批/ })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /周予安 申请加入 ToWork/ })).not.toBeInTheDocument()
  })

  it('shows a dot when there is a pending item', async () => {
    render(<WorkInboxButton layout="icon" loadSummary={async () => summary} />)
    expect(await screen.findByRole('button', { name: '任务' })).toBeInTheDocument()
    await waitFor(() => expect(document.querySelector('[data-ui~="work.inbox.dot"]')).not.toBeNull())
  })

  it('shows a dot when mail is unread and opens mail in the same window', async () => {
    const user = userEvent.setup()
    publishMailUnread(2)
    render(
      <>
        <WorkInboxButton layout="icon" loadSummary={async () => ({ todo: 0, done: 0, cc: 0, sent: 0 })} />
        <WorkInboxPopup
          open
          anchor={document.body}
          onOpenChange={() => undefined}
          loadItems={async () => []}
          loadSummary={async () => ({ todo: 0, done: 0, cc: 0, sent: 0 })}
        />
      </>
    )
    await waitFor(() => expect(document.querySelector('[data-ui~="work.inbox.dot"]')).not.toBeNull())
    const mail = screen.getByRole('tab', { name: /邮件/ })
    expect(mail).toHaveTextContent('2')
    await user.click(mail)
    expect(await screen.findByRole('heading', { name: '邮件' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /待办/ })).not.toBeInTheDocument()
  })

  it('shows the translated outage on the list and hides the runtime code', async () => {
    render(
      <WorkInboxView
        loadItems={async () => {
          throw new Error('runtime')
        }}
        loadSummary={async () => ({ todo: 0, done: 0, cc: 0, sent: 0 })}
      />
    )
    expect(await screen.findByText('消息中心暂时打不开')).toBeInTheDocument()
    expect(document.body.textContent).not.toContain('runtime')
    expect(screen.queryByRole('button', { name: '提交' })).not.toBeInTheDocument()
  })

  it('shows the box counts from one summary', async () => {
    render(
      <WorkInboxView loadItems={async () => []} loadSummary={async () => ({ todo: 4, done: 1, cc: 2, sent: 3 })} />
    )
    expect(await screen.findByRole('button', { name: /待办/ })).toHaveTextContent('4')
    expect(screen.getByRole('button', { name: /已办/ })).toHaveTextContent('1')
    expect(screen.getByRole('button', { name: /抄送我/ })).toHaveTextContent('2')
    expect(screen.getByRole('button', { name: /已发起/ })).toHaveTextContent('3')
  })

  it('filters once and does not reload when a card is selected', async () => {
    const user = userEvent.setup({ delay: null })
    const loadItems = vi.fn(async () => [item])
    render(<WorkInboxView loadItems={loadItems} loadSummary={async () => summary} />)
    await screen.findByRole('button', { name: /周予安 申请加入 ToWork/ })
    const listed = loadItems.mock.calls.length
    await user.click(screen.getByRole('button', { name: /周予安 申请加入 ToWork/ }))
    expect(loadItems).toHaveBeenCalledTimes(listed)

    await user.click(screen.getByRole('button', { name: '加入团队' }))
    await waitFor(() =>
      expect(loadItems).toHaveBeenCalledWith(expect.objectContaining({ box: 'todo', kind: 'join_request' }))
    )
    await user.click(screen.getByRole('button', { name: '流程审批' }))
    await waitFor(() => expect(loadItems).toHaveBeenCalledWith(expect.objectContaining({ kind: 'approval' })))
    await user.click(screen.getByRole('button', { name: '请假' }))
    await waitFor(() => expect(loadItems).toHaveBeenCalledWith(expect.objectContaining({ kind: 'leave' })))
    await user.click(screen.getByRole('button', { name: /已办/ }))
    await waitFor(() => expect(loadItems).toHaveBeenCalledWith(expect.objectContaining({ box: 'done', kind: 'leave' })))
    const side = screen.getByRole('navigation')
    expect(within(side).getByRole('button', { name: '请假' })).toHaveAttribute('aria-current', 'page')
    expect(within(side).getByRole('button', { name: /已办/ })).toHaveAttribute('aria-current', 'page')

    await user.type(screen.getByRole('textbox', { name: '搜索' }), '采购')
    await waitFor(() => expect(loadItems).toHaveBeenCalledWith(expect.objectContaining({ q: '采购', box: 'done' })))
  })

  it('filters time and status from in-page menus and can hide the navigation', async () => {
    const user = userEvent.setup()
    const loadItems = vi.fn(async () => [item])
    render(<WorkInboxView loadItems={loadItems} loadSummary={async () => summary} />)
    const card = await screen.findByRole('button', { name: /周予安 申请加入 ToWork/ })
    expect(card).toHaveTextContent('流程标题：')
    expect(card).toHaveTextContent('发起')
    expect(card).toHaveTextContent('当前 · 或签')
    expect(card).toHaveTextContent('结束')
    expect(document.querySelector('select')).toBeNull()

    await user.click(screen.getByRole('button', { name: '所有时间' }))
    expect(screen.getByRole('listbox', { name: '时间' })).toBeInTheDocument()
    await user.click(screen.getByRole('option', { name: '最近 7 天' }))
    await waitFor(() => expect(loadItems).toHaveBeenCalledWith(expect.objectContaining({ days: 7 })))

    expect(screen.getByRole('button', { name: '全部类型' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '全部部门' })).toBeInTheDocument()
    expect(document.querySelector('select, datalist')).toBeNull()

    await user.click(screen.getByRole('button', { name: '全部状态' }))
    await user.click(screen.getByRole('option', { name: '已通过' }))
    await waitFor(() =>
      expect(loadItems).toHaveBeenCalledWith(expect.objectContaining({ status: 'approved', days: 7 }))
    )
    expect(document.querySelector('select')).toBeNull()

    await user.click(screen.getByRole('button', { name: '收起导航' }))
    expect(screen.queryByRole('button', { name: /待办/ })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '展开导航' }))
    expect(screen.getByRole('button', { name: /待办/ })).toBeInTheDocument()

    expect(document.querySelector('[data-ui="work.inbox.detail.drawer"]')).toBeNull()
    await user.click(screen.getByRole('button', { name: /周予安 申请加入 ToWork/ }))
    const drawer = document.querySelector('[data-ui="work.inbox.detail.drawer"]')
    expect(drawer).toHaveAttribute('data-state', 'open')
    expect(drawer?.className).toContain('absolute')
    expect(drawer?.className).toContain('right-0')
    expect(drawer?.className).toContain('w-[320px]')
    expect(drawer?.className).toContain('work-inbox-detail-slide')
    expect(screen.getByRole('button', { name: '同意' })).toBeInTheDocument()
    const close = screen.getByRole('button', { name: '关闭' })
    expect(drawer).toContainElement(close)
    expect(close).toHaveClass('inline-flex', 'size-6', 'items-center', 'justify-center', 'p-0', 'leading-none')
    expect(close.querySelector('svg')).toHaveClass('block', 'size-3.5')
    expect(screen.getByRole('textbox', { name: '审批意见' })).toHaveClass(
      'rounded-[8px]',
      'border-0',
      'bg-[#f5f5f5]',
      'text-[13px]',
      'focus-visible:ring-inset'
    )
    await user.click(close)
    expect(drawer).toHaveAttribute('data-state', 'closed')
    expect(screen.queryByRole('button', { name: '同意' })).not.toBeInTheDocument()
    expect(screen.queryByText('此单据需通过侧边栏展示')).not.toBeInTheDocument()
    expect(screen.queryByText('选择一条单据查看和处理')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /周予安 申请加入 ToWork/ }))
    expect(drawer).toHaveAttribute('data-state', 'open')
    expect(screen.getByRole('button', { name: '同意' })).toBeInTheDocument()
  })

  it('keeps the missing-approver sentence on a decision', async () => {
    const user = userEvent.setup()
    render(
      <WorkInboxView
        loadItems={async () => [item]}
        loadSummary={async () => summary}
        decideItem={async () => {
          throw new Error('no other approver')
        }}
      />
    )
    await user.click(await screen.findByRole('button', { name: /周予安 申请加入 ToWork/ }))
    await user.type(screen.getByRole('textbox', { name: '审批意见' }), '同意')
    await user.click(screen.getByRole('button', { name: '同意' }))
    const agrees = screen.getAllByRole('button', { name: '同意' })
    await user.click(agrees[agrees.length - 1])
    expect(await screen.findAllByText('这个团队里还没有其他审批人')).toHaveLength(2)
  })

  it('puts back before forward and walks the approval list', async () => {
    const user = userEvent.setup()
    const loadItems = vi.fn(async () => [item])
    render(<WorkInboxWindow loadItems={loadItems} loadSummary={async () => summary} />)
    expect(await screen.findByRole('heading', { level: 1, name: '任务' })).toBeInTheDocument()
    const nav = await screen.findByRole('toolbar', { name: '小程序导航' })
    expect(
      within(nav)
        .getAllByRole('button')
        .map((button) => button.getAttribute('aria-label'))
    ).toEqual(['回到首页', '后退', '前进', '刷新'])
    expect(within(nav).getByRole('button', { name: '后退' })).toBeDisabled()
    expect(within(nav).getByRole('button', { name: '前进' })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: /已办/ }))
    await waitFor(() => expect(loadItems.mock.calls.at(-1)?.[0]).toEqual(expect.objectContaining({ box: 'done' })))
    expect(within(nav).getByRole('button', { name: '后退' })).toBeEnabled()
    await user.click(within(nav).getByRole('button', { name: '后退' }))
    await waitFor(() => expect(loadItems.mock.calls.at(-1)?.[0]).toEqual(expect.objectContaining({ box: 'todo' })))
    expect(within(nav).getByRole('button', { name: '前进' })).toBeEnabled()
    await user.click(within(nav).getByRole('button', { name: '前进' }))
    await waitFor(() => expect(loadItems.mock.calls.at(-1)?.[0]).toEqual(expect.objectContaining({ box: 'done' })))
    await user.click(within(nav).getByRole('button', { name: '回到首页' }))
    await waitFor(() =>
      expect(loadItems.mock.calls.at(-1)?.[0]).toEqual(expect.objectContaining({ box: 'todo', kind: undefined }))
    )
    const calls = loadItems.mock.calls.length
    await user.click(within(nav).getByRole('button', { name: '刷新' }))
    await waitFor(() => expect(loadItems.mock.calls.length).toBeGreaterThan(calls))
    expect(screen.queryByRole('radio', { name: '发起申请' })).not.toBeInTheDocument()
  })

  it('shows the decider on a finished review and skips a withdrawal', async () => {
    const user = userEvent.setup()
    const approved: WorkItem = {
      ...item,
      id: 'leave-1',
      kind: 'leave',
      title: '事假两天',
      summary: '家中有事',
      status: 'approved',
      actionable: false,
      approvable: false,
      rejectable: false,
      assignee_names: ['创建人', '静默审批人'],
      decider_name: '创建人',
      decided_at: '2026-10-14T03:51:53Z',
      decision_comment: '同意请假'
    }
    const { unmount } = render(<WorkInboxView loadItems={async () => [approved]} loadSummary={async () => summary} />)
    await user.click(await screen.findByRole('button', { name: /事假两天/ }))
    const detail = document.querySelector('[data-ui="work.inbox.detail"]')
    const review = detail?.querySelector('[data-step="review"]')
    expect(review).toHaveAttribute('data-state', 'done')
    expect(review?.querySelector('[data-ui="work.inbox.flow.decider"]')).toHaveTextContent('创建人')
    expect(review?.querySelector('time')).toHaveAttribute('dateTime', '2026-10-14T03:51:53Z')
    expect(review?.querySelector('[data-ui="work.inbox.flow.comment"]')).toHaveTextContent('同意请假')
    expect(detail?.querySelector('[data-step="end"]')).toHaveAttribute('aria-current', 'step')
    expect(detail?.textContent).not.toContain('会签')
    unmount()

    const withdrawn: WorkItem = {
      ...item,
      id: 'leave-2',
      kind: 'leave',
      title: '已撤回请假',
      status: 'cancelled',
      actionable: false,
      approvable: false,
      rejectable: false,
      decider_name: '创建人',
      decided_at: '2026-10-14T03:51:53Z',
      decision_comment: '撤回'
    }
    render(<WorkInboxView loadItems={async () => [withdrawn]} loadSummary={async () => summary} />)
    const card = await screen.findByRole('button', { name: /已撤回请假/ })
    expect(card.querySelector('[data-step="review"]')).toHaveAttribute('data-state', 'skipped')
    expect(card.querySelector('[data-ui="work.inbox.flow.decider"]')).toBeNull()
    expect(card.textContent).not.toContain('会签')
  })

  it('does not draw an approval flow on a notice', async () => {
    const notice: WorkItem = {
      ...item,
      id: 'notice-1',
      kind: 'notice',
      title: '停水通知',
      summary: '明天停水',
      current_node: '公告'
    }
    render(<WorkInboxView loadItems={async () => [notice]} loadSummary={async () => summary} />)
    const card = await screen.findByRole('button', { name: /停水通知/ })
    expect(card.querySelector('[data-ui="work.inbox.flow"]')).toBeNull()
    expect(card).toHaveTextContent('当前节点：公告')
  })

  it('keeps an approved meeting and lets the initiator write the schedule again', async () => {
    const user = userEvent.setup()
    const meeting: WorkItem = {
      ...item,
      id: 'meeting-1',
      kind: 'meeting',
      title: '周例会',
      summary: '三楼',
      status: 'approved',
      actionable: false,
      approvable: false,
      rejectable: false,
      fields: { title: '周例会', schedule: 'unwritten' },
      schedule_retry: true
    }
    const retrySchedule = vi.fn(async () => ({
      ...meeting,
      schedule_retry: false,
      fields: { title: '周例会', schedule: 'written' }
    }))
    render(
      <WorkInboxView
        loadItems={async () => [meeting]}
        loadSummary={async () => summary}
        retrySchedule={retrySchedule}
      />
    )
    const card = await screen.findByRole('button', { name: /周例会/ })
    expect(card).toHaveTextContent('周例会')
    await user.click(card)
    expect(await screen.findByText('日程未写入')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '重新写入日程' }))
    await waitFor(() => expect(retrySchedule).toHaveBeenCalledWith('meeting-1'))
  })
})
