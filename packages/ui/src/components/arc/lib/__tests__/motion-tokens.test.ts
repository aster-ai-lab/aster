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
})
