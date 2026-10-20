import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { render, screen } from '@testing-library/react'
import postcss from 'postcss'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import type { ComposerSerializedToken } from '@renderer/components/composer/tokens'

import { useWorkbenchHomeComposer } from '../WorkbenchHomeComposerExtras'
import { readTaskPermission } from '../workbenchPermissionPostures'

// Exercise the shipped Radix thumb/keyboard implementation, not the global switch stub.
vi.unmock('@a3s-lab/ui')
vi.mock('@renderer/hooks/useStudioGoal', () => ({ useStudioGoal: vi.fn() }))

function ShippedHomeToolsRow({ tokens = [] }: { tokens?: ComposerSerializedToken[] } = {}) {
  const home = useWorkbenchHomeComposer({
    enabled: true,
    fileCount: 0,
    text: '',
    onTextChange: () => undefined,
    permissionMode: 'default',
    tokens
  })
  return home.renderLayout({})
}

describe('actual planning switch', () => {
  it('keeps the compact capsule when mounted in the actual composer toolbar cascade', () => {
    localStorage.clear()
    const style = document.createElement('style')
    // The design-system sizing is below its owning host styles, as in the real Tailwind output.
    style.textContent = '.cs-switch-root { height: 18px; width: 32px; border-radius: 9999px; }\n' +
      readFileSync(resolve('src/renderer/pages/agents/workbenchComposer.css'), 'utf8')
    document.head.append(style)
    // The contract transform assigns test JSX its own data-ui. Create the actual host ancestry explicitly.
    const host = document.createElement('div')
    host.dataset.ui = 'workbench.composer'
    host.innerHTML = '<div data-composer-toolbar><div></div></div>'
    document.body.append(host)
    try {
      render(<ShippedHomeToolsRow />, { container: host.firstElementChild!.firstElementChild as HTMLElement })
      const toggle = screen.getByRole('switch', { name: '规划模式' })
      expect(readFileSync(resolve('src/renderer/pages/agents/workbenchComposer.css'), 'utf8')).toContain('height: 30px')
      const chipSizingRules: string[] = []
      postcss.parse(readFileSync(resolve('src/renderer/pages/agents/workbenchComposer.css'), 'utf8')).walkRules((rule) => {
        if (!rule.nodes.some((node) => node.type === 'decl' && node.prop === 'height' && node.value === '30px')) return
        expect(toggle.closest('[data-composer-toolbar]')).toBeTruthy()
        if (rule.selector.includes('[data-composer-toolbar]')) chipSizingRules.push(rule.selector)
      })
      // Happy DOM does not implement this descendant/:first-child cascade. Check the parsed owning rule as well.
      expect(chipSizingRules).toHaveLength(1)
      expect(chipSizingRules[0]).toContain("button:not([role='switch'])")
      expect(getComputedStyle(toggle).height).toBe('18px')
      expect(getComputedStyle(toggle).width).toBe('32px')
      expect(getComputedStyle(toggle).borderRadius).toBe('9999px')
    } finally {
      style.remove()
      host.remove()
    }
  })

  it('renders the planning switch thumb in normal flow and keeps keyboard and IME behavior safe', async () => {
    localStorage.clear()
    const user = userEvent.setup()
    render(<ShippedHomeToolsRow />)
    const toggle = screen.getByRole('switch', { name: '规划模式' })
    const thumb = toggle.querySelector('[data-slot="switch-thumb"]')
    expect(thumb).toBeInTheDocument()
    expect(thumb).toHaveClass('block')
    expect(thumb).not.toHaveClass('absolute')
    expect(thumb?.querySelector('svg')).toHaveClass('size-full')
    const posture = readTaskPermission('draft')?.mode ?? 'default'
    toggle.focus()
    expect(toggle).toHaveFocus()
    await user.keyboard(' ')
    expect(toggle).toHaveAttribute('aria-checked', 'true')
    expect(thumb).toHaveAttribute('data-state', 'checked')
    const composing = new KeyboardEvent('keydown', { key: 'Enter', isComposing: true, bubbles: true, cancelable: true })
    expect(toggle.dispatchEvent(composing)).toBe(false)
    const legacyIme = new KeyboardEvent('keydown', { key: 'Enter', keyCode: 229, bubbles: true, cancelable: true })
    expect(toggle.dispatchEvent(legacyIme)).toBe(false)
    expect(toggle).toHaveAttribute('aria-checked', 'true')
    expect(readTaskPermission('draft')?.mode ?? 'default').toBe(posture)
    await user.keyboard(' ')
    expect(toggle).toHaveAttribute('aria-checked', 'false')
  })

  it('keeps an unsupported runtime disabled for both pointer and keyboard input', async () => {
    localStorage.clear()
    const user = userEvent.setup()
    render(
      <ShippedHomeToolsRow
        tokens={[
          {
            id: 'agent',
            index: 0,
            textOffset: 0,
            kind: 'reference',
            label: 'Cloud',
            payload: { agentRuntime: { runtime: 'cloud' } }
          }
        ]}
      />
    )
    const toggle = screen.getByRole('switch', { name: '规划模式' })
    expect(toggle).toBeDisabled()
    await user.click(toggle)
    await user.keyboard(' ')
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    expect(readTaskPermission('draft')?.planFirst ?? false).toBe(false)
  })
})
