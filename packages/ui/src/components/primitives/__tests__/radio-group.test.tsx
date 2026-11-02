// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'

import { RadioGroup, RadioGroupItem } from '../radio-group'

describe('RadioGroup', () => {
  it('supports arrow-key selection and skips disabled options', async () => {
    const user = userEvent.setup()
    render(
      <RadioGroup defaultValue="one" aria-label="Choice">
        <RadioGroupItem value="one" aria-label="One" />
        <RadioGroupItem value="two" aria-label="Two" disabled />
        <RadioGroupItem value="three" aria-label="Three" />
      </RadioGroup>
    )
    const one = screen.getByRole('radio', { name: 'One' })
    const three = screen.getByRole('radio', { name: 'Three' })
    await user.tab()
    expect(one).toHaveFocus()
    await user.keyboard('{ArrowRight>}')
    await waitFor(() => expect(three).toHaveFocus())
    expect(three).toBeChecked()
    await user.keyboard('{/ArrowRight}')
    expect(screen.getByRole('radio', { name: 'Two' })).toBeDisabled()
  })
})
