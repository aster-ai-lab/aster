import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { ApprovalFlowDiagram } from './ApprovalFlowDiagram'

function step(id: string): Element {
  const node = document.querySelector(`[data-ui="work.inbox.flow.step"][data-step="${id}"]`)
  if (!node) throw new Error(`missing step ${id}`)
  return node
}

describe('approval flow diagram', () => {
  it('marks the review as the only current step while the item is pending', () => {
    render(<ApprovalFlowDiagram status="pending" layout="row" assigneeNames={['创建人', '   ', '静默审批人']} />)
    expect(screen.getByRole('list', { name: '审批流程' })).toBeInTheDocument()
    expect(step('start')).toHaveAttribute('data-state', 'done')
    expect(step('review')).toHaveAttribute('data-state', 'current')
    expect(step('review')).toHaveAttribute('aria-current', 'step')
    expect(step('review')).toHaveTextContent('当前 · 或签')
    expect(
      [...step('review').querySelectorAll('[data-ui="work.inbox.flow.person"]')].map((node) => node.textContent)
    ).toEqual(['创建人', '静默审批人'])
    expect(step('end')).toHaveAttribute('data-state', 'waiting')
    expect(step('end')).toHaveTextContent('未开始')
    expect(document.querySelectorAll('[aria-current="step"]')).toHaveLength(1)
    expect(screen.queryByText('会签')).not.toBeInTheDocument()
    const people = step('review').querySelector('[data-ui="work.inbox.flow.people"]')
    expect(people).toContainElement(step('review').querySelector('[data-ui="work.inbox.flow.person"]'))
    expect(people).toHaveClass('truncate')
  })

  it('keeps every assignee on the one review step', () => {
    const names = ['审批人1', '审批人2', '审批人3', '审批人4', '审批人5', '审批人6']
    render(<ApprovalFlowDiagram status="pending" layout="column" assigneeNames={names} />)
    expect(document.querySelectorAll('[data-ui="work.inbox.flow.step"]')).toHaveLength(3)
    expect(
      [...step('review').querySelectorAll('[data-ui="work.inbox.flow.person"]')].map((node) => node.textContent)
    ).toEqual(names)
    expect(step('review').querySelector('[data-ui="work.inbox.flow.people"]')).toHaveClass('flex-wrap')
    expect(step('end').querySelector('[data-ui="work.inbox.flow.person"]')).toBeNull()
  })

  it('lands an approved item on the end and keeps the review finished', () => {
    render(
      <ApprovalFlowDiagram
        status="approved"
        layout="column"
        deciderName="创建人"
        decidedAt="2026-10-14T03:51:53Z"
        comment="同意这笔"
      />
    )
    expect(step('review')).toHaveAttribute('data-state', 'done')
    expect(step('review')).not.toHaveAttribute('aria-current')
    expect(step('review').querySelector('[data-ui="work.inbox.flow.decider"]')).toHaveTextContent('创建人')
    expect(step('review').querySelector('time')).toHaveAttribute('dateTime', '2026-10-14T03:51:53Z')
    expect(step('review').querySelector('time')?.textContent ?? '').toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/)
    expect(step('review').querySelector('[data-ui="work.inbox.flow.comment"]')).toHaveTextContent('同意这笔')
    expect(step('end')).toHaveAttribute('aria-current', 'step')
    expect(step('end')).toHaveTextContent('当前 · 已通过')
    expect(screen.queryByText('会签')).not.toBeInTheDocument()
  })

  it('lands a rejected item on the end with the rejection', () => {
    render(<ApprovalFlowDiagram status="rejected" layout="row" />)
    expect(step('review')).toHaveAttribute('data-state', 'done')
    expect(step('end')).toHaveAttribute('aria-current', 'step')
    expect(step('end')).toHaveTextContent('当前 · 已拒绝')
  })

  it('skips the review when the initiator withdrew the item', () => {
    render(
      <ApprovalFlowDiagram
        status="cancelled"
        layout="column"
        deciderName="创建人"
        decidedAt="2026-10-14T03:51:53Z"
        comment="撤回"
      />
    )
    expect(step('start')).toHaveAttribute('data-state', 'done')
    expect(step('review')).toHaveAttribute('data-state', 'skipped')
    expect(step('review')).toHaveTextContent('已跳过')
    expect(step('review').querySelector('[data-ui="work.inbox.flow.decider"]')).toBeNull()
    expect(step('review').textContent).not.toContain('创建人')
    expect(step('end')).toHaveAttribute('aria-current', 'step')
    expect(step('end')).toHaveTextContent('当前 · 已撤回')
    expect(screen.queryByText('会签')).not.toBeInTheDocument()
  })
})
