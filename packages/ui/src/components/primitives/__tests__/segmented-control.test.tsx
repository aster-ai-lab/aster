// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { SegmentedControl } from '../segmented-control'

const options = [
  { value: 'app', label: 'App' },
  { value: 'window', label: 'Window' },
  { value: 'disabled', label: 'Disabled', disabled: true }
] as const

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
  vi.unstubAllGlobals()
})

describe('SegmentedControl', () => {
  it('keeps controlled selection until the value prop changes', () => {
    const onValueChange = vi.fn()

    render(<SegmentedControl value="app" options={options} onValueChange={onValueChange} />)

    fireEvent.click(screen.getByRole('radio', { name: 'Window' }))

    expect(onValueChange).toHaveBeenCalledWith('window')
    expect(screen.getByRole('radio', { name: 'App' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('radio', { name: 'Window' })).toHaveAttribute('aria-checked', 'false')
  })

  it('does not emit changes for disabled options', () => {
    const onValueChange = vi.fn()

    render(<SegmentedControl defaultValue="app" options={options} onValueChange={onValueChange} />)

    fireEvent.click(screen.getByRole('radio', { name: 'Disabled' }))

    expect(onValueChange).not.toHaveBeenCalled()
    expect(screen.getByRole('radio', { name: 'App' })).toHaveAttribute('aria-checked', 'true')
  })

  it('keeps localized labels on one line without shrinking the options', () => {
    render(<SegmentedControl defaultValue="app" options={options} />)

    expect(screen.getByRole('radio', { name: 'Window' })).toHaveClass('shrink-0', 'whitespace-nowrap')
  })

  it('uses one tab stop and keyboard selection that skips disabled options and wraps', async () => {
    const user = userEvent.setup()
    render(<SegmentedControl defaultValue="app" options={options} />)
    const app = screen.getByRole('radio', { name: 'App' })
    const window = screen.getByRole('radio', { name: 'Window' })

    await user.tab()
    expect(app).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(window).toHaveFocus()
    expect(window).toHaveAttribute('aria-checked', 'true')
    expect(app).toHaveAttribute('tabindex', '-1')
    await user.keyboard('{ArrowRight}')
    expect(app).toHaveFocus()
    await user.keyboard('{End}')
    expect(window).toHaveFocus()
    await user.keyboard('{Home}')
    expect(app).toHaveFocus()
    expect(app).toHaveAttribute('aria-checked', 'true')
  })

  it('moves keyboard focus while a controlled selection waits for its owner', async () => {
    const user = userEvent.setup()
    const onValueChange = vi.fn()
    const view = render(<SegmentedControl value="app" options={options} onValueChange={onValueChange} />)
    await user.tab()
    await user.keyboard('{ArrowDown}')

    expect(onValueChange).toHaveBeenCalledWith('window')
    expect(screen.getByRole('radio', { name: 'Window' })).toHaveFocus()
    expect(screen.getByRole('radio', { name: 'App' })).toHaveAttribute('aria-checked', 'true')
    view.rerender(<SegmentedControl value="window" options={options} onValueChange={onValueChange} />)
    expect(screen.getByRole('radio', { name: 'Window' })).toHaveAttribute('aria-checked', 'true')
  })

  it('keeps selection usable when reduced motion changes and releases the media listener', async () => {
    const user = userEvent.setup()
    let notify = () => {}
    const query = {
      matches: true,
      addEventListener: vi.fn((_type: string, listener: () => void) => {
        notify = listener
      }),
      removeEventListener: vi.fn()
    }
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => query)
    )
    const view = render(<SegmentedControl defaultValue="app" options={options} />)

    await user.tab()
    await user.keyboard('{ArrowRight}')
    expect(screen.getByRole('radio', { name: 'Window' })).toHaveAttribute('aria-checked', 'true')
    const stillPlate = view.container.querySelector('[data-slot="segmented-control-indicator"]')
    act(() => {
      query.matches = false
      notify()
    })
    expect(view.container.querySelector('[data-slot="segmented-control-indicator"]')).not.toBe(stillPlate)
    expect(screen.getByRole('radio', { name: 'Window' })).toHaveFocus()
    await user.keyboard('{ArrowLeft}')
    expect(screen.getByRole('radio', { name: 'App' })).toHaveAttribute('aria-checked', 'true')
    expect(view.container.querySelectorAll('[data-slot="segmented-control-indicator"]')).toHaveLength(1)
    view.unmount()
    expect(query.removeEventListener).toHaveBeenCalledWith('change', notify)
  })

  it('follows horizontal direction while vertical keys keep their order', async () => {
    const user = userEvent.setup()
    render(<SegmentedControl options={[...options.slice(0, 2), { value: 'third', label: 'Third' }]} />)
    const app = screen.getByRole('radio', { name: 'App' })
    app.style.direction = 'rtl'
    await user.tab()
    await user.keyboard('{ArrowRight}')
    expect(screen.getByRole('radio', { name: 'Third' })).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    expect(app).toHaveFocus()
  })
})
