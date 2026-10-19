import { fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { describe, expect, it } from 'vitest'

import { emptyDrafts, type OfficeDrafts } from '../officeAppletDraft'
import { OfficeAppletFields } from '../OfficeAppletFields'

function LeaveForm() {
  const [drafts, setDrafts] = useState<OfficeDrafts>(emptyDrafts())
  return (
    <>
      <OfficeAppletFields appletId="leave" drafts={drafts} onChange={setDrafts} />
      <output data-testid="leave-type-value">{drafts.leave.type}</output>
    </>
  )
}

describe('office menu', () => {
  it('puts the selected leave type into the closed field name and drops the list', () => {
    render(<LeaveForm />)
    const trigger = document.getElementById('leave-type') as HTMLButtonElement
    const fieldName = trigger.getAttribute('aria-label')
    expect(fieldName).toBeTruthy()
    expect(trigger.getAttribute('aria-expanded')).toBe('false')

    fireEvent.click(trigger)
    const list = document.getElementById('leave-type-list')
    expect(list).not.toBeNull()
    const option = list?.querySelector('button') as HTMLButtonElement
    const optionName = option.textContent?.trim()
    expect(optionName).toBeTruthy()

    fireEvent.mouseDown(option)
    expect(screen.getByTestId('leave-type-value')).toHaveTextContent('事假')
    expect(document.getElementById('leave-type-list')).not.toBeInTheDocument()
    expect(trigger.getAttribute('aria-expanded')).toBe('false')
    expect(trigger.getAttribute('aria-label')).toBe(`${fieldName} ${optionName}`)
  })
})
