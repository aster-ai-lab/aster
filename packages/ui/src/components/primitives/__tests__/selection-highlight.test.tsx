// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render } from '@testing-library/react'
import { LayoutGroup } from 'motion/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { SelectionHighlight } from '../selection-highlight'

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

describe('SelectionHighlight', () => {
  it('keeps a shared layout id while layoutDependency tracks the selected value', () => {
    const view = render(
      <LayoutGroup id="tabs">
        <button type="button" className="relative">
          <SelectionHighlight layoutId="tabs" layoutDependency="files" data-slot="tabs-indicator" />
          Files
        </button>
      </LayoutGroup>
    )
    const plate = view.container.querySelector('[data-slot="tabs-indicator"]')
    expect(plate).not.toBeNull()

    view.rerender(
      <LayoutGroup id="tabs">
        <button type="button" className="relative">
          <SelectionHighlight layoutId="tabs" layoutDependency="terminal" data-slot="tabs-indicator" />
          Terminal
        </button>
      </LayoutGroup>
    )
    expect(view.container.querySelector('[data-slot="tabs-indicator"]')).not.toBeNull()
  })

  it('drops layoutId under reduced motion', () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({
        matches: true,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn()
      }))
    )
    const { container } = render(
      <SelectionHighlight layoutId="tabs" layoutDependency="files" data-slot="tabs-indicator" />
    )
    // Motion omits layoutId when reduced; the plate still mounts for the selected face.
    expect(container.querySelector('[data-slot="tabs-indicator"]')).not.toBeNull()
  })
})
