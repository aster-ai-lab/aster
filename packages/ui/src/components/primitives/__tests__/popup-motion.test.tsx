// @vitest-environment jsdom
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '../dropdown-menu'

const here = dirname(fileURLToPath(import.meta.url))

const sources = [
  join(here, '../../../styles/popup-motion.css'),
  join(here, '../dropdown-menu.tsx'),
  join(here, '../context-menu.tsx'),
  join(here, '../popover.tsx'),
  join(here, '../select.tsx'),
  join(here, '../hover-card.tsx'),
  join(here, '../tooltip.tsx'),
  join(here, '../drawer.tsx'),
  join(here, '../../console/SearchSelect.tsx')
]

afterEach(() => {
  cleanup()
})

describe('anchored popup motion', () => {
  it('does not assign transform or filter, and does not use the enter keyframe', () => {
    for (const path of sources) {
      const source = readFileSync(path, 'utf8')
      expect(source, path).not.toMatch(/animate-in|animate-out|slide-in-from-|zoom-in-|zoom-out-/)
      expect(source, path).not.toMatch(/transform:\s+(?!none\b)|filter:/)
    }
    const motion = readFileSync(sources[0], 'utf8')
    expect(motion).toContain(".a3s-popup-rise[data-state='closed']")
    expect(motion).toContain('a3s-popup-leave-below 200ms')
    expect(motion).toContain('a3s-drawer-in 160ms')
    expect(motion).toContain('a3s-drawer-out 200ms')
  })

  it('shows a menu item when the menu is open', () => {
    render(
      <DropdownMenu open>
        <DropdownMenuTrigger>Open</DropdownMenuTrigger>
        <DropdownMenuContent>
          <DropdownMenuItem>Alpha</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    )

    const item = screen.getByRole('menuitem', { name: 'Alpha' })
    const menu = screen.getByRole('menu')
    expect(item).toBeInTheDocument()
    expect(menu).toHaveAttribute('data-side')
    expect(menu.className).not.toMatch(/animate-in|slide-in-from-/)
  })
})
