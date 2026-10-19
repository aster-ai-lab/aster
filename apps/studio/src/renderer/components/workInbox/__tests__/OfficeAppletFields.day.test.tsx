import { act, fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { describe, expect, it } from 'vitest'

import { emptyDrafts, formatOfficeDay, type OfficeDrafts } from '../officeAppletDraft'
import { OfficeAppletFields } from '../OfficeAppletFields'

function LeaveForm() {
  const [drafts, setDrafts] = useState<OfficeDrafts>(emptyDrafts())
  return (
    <>
      <OfficeAppletFields appletId="leave" drafts={drafts} onChange={setDrafts} />
      <output data-testid="leave-span">{`${drafts.leave.start}|${drafts.leave.end}`}</output>
    </>
  )
}

function ExpenseForm() {
  const [drafts, setDrafts] = useState<OfficeDrafts>(emptyDrafts())
  return (
    <>
      <OfficeAppletFields appletId="expense" drafts={drafts} onChange={setDrafts} />
      <output data-testid="spent-on">{drafts.expense.date}</output>
    </>
  )
}

function openToday(key?: string) {
  render(<ExpenseForm />)
  fireEvent.click(document.getElementById('expense-date') as HTMLButtonElement)
  const today = formatOfficeDay(new Date())
  const day = document.querySelector<HTMLButtonElement>(`[data-day="${today}"] button`)
  expect(day).not.toBeNull()
  if (key) fireEvent.keyDown(day as HTMLButtonElement, { key })
  else fireEvent.click(day as HTMLButtonElement)
  return today
}

describe('leave range field', () => {
  it('stores the two pressed days when each day receives mouse down', () => {
    render(<LeaveForm />)
    fireEvent.click(document.getElementById('leave-start') as HTMLButtonElement)
    const start = document.querySelector<HTMLButtonElement>('[data-day="2026-10-19"] button')
    const end = document.querySelector<HTMLButtonElement>('[data-day="2026-10-22"] button')
    expect(start).not.toBeNull()
    expect(end).not.toBeNull()
    fireEvent.mouseDown(start?.querySelector('span') as HTMLSpanElement)
    fireEvent.mouseDown(end?.querySelector('span') as HTMLSpanElement)
    expect(screen.getByTestId('leave-span')).toHaveTextContent('2026-10-19|2026-10-22')
  })
})

describe('expense day field', () => {
  it('stores today when Space is pressed on that day', () => {
    const today = openToday(' ')
    expect(screen.getByTestId('spent-on')).toHaveTextContent(today)
    expect(document.querySelector('[data-ui="workbench.applet.expense.calendar"]')).not.toBeInTheDocument()
  })

  it('stores today when Enter is pressed on that day', () => {
    const today = openToday('Enter')
    expect(screen.getByTestId('spent-on')).toHaveTextContent(today)
    expect(document.querySelector('[data-ui="workbench.applet.expense.calendar"]')).not.toBeInTheDocument()
  })

  it('stores today when the day is clicked', () => {
    const today = openToday()
    expect(screen.getByTestId('spent-on')).toHaveTextContent(today)
    expect(document.querySelector('[data-ui="workbench.applet.expense.calendar"]')).not.toBeInTheDocument()
  })

  it('stores today from the today button without opening the calendar', () => {
    render(<ExpenseForm />)
    fireEvent.click(document.querySelector('[data-ui="workbench.applet.expense.date.today"]') as HTMLButtonElement)
    const today = formatOfficeDay(new Date())
    expect(screen.getByTestId('spent-on')).toHaveTextContent(today)
    expect(document.querySelector('[data-ui="workbench.applet.expense.calendar"]')).not.toBeInTheDocument()
  })

  it('stores today when Space reaches the window while the calendar is open', () => {
    render(<ExpenseForm />)
    fireEvent.click(document.getElementById('expense-date') as HTMLButtonElement)
    expect(document.querySelector('[data-ui="workbench.applet.expense.calendar"]')).toBeInTheDocument()
    act(() => {
      window.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Unidentified', code: 'Space', bubbles: true, cancelable: true })
      )
    })
    const today = formatOfficeDay(new Date())
    expect(screen.getByTestId('spent-on')).toHaveTextContent(today)
    expect(document.querySelector('[data-ui="workbench.applet.expense.calendar"]')).not.toBeInTheDocument()
  })
})
