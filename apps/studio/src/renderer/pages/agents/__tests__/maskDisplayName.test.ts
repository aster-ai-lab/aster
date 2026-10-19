import { describe, expect, it } from 'vitest'

import { maskDisplayName } from '../maskDisplayName'

describe('maskDisplayName', () => {
  it('masks a name, a mailbox, and a phone-shaped name without keeping the raw value', () => {
    expect(maskDisplayName('')).toBe('')
    expect(maskDisplayName(' 林知夏 ')).toBe('林*夏')
    expect(maskDisplayName('张三')).toBe('张*')
    expect(maskDisplayName('林')).toBe('林*')
    expect(maskDisplayName('欧阳娜娜')).toBe('欧**娜')
    expect(maskDisplayName('Roy Lin')).toBe('R*y L*n')
    expect(maskDisplayName('lin@example.com')).toBe('l***@example.com')

    const phone = maskDisplayName('13800138000')
    expect(phone).toBe('138****8000')
    expect(phone).not.toContain('13800138000')
    expect(maskDisplayName('+86 13800138000')).toBe('138****8000')
  })
})
