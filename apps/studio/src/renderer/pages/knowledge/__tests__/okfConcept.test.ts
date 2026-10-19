import { describe, expect, it } from 'vitest'

import { isOkfIndex } from '../okfConcept'

describe('isOkfIndex', () => {
  it('accepts an index whose frontmatter is only the OKF version', () => {
    expect(isOkfIndex('---\nokf_version: "0.2"\n---\n\n- [规范](/规范.md)\n')).toBe(true)
  })

  it('rejects a body that merely mentions the version', () => {
    expect(isOkfIndex('---\ntitle: "索引"\nokf_version: "0.2"\n---\n')).toBe(false)
    expect(isOkfIndex('okf_version: "0.2"\n')).toBe(false)
  })
})
