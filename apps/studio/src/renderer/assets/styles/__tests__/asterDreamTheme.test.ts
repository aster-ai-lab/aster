import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { afterEach, describe, expect, it } from 'vitest'

const source = readFileSync(resolve('src/renderer/assets/styles/aster-dream-theme.css'), 'utf8')

describe('AsterDream tenant theme', () => {
  afterEach(() => {
    document.querySelector('[data-aster-theme-test]')?.remove()
    document.documentElement.removeAttribute('style')
    document.documentElement.classList.remove('dark')
    delete document.documentElement.dataset.tenant
  })

  it('applies blue/silver in both modes, retains a chosen primary, and leaves toyou untouched', () => {
    const style = document.createElement('style')
    style.dataset.asterThemeTest = ''
    style.textContent = source
    document.head.append(style)
    const root = document.documentElement
    const color = (name: string) => getComputedStyle(root).getPropertyValue(name).trim()
    root.dataset.tenant = 'aster'
    expect(color('--cs-theme-primary')).toBe('#005abb')
    expect(color('--aster-dream-canvas')).toBe('#f7f9fc')
    expect(color('--aster-dream-nav')).toBe('#e9edf3')
    root.classList.add('dark')
    expect(color('--cs-theme-primary')).toBe('#4c9be6')
    expect(color('--aster-dream-canvas')).toBe('#111112')
    root.style.setProperty('--cs-theme-primary', '#ab2244')
    expect(color('--cs-theme-primary')).toBe('#ab2244')
    root.removeAttribute('style')
    root.dataset.tenant = 'toyou'
    expect(color('--cs-theme-primary')).toBe('')
    expect(color('--aster-dream-canvas')).toBe('')
  })
})
