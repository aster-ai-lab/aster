// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { SearchInput } from '../index'

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

describe('SearchInput', () => {
  it('renders a search field with the given placeholder', () => {
    render(<SearchInput placeholder="搜索" value="" onChange={() => {}} />)

    expect(screen.getByRole('searchbox')).toHaveAttribute('placeholder', '搜索')
  })

  it('calls onChange when the user types', () => {
    const onChange = vi.fn()
    render(<SearchInput value="" onChange={onChange} />)

    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'cherry' } })

    expect(onChange).toHaveBeenCalledTimes(1)
  })

  it('shows the clear button only when onClear is set and the value is non-empty', () => {
    const { rerender } = render(<SearchInput value="" onChange={() => {}} onClear={() => {}} clearLabel="清除" />)
    expect(screen.queryByRole('button', { name: '清除' })).not.toBeInTheDocument()

    rerender(<SearchInput value="cherry" onChange={() => {}} onClear={() => {}} clearLabel="清除" />)
    expect(screen.getByRole('button', { name: '清除' })).toBeInTheDocument()
  })

  it('hides the clear button when no onClear handler is provided', () => {
    render(<SearchInput value="cherry" onChange={() => {}} />)

    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('invokes onClear when the clear button is clicked', () => {
    const onClear = vi.fn()
    render(<SearchInput value="cherry" onChange={() => {}} onClear={onClear} clearLabel="清除" />)

    fireEvent.click(screen.getByRole('button', { name: '清除' }))

    expect(onClear).toHaveBeenCalledTimes(1)
  })

  it('keeps the clear space stable and returns focus to the Aster search field', async () => {
    const previousTenant = document.documentElement.dataset.tenant
    document.documentElement.dataset.tenant = 'aster'
    function Example() {
      const [value, setValue] = useState('query')
      return (
        <SearchInput
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onClear={() => setValue('')}
          clearLabel="清除"
        />
      )
    }
    try {
      const user = userEvent.setup()
      const { container } = render(<Example />)
      const slot = container.querySelector('[data-align="inline-end"]')
      expect(slot).toHaveClass('w-8')
      await user.click(screen.getByRole('button', { name: '清除' }))
      expect(screen.getByRole('searchbox')).toHaveValue('')
      expect(screen.getByRole('searchbox')).toHaveFocus()
      expect(screen.queryByRole('button', { name: '清除' })).not.toBeInTheDocument()
      expect(container.querySelector('[data-align="inline-end"]')).toBe(slot)
      await user.type(screen.getByRole('searchbox'), 'new query')
      expect(screen.getByRole('searchbox')).toHaveValue('new query')
    } finally {
      if (previousTenant === undefined) delete document.documentElement.dataset.tenant
      else document.documentElement.dataset.tenant = previousTenant
    }
  })

  it('disables the input and the clear button when disabled', () => {
    render(<SearchInput value="cherry" onChange={() => {}} onClear={() => {}} clearLabel="清除" disabled />)

    expect(screen.getByRole('searchbox')).toBeDisabled()
    expect(screen.getByRole('button', { name: '清除' })).toBeDisabled()
  })

  it('supports a custom clear button label', () => {
    render(<SearchInput value="cherry" onChange={() => {}} onClear={() => {}} clearLabel="清除" />)

    expect(screen.getByRole('button', { name: '清除' })).toBeInTheDocument()
  })

  it('renders the clear action as a square icon button', () => {
    render(<SearchInput value="cherry" onChange={() => {}} onClear={() => {}} clearLabel="清除" />)

    expect(screen.getByRole('button', { name: '清除' })).toHaveClass('size-6', 'min-h-0')
  })

  it('defaults to the h-9 field height', () => {
    const { container } = render(<SearchInput value="" onChange={() => {}} />)

    expect(container.querySelector('[data-slot="input-group"]')).toHaveClass('h-9')
  })

  it('renders the compact 30px field when size is sm', () => {
    const { container } = render(<SearchInput size="sm" value="" onChange={() => {}} />)

    const group = container.querySelector('[data-slot="input-group"]')
    expect(group).toHaveClass('h-[30px]')
    expect(group).not.toHaveClass('h-9')
  })

  it('applies containerClassName to the input group, overriding the size height', () => {
    const { container } = render(<SearchInput size="sm" containerClassName="h-8" value="" onChange={() => {}} />)

    const group = container.querySelector('[data-slot="input-group"]')
    expect(group).toHaveClass('h-8')
    expect(group).not.toHaveClass('h-7')
  })

  it('turns off the native searchfield paint so the group track is the only field', () => {
    render(<SearchInput value="" onChange={() => {}} />)

    // WebKit would otherwise draw its own fill, outline and accent focus ring
    // inside the group.
    expect(screen.getByRole('searchbox')).toHaveClass('appearance-none')
  })
})
