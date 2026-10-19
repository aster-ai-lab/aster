// @vitest-environment happy-dom
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { Checkbox } from '../checkbox'

describe('Checkbox', () => {
  it('uses the 16px ink box from the design spec', () => {
    render(<Checkbox defaultChecked aria-label="Accept" />)
    const box = screen.getByRole('checkbox', { name: 'Accept' })
    const className = box.className
    for (const token of [
      'size-4',
      'rounded-[4px]',
      'border-[#e5e5e5]',
      'bg-white',
      'data-[state=checked]:bg-[#171717]',
      'data-[state=checked]:text-white',
      'focus-visible:ring-black/10',
      'disabled:opacity-40'
    ]) {
      expect(className).toContain(token)
    }
    expect(className).not.toContain('hover:scale')
    expect(className).not.toContain('animate-checkbox')
    expect(box.querySelector('svg')?.getAttribute('stroke-width')).toBe('1.7')
  })
})
