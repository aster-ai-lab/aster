import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { rowMotionClassName, ROW_LEAVE_MS } from '../rowMotion'

const here = dirname(fileURLToPath(import.meta.url))

describe('row motion', () => {
  it('fades and translates, and does not use the enter keyframe', () => {
    const source = readFileSync(join(here, '../rowMotion.ts'), 'utf8')
    const css = readFileSync(join(here, '../../styles/row-motion.css'), 'utf8')
    expect(source).not.toMatch(/animate-in|animate-out|slide-in-from-|slide-out-to-/)
    expect(css).not.toMatch(/transform:|filter:/)
    expect(css).toContain('translate: 0 4px')
    expect(css).toContain('translate: -8px 0')
    expect(css).toContain('a3s-row-in 200ms')
    expect(css).toContain('a3s-row-out 200ms')
    expect(ROW_LEAVE_MS).toBe(200)
    expect(rowMotionClassName()).toContain('a3s-row-enter')
    expect(rowMotionClassName('leaving')).toContain('a3s-row-leave')
    expect(rowMotionClassName('leaving')).not.toContain('a3s-row-enter')
  })
})
