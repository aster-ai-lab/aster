import { describe, expect, it } from 'vitest'

import { formatCompactDiffCount } from '../diffStat'

describe('formatCompactDiffCount', () => {
  it('keeps small counts and compacts thousands and millions', () => {
    expect(formatCompactDiffCount(999)).toBe('999')
    expect(formatCompactDiffCount(1500)).toBe('1.5k')
    expect(formatCompactDiffCount(1_500_000)).toBe('1.5m')
  })
})
