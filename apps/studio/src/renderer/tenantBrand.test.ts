import { afterEach, describe, expect, it } from 'vitest'

import { studioCompanyName } from './tenantBrand'

describe('studioCompanyName', () => {
  afterEach(() => {
    delete document.documentElement.dataset.a3sCompanyZh
    delete document.documentElement.dataset.a3sCompanyEn
  })

  it('uses the Chinese tenant name for Chinese and the English name otherwise', () => {
    document.documentElement.dataset.a3sCompanyZh = '通友科技'
    document.documentElement.dataset.a3sCompanyEn = 'Toyou AI Labs'
    expect(studioCompanyName('zh-CN')).toBe('通友科技')
    expect(studioCompanyName('zh-TW')).toBe('通友科技')
    expect(studioCompanyName('en-US')).toBe('Toyou AI Labs')
  })

  it('falls back to the name that exists', () => {
    document.documentElement.dataset.a3sCompanyEn = 'Toyou AI Labs'
    expect(studioCompanyName('zh-CN')).toBe('Toyou AI Labs')
    delete document.documentElement.dataset.a3sCompanyEn
    document.documentElement.dataset.a3sCompanyZh = '通友科技'
    expect(studioCompanyName('en-US')).toBe('通友科技')
    delete document.documentElement.dataset.a3sCompanyZh
    expect(studioCompanyName('zh-CN')).toBe('')
  })
})
