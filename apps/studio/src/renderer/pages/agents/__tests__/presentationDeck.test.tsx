import { act, render, screen } from '@testing-library/react'
import i18next from 'i18next'
import { beforeEach, describe, expect, it } from 'vitest'

import {
  armPresentationDeck,
  collapsePresentationSidebar,
  notePresentationAgentProse,
  outlineFromReply,
  PresentationSessionDeck,
  PresentationWorkspaceOpener,
  resetPresentationDeckForTests
} from '../presentationDeck'

const brief = { title: '演示文稿', topic: '季度业务回顾', lines: ['页数：自动', '语言：简体中文'] }

describe('presentation work mode', () => {
  beforeEach(() => {
    resetPresentationDeckForTests()
  })

  it('waits for workspace admission before opening once', () => {
    armPresentationDeck('task-1', brief)
    let opens = 0
    const view = render(<PresentationWorkspaceOpener sessionId="task-1" ready={false} openWorkspace={() => opens++} />)
    expect(opens).toBe(0)
    view.rerender(<PresentationWorkspaceOpener sessionId="task-1" ready openWorkspace={() => opens++} />)
    expect(opens).toBe(1)
    view.rerender(<PresentationWorkspaceOpener sessionId="task-1" ready openWorkspace={() => opens++} />)
    expect(opens).toBe(1)
  })

  it('renders the outline on the armed task and ignores every other session', () => {
    expect(outlineFromReply('{"step":"slide","layout":"title"}')).toBeNull()
    expect(outlineFromReply('```json\n{"step":"outline","outline":["## 开场"]}\n```')).toEqual(['## 开场'])

    const other = render(<PresentationSessionDeck sessionId="other-task" />)
    act(() => {
      armPresentationDeck('task-1', brief)
    })
    expect(document.querySelector('[data-ui="workbench.presentation"]')).toBeNull()
    other.unmount()

    render(<PresentationSessionDeck sessionId="task-1" />)
    const work = screen.getByRole('region', { name: '演示文稿' })
    expect(work).toHaveTextContent('季度业务回顾')
    expect(work).toHaveTextContent('页数：自动')
    expect(work.querySelector('[data-presentation-status="outline"]')).toHaveTextContent(
      i18next.t('workbench.presentation.preparing')
    )
    expect(work).not.toHaveTextContent('让想法变成幻灯片')

    const reply = JSON.stringify({ step: 'outline', outline: ['## 开场\n这是要点', '## 结尾'] })
    act(() => {
      expect(notePresentationAgentProse(reply)).toBe('applied')
    })
    expect(work.querySelector('[data-presentation-status="ready"]')).toHaveTextContent(
      i18next.t('workbench.presentation.ready')
    )
    expect(work.querySelector('[data-presentation-slide="1"]')).toHaveTextContent('开场')
    expect(work.querySelector('[data-presentation-slide="1"]')).toHaveTextContent('这是要点')
    expect(work.querySelector('[data-presentation-slide="2"]')).toHaveTextContent('结尾')
    act(() => {
      expect(notePresentationAgentProse('还在写，没有大纲')).toBe('idle')
    })
    expect(work.querySelectorAll('[data-presentation-slide]')).toHaveLength(2)

    let opens = 0
    render(<PresentationWorkspaceOpener sessionId="task-1" openWorkspace={() => (opens += 1)} />)
    render(<PresentationWorkspaceOpener sessionId="other-task" openWorkspace={() => (opens += 1)} />)
    expect(opens).toBe(1)
    expect(collapsePresentationSidebar('task-1', 'task-1', null)).toBe('task-1')
    expect(collapsePresentationSidebar('task-1', 'task-1', 'task-1')).toBeNull()
    expect(collapsePresentationSidebar('other-task', 'task-1', null)).toBeNull()
  })
})
