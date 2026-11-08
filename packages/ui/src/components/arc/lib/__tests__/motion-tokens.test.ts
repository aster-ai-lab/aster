import { describe, expect, it } from 'vitest'

import { motionTokens } from '../motion-tokens'

/**
 * Locks ArcUI / DESIGN.md motion numbers used by product chrome.
 * Shared highlight ~420ms, dialog/popup 160/200, press snappy ~260ms.
 */
describe('motionTokens (DESIGN.md)', () => {
  it('matches dialog / popup enter and exit durations', () => {
    expect(motionTokens.duration.fast).toBe(0.16)
    expect(motionTokens.duration.exit).toBe(0.2)
  })

  it('matches shared-highlight morph spring', () => {
    expect(motionTokens.spring.morph).toMatchObject({
      type: 'spring',
      visualDuration: 0.42,
      bounce: 0
    })
  })

  it('matches press / glyph snappy spring', () => {
    expect(motionTokens.spring.snappy).toMatchObject({
      type: 'spring',
      visualDuration: 0.26,
      bounce: 0
    })
  })

  it('keeps every spring critically damped (DESIGN.md收紧)', () => {
    for (const spring of Object.values(motionTokens.spring)) {
      expect((spring as { bounce?: number }).bounce ?? 0).toBe(0)
    }
  })

  it('keeps durations monotonically increasing', () => {
    const { instant, fast, exit, standard, considered } = motionTokens.duration
    expect([instant, fast, exit, standard, considered]).toEqual(
      [...[instant, fast, exit, standard, considered]].sort((a, b) => a - b)
    )
  })

  it('keeps stagger and blur inside the documented budget', () => {
    expect(Math.max(...Object.values(motionTokens.stagger))).toBeLessThanOrEqual(0.08)
    expect(Math.max(...Object.values(motionTokens.blur))).toBeLessThanOrEqual(8)
  })
})
