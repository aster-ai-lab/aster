// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { Input } from '../textarea'

afterEach(() => {
  cleanup()
})

describe('TextareaInput', () => {
  it('reports edits to the consumer', async () => {
    const user = userEvent.setup()
    const onValueChange = vi.fn()
    render(<Input aria-label="Answer" onValueChange={onValueChange} />)
    await user.type(screen.getByRole('textbox', { name: 'Answer' }), 'hello')
    expect(onValueChange).toHaveBeenLastCalledWith('hello')
  })

  it('preserves explicit accessibility errors and lets hasError override them', () => {
    const { rerender } = render(<Input aria-label="Answer" aria-invalid="grammar" />)
    const field = screen.getByRole('textbox', { name: 'Answer' })
    expect(field).toHaveAttribute('aria-invalid', 'grammar')
    rerender(<Input aria-label="Answer" aria-invalid="false" hasError />)
    expect(field).toHaveAttribute('aria-invalid', 'true')
  })

  it('keeps consumer classes on the field', () => {
    render(<Input className="resize-none" />)
    expect(screen.getByRole('textbox')).toHaveClass('resize-none')
  })
})
