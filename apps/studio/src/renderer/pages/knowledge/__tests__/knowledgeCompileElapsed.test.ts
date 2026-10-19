import { describe, expect, it } from 'vitest'

import { compileElapsed } from '../knowledgeCompileElapsed'

describe('compileElapsed', () => {
  it('keeps a short compile under one second and formats longer runs without a percent', () => {
    expect(compileElapsed(0)).toEqual({ kind: 'under' })
    expect(compileElapsed(0.4)).toEqual({ kind: 'under' })
    expect(compileElapsed(3)).toEqual({ kind: 'seconds', count: 3 })
    expect(compileElapsed(65)).toEqual({ kind: 'clock', time: '1:05' })
  })
})
