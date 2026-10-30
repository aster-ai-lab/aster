// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, expect, it } from 'vitest'

import { InlineDisclosure } from '../inline-disclosure'

afterEach(cleanup)

it('immediately removes folded content from interaction and retains its state on reversal', async () => {
  const user = userEvent.setup()
  const { container, rerender } = render(
    <InlineDisclosure open id="details">
      <input aria-label="Draft" defaultValue="Keep me" />
    </InlineDisclosure>
  )
  const input = screen.getByRole('textbox')
  await user.type(input, ' too')
  rerender(
    <InlineDisclosure open={false} id="details">
      <input aria-label="Draft" defaultValue="Keep me" />
    </InlineDisclosure>
  )
  expect(container.firstChild).toHaveAttribute('aria-hidden', 'true')
  expect(container.firstChild).toHaveAttribute('inert')
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  rerender(
    <InlineDisclosure open id="details">
      <input aria-label="Draft" defaultValue="Keep me" />
    </InlineDisclosure>
  )
  expect(screen.getByRole('textbox')).toBe(input)
  expect(input).toHaveValue('Keep me too')
  expect(container.firstChild).not.toHaveAttribute('inert')
  expect(container.firstChild).not.toHaveAttribute('aria-hidden')
})
