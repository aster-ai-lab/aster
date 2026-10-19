// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { Input } from '../input'

describe('Input', () => {
  it('keeps the field borderless and uses the inset focus ring', () => {
    render(<Input aria-label="Name" />)

    const input = screen.getByRole('textbox', { name: 'Name' })
    expect(input.className).toContain('border-transparent')
    expect(input.className).toContain('bg-[#f5f5f5]')
    expect(input.className).toContain('focus-visible:ring-black/10')
    expect(input.className).not.toContain('focus-visible:border-input')
    expect(input.className).not.toContain('focus-visible:outline-')
  })
})
