// @vitest-environment happy-dom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { SearchSelect } from '../SearchSelect'
import { PortalContainerProvider } from '../../primitives/portal-container'

const options = [
  { value: 'alpha', label: 'Alpha' },
  { value: 'beta', label: 'Beta' },
  { value: 'gamma', label: 'Gamma' }
]

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

describe('SearchSelect search debounce', () => {
  it('filters on every keystroke by default', () => {
    const onChange = vi.fn()
    render(<SearchSelect options={options} value="" onChange={onChange} searchPlaceholder="Search items" />)

    fireEvent.click(screen.getByRole('button', { name: /select/i }))
    fireEvent.change(screen.getByPlaceholderText('Search items'), { target: { value: 'gam' } })

    expect(screen.getByRole('option', { name: 'Gamma' })).toBeInTheDocument()
    expect(screen.queryByRole('option', { name: 'Alpha' })).not.toBeInTheDocument()
    expect(screen.queryByRole('option', { name: 'Beta' })).not.toBeInTheDocument()
  })

  it('settles debounced filtering after the last keystroke when searchDebounceMs is set', async () => {
    vi.useFakeTimers()
    try {
      const onChange = vi.fn()
      render(
        <SearchSelect
          options={options}
          value=""
          onChange={onChange}
          searchPlaceholder="Search items"
          searchDebounceMs={200}
        />
      )

      fireEvent.click(screen.getByRole('button', { name: /select/i }))
      const input = screen.getByPlaceholderText('Search items')
      fireEvent.change(input, { target: { value: 'g' } })
      fireEvent.change(input, { target: { value: 'ga' } })

      expect(screen.getByRole('option', { name: 'Alpha' })).toBeInTheDocument()
      expect(screen.getByRole('option', { name: 'Beta' })).toBeInTheDocument()

      await act(async () => {
        vi.advanceTimersByTime(250)
      })

      expect(screen.queryByRole('option', { name: 'Alpha' })).not.toBeInTheDocument()
      expect(screen.queryByRole('option', { name: 'Beta' })).not.toBeInTheDocument()
      expect(screen.getByRole('option', { name: 'Gamma' })).toBeInTheDocument()
    } finally {
      vi.useRealTimers()
    }
  })
})

describe('SearchSelect menu scroll', () => {
  it('keeps a dialog menu inside its interaction boundary and uses local coordinates', () => {
    const container = document.createElement('div')
    document.body.append(container)
    vi.spyOn(container, 'getBoundingClientRect').mockReturnValue({ left: 100, top: 80 } as DOMRect)
    const onChange = vi.fn()
    render(
      <PortalContainerProvider container={container}>
        <SearchSelect options={options} value="alpha" onChange={onChange} aria-label="Language" />
      </PortalContainerProvider>,
      { container }
    )
    const trigger = screen.getByRole('button', { name: 'Language' })
    vi.spyOn(trigger.parentElement!, 'getBoundingClientRect').mockReturnValue({
      left: 340, top: 180, bottom: 216, width: 160
    } as DOMRect)
    fireEvent.click(trigger)
    const panel = container.querySelector<HTMLElement>('.ui-search-select__panel')!
    expect(panel).toHaveStyle({ position: 'absolute', left: '240px', top: '142px' })
    expect(container).toContainElement(screen.getByRole('listbox'))
    fireEvent.click(screen.getByRole('option', { name: 'Beta' }))
    expect(onChange).toHaveBeenCalledWith('beta')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })
  it('wheels the menu list without scrolling an ancestor', () => {
    const scrollIntoView = vi.fn()
    const previous = HTMLElement.prototype.scrollIntoView
    HTMLElement.prototype.scrollIntoView = scrollIntoView
    try {
      const options = Array.from({ length: 30 }, (_, index) => ({
        value: `v${index}`,
        label: `Label ${index}`
      }))
      render(<SearchSelect options={options} value="v20" onChange={vi.fn()} aria-label="Language" />)
      fireEvent.click(screen.getByRole('button', { name: 'Language' }))
      expect(scrollIntoView).not.toHaveBeenCalled()

      const list = document.body.querySelector('.ui-search-select__list') as HTMLUListElement
      const panel = document.body.querySelector('.ui-search-select__panel') as HTMLElement
      Object.defineProperty(list, 'scrollHeight', { configurable: true, value: 800 })
      Object.defineProperty(list, 'clientHeight', { configurable: true, value: 192 })
      const seen = vi.fn()
      document.addEventListener('wheel', seen)
      const event = new WheelEvent('wheel', { bubbles: true, cancelable: true, deltaY: 40 })
      panel.dispatchEvent(event)
      document.removeEventListener('wheel', seen)

      expect(event.defaultPrevented).toBe(true)
      expect(seen).not.toHaveBeenCalled()
      expect(list.scrollTop).toBe(40)
    } finally {
      HTMLElement.prototype.scrollIntoView = previous
    }
  })
})

describe('SearchSelect menu card', () => {
  it('settles an inactive popup for its lifetime and animates the next foreground opening', () => {
    const focus = vi.spyOn(document, 'hasFocus').mockReturnValue(true)
    try {
      render(<SearchSelect options={options} value="beta" onChange={vi.fn()} aria-label="Status" />)
      const trigger = screen.getByRole('button', { name: 'Status' })
      const panel = () => document.body.querySelector<HTMLElement>('.ui-search-select__panel')!
      fireEvent.click(trigger)
      expect(panel().style.animation).toBe('')
      focus.mockReturnValue(false)
      fireEvent.blur(window)
      expect(panel().style.animation).toBe('none')
      focus.mockReturnValue(true)
      fireEvent.focus(window)
      expect(panel().style.animation).toBe('none')
      fireEvent.click(trigger)
      focus.mockReturnValue(false)
      fireEvent.click(trigger)
      expect(panel().style.animation).toBe('none')
      fireEvent.click(trigger)
      focus.mockReturnValue(true)
      fireEvent.click(trigger)
      expect(panel().style.animation).toBe('')
    } finally {
      focus.mockRestore()
    }
  })

  it('opens a card above the page instead of a bare text stack', () => {
    render(<SearchSelect options={options} value="beta" onChange={vi.fn()} aria-label="Status" />)

    const trigger = screen.getByRole('button', { name: 'Status' })
    expect(trigger.className).toContain('bg-[var(--color-control-fill)]')
    expect(trigger.className).toContain('rounded-[8px]')

    fireEvent.click(trigger)

    const panel = document.body.querySelector('.ui-search-select__panel')
    expect(panel).not.toBeNull()
    expect(panel?.className).toContain('rounded-[12px]')
    expect(panel?.className).toContain('bg-[var(--color-bg-elevated)]')
    expect(panel?.className).toContain('shadow-[var(--shadow-elevated)]')
    expect(panel?.className).toContain('border-[color:var(--color-border)]')

    const selected = screen.getByRole('option', { name: 'Beta' }).querySelector('button')
    expect(selected?.className).toContain('bg-[var(--color-surface)]')
    expect(selected?.className).toContain('font-medium')
  })
})

describe('SearchSelect option activation', () => {
  it('picks when assistive tech activates the option element itself', () => {
    const onChange = vi.fn()
    render(<SearchSelect options={options} value="" onChange={onChange} />)

    fireEvent.click(screen.getByRole('button', { name: /select/i }))
    fireEvent.click(screen.getByRole('option', { name: 'Beta' }))

    expect(onChange).toHaveBeenCalledTimes(1)
    expect(onChange).toHaveBeenCalledWith('beta')
  })

  it('toggles a multi-select option once when its inner button is clicked', () => {
    const onChange = vi.fn()
    render(<SearchSelect multiple options={options} value={[]} onChange={onChange} />)

    fireEvent.click(screen.getByRole('button', { name: /select/i }))
    const option = screen.getByRole('option', { name: 'Beta' })
    const inner = option.querySelector('button')
    expect(inner).not.toBeNull()
    fireEvent.click(inner as HTMLButtonElement)

    expect(onChange).toHaveBeenCalledTimes(1)
    expect(onChange).toHaveBeenCalledWith(['beta'])
  })
})

describe('SearchSelect keyboard', () => {
  it('opens on the chosen option, walks Home and End, and returns focus to the trigger', () => {
    const onChange = vi.fn()
    render(
      <SearchSelect options={options} value="beta" onChange={onChange} aria-label="Status" searchable={false} />
    )
    const trigger = screen.getByRole('button', { name: 'Status' })
    trigger.focus()
    fireEvent.keyDown(trigger, { key: 'ArrowUp' })
    const active = () => document.body.querySelector('.ui-search-select__option.is-active')?.textContent
    expect(active()).toBe('Beta')
    expect(screen.getByRole('option', { name: 'Alpha' }).querySelector('button')).toHaveAttribute('tabindex', '-1')

    fireEvent.keyDown(trigger, { key: 'End' })
    expect(active()).toBe('Gamma')
    fireEvent.keyDown(trigger, { key: 'Home' })
    expect(active()).toBe('Alpha')
    fireEvent.keyDown(trigger, { key: 'Enter' })
    expect(onChange).toHaveBeenCalledWith('alpha')
    expect(trigger).toHaveFocus()

    fireEvent.keyDown(trigger, { key: 'ArrowDown' })
    fireEvent.keyDown(trigger, { key: 'Tab' })
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
  })

  it('exposes the searchable field as a combobox that tracks the active option', () => {
    render(<SearchSelect options={options} value="gamma" onChange={vi.fn()} aria-label="Status" />)
    fireEvent.click(screen.getByRole('button', { name: 'Status' }))
    const field = screen.getByRole('combobox', { name: 'Status' })
    expect(field).toHaveAttribute('aria-activedescendant', screen.getByRole('option', { name: 'Gamma' }).id)
  })
})
