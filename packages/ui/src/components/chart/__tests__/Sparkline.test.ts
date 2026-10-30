import { describe, expect, it } from 'vitest'

import { buildSparklinePoints } from '../Sparkline'

describe('buildSparklinePoints', () => {
  it('uses the latest 64 values and keeps a flat series centered', () => {
    expect(buildSparklinePoints([5, 5])).toBe('0,16 48,16')
    expect(buildSparklinePoints(Array.from({ length: 65 }, (_, index) => index)).split(' ')).toHaveLength(64)
  })

  it('maps lower values downward and higher values upward', () => {
    expect(buildSparklinePoints([0, 10])).toBe('0,30 48,2')
  })
})
