// @vitest-environment happy-dom
import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { SearchSelect } from '../SearchSelect'
import { PortalContainerProvider } from '../../primitives/portal-container'

const options = [
  { value: 'alpha', label: 'Alpha' },
  { value: 'beta', label: 'Beta' },
  { value: 'gamma', label: 'Gamma' }
]

describe('SearchSelect editor choices', () => {
  it('skips disabled options for keyboard activation and refuses their pointer activation', () => {
    const change = vi.fn()
    render(<SearchSelect searchable={false} allowClear={false} options={[
      { value: 'alpha', label: 'Alpha' },
      { value: 'beta', label: 'Beta', disabled: true },
      { value: 'gamma', label: 'Gamma' }
    ]} value="alpha" onChange={change} />)
    const trigger = screen.getByRole('button', { name: 'Alpha' })
    fireEvent.click(trigger)
    fireEvent.click(screen.getByRole('option', { name: 'Beta' }))
    expect(change).not.toHaveBeenCalled()
    fireEvent.keyDown(trigger, { key: 'ArrowDown' })
    fireEvent.keyDown(trigger, { key: 'Enter' })
    expect(change).toHaveBeenCalledExactlyOnceWith('gamma')
  })

  it('keeps a select-only trigger focused with its active option identified', () => {
    render(<SearchSelect searchable={false} allowClear={false} options={options} value="beta" onChange={vi.fn()} />)
    const trigger = screen.getByRole('button', { name: 'Beta' })
    trigger.focus()
    fireEvent.keyDown(trigger, { key: 'ArrowDown' })
    expect(trigger).toHaveFocus()
    expect(trigger).toHaveAttribute('aria-activedescendant', screen.getByRole('option', { name: 'Beta' }).id)
    fireEvent.keyDown(trigger, { key: 'Escape' })
    expect(trigger).toHaveFocus()
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('shows grouped font previews and option metadata', () => {
    render(<SearchSelect searchable={false} allowClear={false} options={[
      { value: 'serif', label: 'Serif', group: 'Fonts', meta: 'Installed', previewStyle: { fontFamily: 'serif' } }
    ]} value="serif" onChange={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Serif' }))
    expect(screen.getByText('Fonts')).toBeInTheDocument()
    expect(screen.getByText('Installed')).toBeInTheDocument()
    expect(screen.getByRole('option').querySelector('.ui-search-select__option-label')).toHaveStyle({ fontFamily: 'serif' })
  })
})

describe('SearchSelect popup anchoring', () => {
  it.each([
    { viewport: 600, top: 550, bottom: 580, expectedTop: 404, maxHeight: 536 },
    { viewport: 600, top: 390, bottom: 420, expectedTop: 426, maxHeight: 166 },
    { viewport: 200, top: 70, bottom: 100, expectedTop: 106, maxHeight: 86 }
  ])('anchors the menu at trigger top $top in viewport $viewport', ({ viewport, top, bottom, expectedTop, maxHeight }) => {
    vi.stubGlobal('innerHeight', viewport)
    const height = vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockImplementation(function (this: HTMLElement) {
      return this.getAttribute('role') === 'presentation' ? 140 : 0
    })
    try {
      render(
        <SearchSelect
          size="sm"
          searchable={false}
          aria-label="Rows per page"
          options={[10, 20, 50, 100].map(value => ({ value: String(value), label: `${value} / page` }))}
          value="20"
          onChange={vi.fn()}
        />
      )
      const trigger = screen.getByRole('button', { name: 'Rows per page' })
      vi.spyOn(trigger.parentElement!, 'getBoundingClientRect').mockReturnValue({
        left: 340, top, bottom, width: 120
      } as DOMRect)
      fireEvent.click(trigger)
      const panel = screen.getByRole('presentation')
      expect(panel).toHaveStyle({ top: `${expectedTop}px`, left: '340px', maxHeight: `${maxHeight}px` })
      expect(screen.getByRole('option', { name: '100 / page' })).toBeInTheDocument()
    } finally {
      height.mockRestore()
      vi.unstubAllGlobals()
    }
  })
})

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
  it('keeps pointer focus until the option itself has committed its choice', async () => {
    const onChange = vi.fn()
    const user = userEvent.setup()
    render(<SearchSelect searchable={false} allowClear={false} options={options} value="alpha" onChange={onChange} />)
    const trigger = screen.getByRole('button', { name: 'Alpha' })
    await user.click(trigger)
    await user.click(screen.getByRole('option', { name: 'Beta' }))
    expect(onChange).toHaveBeenCalledExactlyOnceWith('beta')
    expect(trigger).toHaveFocus()
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

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
